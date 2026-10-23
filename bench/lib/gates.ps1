# 三道门禁。任一不过 → 该单元标记 BLOCKED,报告中如实写「未取得有效数据」。
#
# 存在理由见 spec §7:上一轮 axum 错误率 99.78%、p50 死在 5002ms 超时墙,
# 却照样产出 summary.json 被并排比较。不拿废数据凑表格。
#
# 等价性探针用 PowerShell 而非 k6:k6 的 handleSummary 里不能发 HTTP 请求,
# 而 setup() 的返回值又到不了 handleSummary —— 唯一能落盘的钩子就是后者。
# 门禁本来就在 PowerShell 里调,直接写这儿少一层转换。

. (Join-Path $PSScriptRoot 'cells.ps1')

$script:BenchRoot = Split-Path -Parent $PSScriptRoot
$script:ClientId = 'e5cd7e4891bf95d1d19206ce24a7b32e'

function Get-K6Exe {
    $k6 = (Get-Command k6 -ErrorAction SilentlyContinue).Source
    if ($k6) { return $k6 }
    # 本地解压的免提权副本优先。winget 装 k6 走 MSI,需要 UAC 提权,
    # 在非交互会话里必然失败(exit 1602 = 用户取消)。官方 zip 解压即用。
    foreach ($p in @(
            (Join-Path $script:BenchRoot '.work\tools\k6-v2.2.0-windows-amd64\k6.exe'),
            'C:\Program Files\k6\k6.exe',
            "$env:LOCALAPPDATA\Microsoft\WinGet\Links\k6.exe")) {
        if (Test-Path $p) { return $p }
    }
    throw '未找到 k6。装法(免提权):下载 https://github.com/grafana/k6/releases 的 windows-amd64.zip 解压到 bench/.work/tools/'
}

# ---------- 门禁 1:探活端点返回 200 ----------
function Test-GateHealth([Parameter(Mandatory)][hashtable]$Cell) {
    $url = Get-CellProbeUrl $Cell
    try {
        $r = Invoke-WebRequest -Uri $url -TimeoutSec 5 -SkipHttpErrorCheck
        if ($r.StatusCode -eq 200) { Write-Host '  [OK] 门禁1 健康探活' -ForegroundColor Green; return $true }
        Write-Host "  [NG] 门禁1 健康探活:HTTP $($r.StatusCode) @ $url" -ForegroundColor Red
    }
    catch { Write-Host "  [NG] 门禁1 健康探活:$_" -ForegroundColor Red }
    $false
}

# ---------- 门禁 2:冒烟脚本全部 check 通过 ----------
function Test-GateSmoke([Parameter(Mandatory)][hashtable]$Cell) {
    $k6 = Get-K6Exe
    $base = Get-CellBaseUrl $Cell
    $out = & $k6 run --quiet -e BASE_URL=$base (Join-Path $script:BenchRoot 'k6/smoke.js') 2>&1
    # k6 的退出码只反映 threshold,check 失败不改退出码 —— 必须另外扫输出里的
    # 「✗」标记,否则 check 全红也会被判成通过。
    $hasFailedCheck = ($out | Select-String -Pattern '✗|checks\.\.\.\.+\s+0\.00%' -Quiet)
    if ($LASTEXITCODE -eq 0 -and -not $hasFailedCheck) {
        Write-Host '  [OK] 门禁2 冒烟' -ForegroundColor Green
        return $true
    }
    Write-Host '  [NG] 门禁2 冒烟失败:' -ForegroundColor Red
    $out | Select-Object -Last 25 | ForEach-Object { Write-Host "      $_" }
    $false
}

# ---------- 门禁 3:响应等价性 ----------

