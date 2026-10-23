# 压测数据集生成器 —— 输出引擎中立的 INSERT 语句,MySQL 与 PostgreSQL 共用同一份。
#
# 为什么不用 CSV + LOAD DATA / \copy:MySQL 的 local_infile 实测是 OFF,
# 打开它需要 SET GLOBAL(违反「不改数据库配置」的约束)。批量 INSERT 两边都能吃,
# 一次性成本(几十秒)只在建库时付一次,不影响每轮复位的速度。
#
# 为什么没有随机种子:所有字段都由行号 i 直接派生,不存在 RNG 状态,
# 因此「同参数两次生成逐字节相同」是构造性保证,不依赖种子实现的稳定性。
# 这比 spec §4.2 写的「固定随机种子」更强。

Set-StrictMode -Version Latest

# RuoYi 与 axum 的 init 脚本里 admin 用的同一个 hash:明文 admin123,cost 10。
# 全体用户共用它,让 login 场景两边算同样多的 bcrypt(公平性 F11)。
$script:BcryptAdmin123 = '$2a$10$7JB720yubVSZvUI0rEqK/.VqGOZTH.ulu33dHOiBE8ByOhJIrdAu2'
$script:FixedTime      = '2026-01-01 00:00:00'
$script:DeptIdBase     = 900
$script:UserIdBase     = 1000000
# 压测自带的角色与岗位。
#
# 不能借用种子里的 ID:实测 sys_role 的 ID 全是雪花号(1761300000000000001 等),
# 没有 role_id=2;sys_post 更是**一行都没有**。第一版生成器写死 role_id=2 /
# post_id=4,20 万条 sys_user_role / sys_user_post 全部悬空 —— 没有外键约束所以
# 插入照样成功,但 /system/user/{id} 取不到角色、write_mix 的 PUT 会写回不存在
# 的 ID。自带就没有这个依赖。
#
# data_scope='2'(自定义/本部门)而不是 '1'(全部数据):给 20 万用户发超管权限
# 会让数据权限过滤形同虚设,那是另一种失真。
$script:BenchRoleId = 990001
$script:BenchPostId = 990002

