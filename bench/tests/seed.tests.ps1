# gen-seed-sql.ps1 的单测。由 bench/test-bench.ps1 dot-source。
. (Join-Path $PSScriptRoot '../seed/gen-seed-sql.ps1')

$tmp1 = Join-Path ([System.IO.Path]::GetTempPath()) 'bench-seed-a.sql'
$tmp2 = Join-Path ([System.IO.Path]::GetTempPath()) 'bench-seed-b.sql'

section '数据集生成'
$r1 = New-SeedSql -OutPath $tmp1 -UserCount 1000 -DeptCount 10 -BatchSize 400
assert_eq '用户数' 1000 $r1.Users
assert_eq '部门数' 10   $r1.Depts

section '确定性 —— 同参数两次生成必须逐字节相同'
# 数据完全由行号派生,不用 RNG,所以连种子都不需要。这是 PG 与 MySQL
# 能拿到「逐字节相同数据」的根据。
$r2 = New-SeedSql -OutPath $tmp2 -UserCount 1000 -DeptCount 10 -BatchSize 400
assert_eq '两次 sha256 一致' $r1.Sha256 $r2.Sha256

section 'SQL 引擎中立性'
$sql = Get-Content $tmp1 -Raw
# 用 [char]0x60 取反引号,别在源码里裸写 —— PowerShell 里它是转义符,
# 裸写在字符串中极易被后续编辑误伤。
assert_notmatch '无反引号(MySQL 专用标识符)' ([char]0x60) $sql
assert_notmatch '无 ENGINE 子句'  'ENGINE='      $sql
assert_notmatch '无 ON DUPLICATE' 'ON DUPLICATE' $sql
assert_match   '有 sys_user 插入'      'INSERT INTO sys_user'      $sql
assert_match   '有 sys_dept 插入'      'INSERT INTO sys_dept'      $sql
assert_match   '有 sys_user_role 插入' 'INSERT INTO sys_user_role' $sql
assert_match   '有 sys_user_post 插入' 'INSERT INTO sys_user_post' $sql

section '口令统一 —— login 场景两边算同样多的 bcrypt'
# RuoYi 与 axum 的 init 脚本里 admin 用的都是这个 hash(明文 admin123, cost 10)
assert_match '统一 bcrypt hash' '$2a$10$7JB720yubVSZvUI0rEqK/.VqGOZTH.ulu33dHOiBE8ByOhJIrdAu2' $sql

section 'dept_id 打散 —— 不能与 user_id 同相位'
# 用乘法散列(i*37 % DeptCount)而非 i%DeptCount:后者让 user_id 与 dept_id
# 完全同相位,idx_sys_user_dept_id 的物理顺序会过于理想,不反映真实分布。
# DeptCount=10 时:dept = 900 + ((i*37) % 10) + 1
#   i=1 → 900 + 7 + 1 = 908 ; i=2 → 900 + 4 + 1 = 905 ; i=3 → 900 + 1 + 1 = 902
# 相邻用户的 dept 差值不为 1,即打散生效。
assert_match 'i=1 落 dept 908' '(1000001, 908,' $sql
assert_match 'i=2 落 dept 905' '(1000002, 905,' $sql
assert_match 'i=3 落 dept 902' '(1000003, 902,' $sql

section '角色/岗位引用必须有效'
# 回归测试:第一版写死 role_id=2 / post_id=4,但种子里的角色 ID 全是雪花号
# (没有 2 号),sys_post 更是一行都没有 —— 20 万条关联全部悬空。没有外键约束
# 所以插入照样成功,但详情接口取不到角色、写场景会写回不存在的 ID。
assert_match '自带压测角色' 'INSERT INTO sys_role' $sql
assert_match '自带压测岗位' 'INSERT INTO sys_post' $sql
assert_match '角色 ID 990001' '(990001,' $sql
assert_match '岗位 ID 990002' '(990002,' $sql
assert_match '关联指向压测角色' '(1000001, 990001)' $sql
assert_match '关联指向压测岗位' '(1000001, 990002)' $sql
assert_notmatch '不再引用 role_id=2' '(1000001, 2)' $sql
# data_scope='2' 而非 '1':给 20 万用户发超管权限会让数据权限过滤形同虚设
assert_match '角色非超管数据范围' "'bench_role', 99, '2'" $sql

section '分批 —— 单条 INSERT 不能塞下全部行'
# 单条几十万行的 VALUES 会撑爆 max_allowed_packet,也让出错时定位不到行。
$insertCount = ([regex]::Matches($sql, 'INSERT INTO sys_user \(')).Count
assert_eq '1000 行 / 批 400 → 3 条 sys_user INSERT' 3 $insertCount

Remove-Item $tmp1, $tmp2 -ErrorAction SilentlyContinue