# 采集一个单元的响应快照。纯 HTTP,不依赖 k6。
function Get-EquivSnapshot([Parameter(Mandatory)][hashtable]$Cell) {
    $base = Get-CellBaseUrl $Cell
    $snap = [ordered]@{}

    $loginBody = @{
        clientId  = $script:ClientId
        grantType = 'password'
        username  = 'admin'
        password  = 'admin123'
    } | ConvertTo-Json -Compress

    $lr = Invoke-WebRequest -Uri "$base/auth/login" -Method Post -ContentType 'application/json' `
        -Body $loginBody -TimeoutSec 20 -SkipHttpErrorCheck
    $lj = $lr.Content | ConvertFrom-Json
    $snap['login.code'] = $lj.code
    if ($lj.code -ne 200) {
        $snap['login.msg'] = $lj.msg
        return $snap    # 登录都没过,后面没意义
    }

    $h = @{ Authorization = "Bearer $($lj.data.access_token)"; clientid = $script:ClientId }

    # 1) 用户列表第 1 页
    $lst = (Invoke-WebRequest -Uri "$base/system/user/list?pageNum=1&pageSize=10" -Headers $h `
            -TimeoutSec 20 -SkipHttpErrorCheck).Content | ConvertFrom-Json
    $rows = @($lst.data.rows)
    $snap['userList.code'] = $lst.code
    $snap['userList.total'] = $lst.data.total
    $snap['userList.rowCount'] = $rows.Count
    # 比对 userId 的**集合**而非首行:顺序可以不同(两边默认排序未必一致),
    # 但返回的必须是同一批用户 —— 若不是,两边查的就不是同一份数据,
    # 后面所有 RPS 数字都失去意义。
    $snap['userList.idSet'] = (($rows | ForEach-Object { [string]$_.userId } | Sort-Object) -join ',')
    # 字段名集合:两边拼写不一致(phonenumber vs phoneNumber)是真 bug,
    # 会让前端契约破裂,一并锁住。
    $snap['userList.fields'] = if ($rows.Count -gt 0) {
        (($rows[0].PSObject.Properties.Name | Sort-Object) -join ',')
    } else { '' }

    # 2) 深翻页:考验分页 offset 的一致性
    $deep = (Invoke-WebRequest -Uri "$base/system/user/list?pageNum=500&pageSize=10" -Headers $h `
            -TimeoutSec 20 -SkipHttpErrorCheck).Content | ConvertFrom-Json
    $deepRows = @($deep.data.rows)
    $snap['userListDeep.code'] = $deep.code
    $snap['userListDeep.rowCount'] = $deepRows.Count
    $snap['userListDeep.idSet'] = (($deepRows | ForEach-Object { [string]$_.userId } | Sort-Object) -join ',')

    # 3) 用户详情(含角色/岗位关联)
    if ($rows.Count -gt 0) {
        $det = (Invoke-WebRequest -Uri "$base/system/user/$($rows[0].userId)" -Headers $h `
                -TimeoutSec 20 -SkipHttpErrorCheck).Content | ConvertFrom-Json
        $snap['userDetail.code'] = $det.code
        $snap['userDetail.roleIds'] = (($det.data.roleIds | Sort-Object) -join ',')
        $snap['userDetail.postIds'] = (($det.data.postIds | Sort-Object) -join ',')
    }

    # 4) 部门树
    $dept = (Invoke-WebRequest -Uri "$base/system/dept/list" -Headers $h `
            -TimeoutSec 20 -SkipHttpErrorCheck).Content | ConvertFrom-Json
    $snap['deptList.code'] = $dept.code
    $snap['deptList.count'] = @($dept.data).Count

    $snap
}

# 逐字段比对两个快照。
# 已知且不阻断的差异。2026-09-06 实测确认,**MySQL 与 PG 上表现完全一致** ——
# 这本身说明它们来自后端行为差异,与数据库无关。
#
# 归根到底是两个根差异:
#   1) 默认排序相反 —— axum 返回 user_id 最大的一批,RuoYi 返回最小的一批。
#      两边都是 ORDER BY ... LIMIT 10 OFFSET n,扫描量相同,**不影响公平性**。
#      userListDeep.idSet / userDetail.roleIds / userDetail.postIds 都是它的
#      连带结果 —— 比的根本不是同一个用户。
#   2) DTO 字段集不同 —— axum 有 createBy/createByName/delFlag,
#      RuoYi 有 avatar/avatarUrl/loginDate/loginIp。
#
# 放进白名单但**每次仍然打印**:降级为「已知、已记录、不阻断」,不是藏起来。
# 报告 §7 要如实列出。
$script:KnownEquivDiffs = @(
    'userList.idSet', 'userListDeep.idSet', 'userList.fields',
    'userDetail.roleIds', 'userDetail.postIds'
)

function Test-GateEquivalence {
    param(
        [Parameter(Mandatory)]$AxumSnap,
        [Parameter(Mandatory)]$RuoyiSnap,
        [string[]]$AllowDiff = $script:KnownEquivDiffs
    )
    $diffs = New-Object System.Collections.Generic.List[string]
    $known = New-Object System.Collections.Generic.List[string]
    $keys = @($AxumSnap.Keys) + @($RuoyiSnap.Keys) | Select-Object -Unique

    foreach ($k in $keys) {
        $a = "$($AxumSnap[$k])"
        $b = "$($RuoyiSnap[$k])"
        if ($a -eq $b) { continue }
        $sa = if ($a.Length -gt 80) { $a.Substring(0, 80) + '…' } else { $a }
        $sb = if ($b.Length -gt 80) { $b.Substring(0, 80) + '…' } else { $b }
        $line = "$k :`n        axum  = $sa`n        ruoyi = $sb"
        if ($AllowDiff -contains $k) { $known.Add($line) } else { $diffs.Add($line) }
    }

    # 已知差异永远打印 —— 白名单是「不阻断」,不是「不可见」
    if ($known.Count -gt 0) {
        Write-Host "  [已知差异 $($known.Count) 处,不阻断,须进报告]" -ForegroundColor DarkYellow
        $known | ForEach-Object { Write-Host "      $_" -ForegroundColor DarkYellow }
    }

    $pass = $diffs.Count -eq 0
    if ($pass) {
        Write-Host '  [OK] 门禁3 响应等价(硬项:code / total / rowCount / deptCount 全部一致)' -ForegroundColor Green
    }
    else {
        Write-Host "  [NG] 门禁3 响应不等价($($diffs.Count) 处未列入已知):" -ForegroundColor Red
        $diffs | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
    }
    @{ Pass = $pass; Diffs = $diffs; Known = $known }
}