function New-SeedSql {
    param(
        [Parameter(Mandatory)][string]$OutPath,
        [int]$UserCount = 200000,
        [int]$DeptCount = 100,
        [int]$BatchSize = 1000
    )

    $dir = Split-Path -Parent $OutPath
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

    # 提到 script 作用域:下面的行生成 scriptblock 由 Write-SeedBatch 调用,
    # 跨函数引用 New-SeedSql 的局部 $DeptCount 依赖 PowerShell 的作用域回溯 ——
    # 能 work 但太隐晦,一旦将来把 Write-SeedBatch 挪进模块就会静默取到 $null。
    $script:GenDeptCount = $DeptCount

    # StreamWriter 而非 Add-Content:20 万行用后者要几分钟,用它是几秒。
    # 显式 UTF8Encoding($false) = 不带 BOM —— mysql 客户端遇到 BOM 会把它
    # 当成第一条语句的一部分而报语法错。
    $sw = [System.IO.StreamWriter]::new($OutPath, $false, [System.Text.UTF8Encoding]::new($false))
    try {
        $sw.WriteLine('-- 压测数据集,由 bench/seed/gen-seed-sql.ps1 生成,勿手改。')
        $sw.WriteLine("-- 用户 $UserCount 人 / 部门 $DeptCount 个")
        $sw.WriteLine()

        # ---- sys_dept:DeptCount 个平级子部门,挂在 init 种子的 dept 100 下 ----
        # 树形只做两层:压测关心的是 dept 表的连接与过滤成本,不是树的深度。
        $sw.WriteLine('INSERT INTO sys_dept (dept_id, parent_id, ancestors, dept_name, dept_category, order_num, leader, phone, email, status, del_flag, create_dept, create_by, create_time, update_by, update_time) VALUES')
        $deptRows = for ($i = 1; $i -le $DeptCount; $i++) {
            $id = $script:DeptIdBase + $i
            "($id, 100, '0,100', '压测部门$i', NULL, $i, NULL, NULL, NULL, '0', '0', 103, 1, '$script:FixedTime', NULL, NULL)"
        }
        $sw.WriteLine(($deptRows -join ",`n") + ';')
        $sw.WriteLine()

        # ---- 压测专用角色与岗位 ----
        # sys_user_role / sys_user_post 要指向真实存在的行,否则详情接口取不到
        # 角色、写场景会写回不存在的 ID。种子里没有可借用的(见文件头注释)。
        $sw.WriteLine('INSERT INTO sys_role (role_id, role_name, role_key, role_sort, data_scope, menu_check_strictly, dept_check_strictly, status, del_flag, create_dept, create_by, create_time, update_by, update_time, remark) VALUES')
        $sw.WriteLine("($script:BenchRoleId, '压测角色', 'bench_role', 99, '2', 1, 1, '0', '0', 103, 1, '$script:FixedTime', NULL, NULL, '压测数据集自带');")
        $sw.WriteLine()

        $sw.WriteLine('INSERT INTO sys_post (post_id, dept_id, post_code, post_category, post_name, post_sort, status, create_dept, create_by, create_time, update_by, update_time, remark, del_flag) VALUES')
        $sw.WriteLine("($script:BenchPostId, 100, 'bench_post', NULL, '压测岗位', 99, '0', 103, 1, '$script:FixedTime', NULL, NULL, '压测数据集自带', '0');")
        $sw.WriteLine()

        # ---- sys_user ----
        # dept_id 用乘法散列打散:若用 i % DeptCount,user_id 与 dept_id 完全同相位,
        # idx_sys_user_dept_id 的物理顺序会过于理想,不反映真实分布。
        # 37 与 100 互质,保证 DeptCount=100 时每个部门恰好 UserCount/100 人。
        $userHeader = 'INSERT INTO sys_user (user_id, dept_id, user_name, nick_name, user_type, email, phone_number, gender, avatar, password, status, del_flag, login_ip, login_date, create_dept, create_by, create_time, update_by, update_time, remark) VALUES'
        Write-SeedBatch -Writer $sw -Header $userHeader -Count $UserCount -BatchSize $BatchSize -Row {
            param($i)
            $uid   = $script:UserIdBase + $i
            $dept  = $script:DeptIdBase + (($i * 37) % $script:GenDeptCount) + 1
            $name  = 'bench_u{0:D6}' -f $i
            $phone = '13{0:D9}' -f (($i * 7919) % 1000000000)
            $gen   = $i % 3
            "($uid, $dept, '$name', '压测用户$i', 'sys_user', '$name@example.com', '$phone', '$gen', NULL, '$script:BcryptAdmin123', '0', '0', '127.0.0.1', '$script:FixedTime', 103, 1, '$script:FixedTime', NULL, NULL, NULL)"
        }

        # ---- sys_user_role:每人 1 个压测角色 ----
        Write-SeedBatch -Writer $sw -Header 'INSERT INTO sys_user_role (user_id, role_id) VALUES' `
            -Count $UserCount -BatchSize $BatchSize -Row {
            param($i) "($($script:UserIdBase + $i), $script:BenchRoleId)"
        }

        # ---- sys_user_post:每人 1 个压测岗位 ----
        Write-SeedBatch -Writer $sw -Header 'INSERT INTO sys_user_post (user_id, post_id) VALUES' `
            -Count $UserCount -BatchSize $BatchSize -Row {
            param($i) "($($script:UserIdBase + $i), $script:BenchPostId)"
        }
    }
    finally { $sw.Dispose() }

    $hash = (Get-FileHash -Path $OutPath -Algorithm SHA256).Hash
    @{ Path = $OutPath; Users = $UserCount; Depts = $DeptCount; Sha256 = $hash }
}

# 分批写 INSERT:单条几十万行的 VALUES 会撑爆 max_allowed_packet,
# 也让出错时定位不到行。每 BatchSize 行一条语句。
function Write-SeedBatch {
    param(
        [Parameter(Mandatory)][System.IO.StreamWriter]$Writer,
        [Parameter(Mandatory)][string]$Header,
        [Parameter(Mandatory)][int]$Count,
        [Parameter(Mandatory)][int]$BatchSize,
        [Parameter(Mandatory)][scriptblock]$Row
    )
    for ($start = 1; $start -le $Count; $start += $BatchSize) {
        $end = [Math]::Min($start + $BatchSize - 1, $Count)
        $Writer.WriteLine($Header)
        $buf = [System.Text.StringBuilder]::new()
        for ($i = $start; $i -le $end; $i++) {
            if ($i -gt $start) { [void]$buf.Append(",`n") }
            [void]$buf.Append((& $Row $i))
        }
        $Writer.WriteLine($buf.ToString() + ';')
    }
    $Writer.WriteLine()
}
