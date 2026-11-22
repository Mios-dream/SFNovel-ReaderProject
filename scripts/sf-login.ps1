<#
.SYNOPSIS
  SF Novel Flow 多端登录辅助脚本（Windows PowerShell 5.1+）。

.DESCRIPTION
  Android 11 起应用读不到其他应用的 Android/data：该目录属主是官方 App、属组是
  1078(ext_data_rw)，应用进程既拿不到属组也绕不开 SELinux，adb 也无法给已安装的
  应用补组。所以整条链路交给 adb 代办：

    pm clear 官方 App → 等用户在手机上登录 → 取出含 session_APP 的日志
    → 在本机解析出 Cookie 与设备编号 → 投递到应用专属目录 → 冷启动本应用

  凭据落在 Android/data/<本应用>/files/login-sync.json，不是「下载」：应用读写
  自己的外部目录不需要任何权限，其他应用在 Android 11+ 上也看不到该目录。应用启动
  时读取一次并立即删除（consume_delivered_login）。

  官方 App 日志是追加写的，登录响应的 Set-Cookie 只出现一次，所以 sync 默认先清
  官方 App 的数据，让本次登录成为日志里唯一的会话。

  子命令：
    sync   完整流程（默认）：清官方 App 数据 → 等手机登录 → 拉日志解析 → 投递 → 冷启动
    push   把已有凭据投递到设备：用一台设备的会话登录另一台，或在电脑上粘一份 JSON
    parse  只解析本机日志文件并打印凭据 JSON，不连接设备
    list   列出 adb 设备，以及两端应用的安装与凭据目录状态

  adb：scripts/tools/adb/ 内置 Windows 与 Linux 的 platform-tools（adb 1.0.41 /
  37.0.0，Apache-2.0，见 tools/adb/NOTICE.txt）。查找顺序：-Adb → -FetchAdb → 内置
  → PATH → ANDROID_HOME / ANDROID_SDK_ROOT 与常见 SDK 目录。

.PARAMETER Command
  sync / push / parse / list，默认 sync。

.PARAMETER Serial
  连接了多台设备时指定序列号，取值见 adb devices。

.PARAMETER TimeoutSeconds
  等待官方 App 登录完成的秒数，默认 300。

.PARAMETER SkipClear
  不清官方 App 数据。日志里可能混有旧会话，仅在你确定不需要时使用。

.PARAMETER NoLaunch
  投递完成后不启动 SF Novel Flow。

.PARAMETER Yes
  跳过清数据的确认提示。

.PARAMETER InputFile
  push / parse 的输入文件；push 省略时从标准输入读取。parse 可直接跟在子命令后面。

.PARAMETER Adb
  指定 adb 可执行文件，默认用内置版本。

.PARAMETER FetchAdb
  允许从 dl.google.com 下载 platform-tools 到 scripts/tools/adb。

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\sf-login.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\sf-login.ps1 sync -SkipClear -Yes

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\sf-login.ps1 list

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\sf-login.ps1 parse .\com.sfacg.log.0.txt

.EXAMPLE
  Get-Content .\creds.json -Raw | powershell -File scripts\sf-login.ps1 push
#>
[CmdletBinding()]
param(
  [Parameter(Position = 0)]
  [ValidateSet("sync", "push", "parse", "list")]
  [string]$Command = "sync",
  [string]$Serial = "",
  [int]$TimeoutSeconds = 300,
  [switch]$SkipClear,
  [switch]$NoLaunch,
  [switch]$Yes,
  [Parameter(Position = 1)]
  [string]$InputFile = "",
  [string]$Adb = "",
  [switch]$FetchAdb,
  [switch]$Help
)

# adb 会往 stderr 写守护进程提示，Stop 会让这些提示中断脚本，故只按退出码判断。
$ErrorActionPreference = "Continue"

$OfficialPackage = "com.sfacg"
$AppPackage = "com.sansan.sf_novel_flow"
$LogDir = "/sdcard/Android/data/$OfficialPackage/files/boluobao/log"
$DeliverDir = "/sdcard/Android/data/$AppPackage/files"
$DeliverFile = "$DeliverDir/login-sync.json"
$UuidPattern = '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}'
$ToolsDir = Join-Path $PSScriptRoot "tools/adb"

function Write-Say([string]$Message) { Write-Host $Message }
function Write-Warn([string]$Message) { Write-Warning $Message }

# ---------------------------------------------------------------- adb 定位

function Get-PlatformKey {
  if ($PSVersionTable.PSEdition -ne "Core") { return "windows" }
  if ($IsWindows) { return "windows" }
  if ($IsMacOS) { return "darwin" }
  return "linux"
}

function Get-AdbName {
  if ((Get-PlatformKey) -eq "windows") { return "adb.exe" }
  return "adb"
}

function Install-BundledAdb {
  $platform = Get-PlatformKey
  $name = Get-AdbName
  $target = Join-Path (Join-Path $ToolsDir $platform) $name
  if (Test-Path -LiteralPath $target) { return $target }

  $url = "https://dl.google.com/android/repository/platform-tools-latest-$platform.zip"
  Write-Say "内置 adb 缺失，正在下载 $url"
  $temp = Join-Path ([IO.Path]::GetTempPath()) ("platform-tools-" + [guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Path $temp -Force | Out-Null
  try {
    $archive = Join-Path $temp "platform-tools.zip"
    Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
    Expand-Archive -LiteralPath $archive -DestinationPath $temp -Force
    $unpacked = Join-Path $temp "platform-tools"
    New-Item -ItemType Directory -Path (Join-Path $ToolsDir $platform) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $unpacked $name) -Destination $target -Force
    # Windows 版 adb 依赖这两个 DLL，缺了会在执行时直接报错。
    foreach ($extra in @("AdbWinApi.dll", "AdbWinUsbApi.dll")) {
      $source = Join-Path $unpacked $extra
      if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $ToolsDir $platform) -Force
      }
    }
    $notice = Join-Path $unpacked "NOTICE.txt"
    if (Test-Path -LiteralPath $notice) { Copy-Item -LiteralPath $notice -Destination $ToolsDir -Force }
  } finally {
    Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
  }
  if (-not (Test-Path -LiteralPath $target)) {
    throw "解压后没有找到 $name，请手动放到 $(Join-Path $ToolsDir $platform)"
  }
  return $target
}

function Resolve-Adb {
  if ($Adb) {
    if (-not (Test-Path -LiteralPath $Adb)) { throw "-Adb 指向的文件不存在：$Adb" }
    return (Resolve-Path -LiteralPath $Adb).Path
  }
  if ($FetchAdb) { return Install-BundledAdb }

  $bundled = Join-Path (Join-Path $ToolsDir (Get-PlatformKey)) (Get-AdbName)
  if (Test-Path -LiteralPath $bundled) { return $bundled }

  $onPath = Get-Command adb -ErrorAction SilentlyContinue
  if ($onPath) { return $onPath.Source }

  $roots = @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT,
    (Join-Path $HOME "Android/Sdk"), (Join-Path $HOME "Library/Android/sdk"),
    "/usr/lib/android-sdk", "/opt/android-sdk")
  if ($env:LOCALAPPDATA) { $roots += (Join-Path $env:LOCALAPPDATA "Android/Sdk") }
  foreach ($root in $roots) {
    if (-not $root) { continue }
    foreach ($name in @("adb.exe", "adb")) {
      $candidate = Join-Path (Join-Path $root "platform-tools") $name
      if (Test-Path -LiteralPath $candidate) { return $candidate }
    }
  }
  throw "找不到 adb：内置目录 scripts/tools/adb 缺失，PATH 与 Android SDK 里也没有。可加 -FetchAdb 现场下载，或用 -Adb 指定 adb 可执行文件。"
}

# ---------------------------------------------------------------- adb 调用

function Invoke-Adb {
  param([Parameter(Mandatory)][string[]]$Arguments)
  $prefix = @()
  if ($script:SelectedSerial) { $prefix = @("-s", $script:SelectedSerial) }
  $output = & $script:AdbPath @prefix @Arguments 2>&1
  [PSCustomObject]@{ Output = @($output); Code = $LASTEXITCODE }
}

function Get-Devices {
  $lines = (Invoke-Adb @("devices", "-l")).Output |
    ForEach-Object { "$_".Trim() } |
    Where-Object { $_ -and $_ -notmatch "^List of devices" }
  foreach ($line in $lines) {
    $parts = $line -split "\s+"
    if ($parts.Count -lt 2) { continue }
    $model = ($parts | Where-Object { $_ -like "model:*" } | Select-Object -First 1)
    if ($model) { $model = $model.Substring("model:".Length) }
    [PSCustomObject]@{ Serial = $parts[0]; State = $parts[1]; Model = $model }
  }
}

function Select-Serial {
  param([string]$Requested)
  $devices = @(Get-Devices)
  $ready = @($devices | Where-Object { $_.State -eq "device" })
  if ($Requested) {
    if ($ready | Where-Object { $_.Serial -eq $Requested }) { return $Requested }
    $current = ($devices | ForEach-Object { "$($_.Serial)（$($_.State)）" }) -join "、"
    if (-not $current) { $current = "无" }
    throw "设备 $Requested 不可用，当前：$current"
  }
  if ($ready.Count -eq 1) { return $ready[0].Serial }
  if ($ready.Count -eq 0) {
    $current = ($devices | ForEach-Object { "$($_.Serial)（$($_.State)）" }) -join "、"
    $suffix = ""
    if ($current) { $suffix = "：$current" }
    throw "没有已授权的设备$suffix，请先执行 adb devices 并在手机上确认 USB 调试"
  }
  Write-Say "检测到多台设备："
  for ($i = 0; $i -lt $ready.Count; $i++) {
    Write-Say ("  {0}. {1}  {2}" -f ($i + 1), $ready[$i].Serial, $ready[$i].Model)
  }
  $choice = Read-Host "选择设备编号（回车取消）"
  if ($choice -notmatch '^\d+$' -or [int]$choice -lt 1 -or [int]$choice -gt $ready.Count) {
    throw "已取消"
  }
  return $ready[[int]$choice - 1].Serial
}

# 从 `cmd package resolve-activity --brief` 的输出里取启动组件。
function Resolve-Launcher {
  param([string]$Package)
  $resolved = (Invoke-Adb @("shell", "cmd package resolve-activity --brief $Package")).Output |
    ForEach-Object { "$_".Trim() } |
    Where-Object { $_ -like "*/*" -and $_ -notlike "*=*" } |
    Select-Object -Last 1
  if (-not $resolved) { throw "找不到 $Package 的启动入口，请确认应用已安装" }
  return $resolved
}

function Test-AppFilesReady {
  return (Invoke-Adb @("shell", "test -d '$DeliverDir' && echo yes || echo no")).Output -join "" -match "yes"
}

# ---------------------------------------------------------------- 日志解析

# 从日志文本里取最新一条 App 会话的 Cookie 与设备编号。
#
# 日志把请求头与响应头原样打印：`Set-Cookie` 给出会话，`User-Agent` 的第三段或
# `SFSecurity` 的 `devicetoken` 给出设备编号。会话不一定来自登录请求——任何一次
# 响应写入 session_APP 都是该设备当前的会话，所以只取最后一条。
# 设备编号取「该会话之前」最近的一次，保证与 Cookie 属于同一台设备。
function Convert-LogToCredentials {
  param([string]$Text)

  $session = [regex]::Matches($Text, 'session_APP=([0-9A-Za-z]+)') | Select-Object -Last 1
  if (-not $session) { return $null }
  $community = [regex]::Matches($Text, '\.SFCommunity=([0-9A-Za-z]+)') | Select-Object -Last 1

  $device = $null
  foreach ($source in @($Text.Substring(0, $session.Index), $Text)) {
    foreach ($pattern in @("devicetoken=($UuidPattern)", "User-Agent: boluobao/[^/\r\n]+/[^/\r\n]+/($UuidPattern)/")) {
      $match = [regex]::Matches($source, $pattern) | Select-Object -Last 1
      if ($match) { $device = $match.Groups[1].Value.ToLower(); break }
    }
    if ($device) { break }
  }
  if (-not $device) { return $null }

  $cookie = "session_APP=$($session.Groups[1].Value)"
  if ($community) { $cookie = ".SFCommunity=$($community.Groups[1].Value); $cookie" }
  [PSCustomObject]@{ cookie = $cookie; deviceId = $device }
}

# 统计日志里的会话条数；文件不存在时按 0 处理。
function Get-SessionCount {
  $probe = (Invoke-Adb @("shell", "grep -ho 'session_APP' $LogDir/*.txt 2>/dev/null | wc -l")).Output -join ""
  $value = 0
  [void][int]::TryParse($probe.Trim(), [ref]$value)
  return $value
}

# 轮转文件编号越大越旧；按旧到新排序，拼接后最后一条会话即最新。
function Get-LogFiles {
  (Invoke-Adb @("shell", "ls $LogDir")).Output |
    ForEach-Object { "$_".Trim() } |
    Where-Object { $_ -match '^com\.sfacg\.log\.(\d+)\.txt$' } |
    Sort-Object -Property @{ Expression = { [int]([regex]::Match($_, '(\d+)').Groups[1].Value) } } -Descending
}

# ---------------------------------------------------------------- 凭据投递

function Read-InputText {
  param([string]$Path)
  if ($Path) {
    if (-not (Test-Path -LiteralPath $Path)) { throw "文件不存在：$Path" }
    return Get-Content -Raw -Encoding UTF8 -LiteralPath $Path
  }
  if (-not [Console]::IsInputRedirected) {
    Write-Say "粘贴凭据后按 Ctrl+Z 回车结束："
  }
  return [Console]::In.ReadToEnd()
}

# 解析应用「复制凭据」得到的 JSON；兼容旧版两行 cookie: / device: 文本。
#
# 后端只收 {cookie, deviceId}，类型由 Cookie 自带：脚本投递会先写设备编号，所以只
# 接受带 session_APP 的 App 会话；网页会话没有配套设备编号，交给应用登录弹窗处理。
function ConvertTo-Credential {
  param([string]$Text)
  $text = $Text.Trim().TrimStart([char]0xFEFF)
  if (-not $text) { throw "没有读到凭据内容" }

  $cookie = ""
  $deviceId = ""
  $payload = $null
  try { $payload = $text | ConvertFrom-Json } catch { $payload = $null }
  if ($payload -and $payload.PSObject.Properties.Name -contains "cookie") {
    $cookie = "$($payload.cookie)".Trim()
    $deviceId = "$($payload.deviceId)".Trim()
  } else {
    foreach ($line in ($text -split "`r?`n")) {
      if ($line -match '^\s*(?i:cookie)\s*[:=]\s*(.+)$') { $cookie = $Matches[1].Trim() }
      elseif ($line -match '^\s*(?i:deviceId|device)\s*[:=]\s*(.+)$') { $deviceId = $Matches[1].Trim() }
    }
  }
  if (-not $cookie) { throw "凭据里没有 cookie" }
  if ($cookie -notmatch "session_APP") {
    throw "凭据里没有 session_APP：脚本只能投递 App 会话，网页会话请在应用的登录弹窗里粘贴"
  }
  if ($deviceId -notmatch "^$UuidPattern$") {
    throw "凭据里的设备编号不是 UUID，无法投递 App 会话"
  }
  [PSCustomObject]@{ cookie = $cookie; deviceId = $deviceId.ToLower() }
}

function Send-Credentials {
  param([PSCustomObject]$Credentials)
  $temp = Join-Path ([IO.Path]::GetTempPath()) ("sf-login-" + [guid]::NewGuid().ToString("N") + ".json")
  try {
    [IO.File]::WriteAllText($temp, ($Credentials | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
    $push = Invoke-Adb @("push", $temp, $DeliverFile)
    if ($push.Code -ne 0) { throw "投递凭据失败：$($push.Output -join ' ')" }
  } finally {
    Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
  }
  Write-Say "已投递登录凭据（设备 $($Credentials.deviceId)）"
}

function Start-Me {
  param([string]$Package)
  $activity = Resolve-Launcher $Package
  # 必须冷启动：消费逻辑只在前端初始化时执行，切前台不会重新触发。
  Invoke-Adb @("shell", "am force-stop $Package") | Out-Null
  Invoke-Adb @("shell", "am start -n $activity") | Out-Null
}

function Confirm-Clear {
  try {
    if ((Read-Host "确认清除并重新登录？输入 yes 继续") -ne "yes") { throw "已取消" }
  } catch {
    throw "已取消"
  }
}

# ---------------------------------------------------------------- 子命令

function Invoke-Sync {
  $installed = (Invoke-Adb @("shell", "pm list packages")).Output -join "`n"
  if ($installed -notmatch [regex]::Escape("package:$OfficialPackage")) {
    throw "手机上没装官方 App（$OfficialPackage），无法取日志"
  }
  if ($installed -notmatch [regex]::Escape("package:$AppPackage")) {
    throw "手机上没装 SF Novel Flow（$AppPackage），请先安装并打开一次"
  }
  # 目录由应用首次启动时创建；不存在说明还没打开过，此时投递会写进 shell 名下的目录，
  # 应用反而删不掉自己那份凭据。
  if (-not (Test-AppFilesReady)) {
    throw "应用专属目录还不存在，请先在手机上打开一次 SF Novel Flow，再重新运行本脚本"
  }

  if (-not $SkipClear) {
    Write-Warn "将清除官方 App（$OfficialPackage）的全部本地数据：登录状态与已下载内容都会丢失。"
    if (-not $Yes) {
      if (-not [Console]::IsInputRedirected) {
        Confirm-Clear
      } else {
        throw "无法交互确认，非交互运行时请加 -Yes"
      }
    }
    $cleared = Invoke-Adb @("shell", "pm clear $OfficialPackage")
    if (($cleared.Output -join " ") -notmatch "Success") {
      throw "清除官方 App 数据失败：$($cleared.Output -join ' ')"
    }
    # Android/data 不随 pm clear 清空，旧日志必须一起删掉，否则会读到历史会话。
    Invoke-Adb @("shell", "rm -rf $LogDir") | Out-Null
  }

  $baseline = Get-SessionCount
  Invoke-Adb @("shell", "am start -n $(Resolve-Launcher $OfficialPackage)") | Out-Null
  Write-Say "已在手机上打开官方 App，请完成登录（账号密码或验证码）。等待写入会话……"

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  $found = $false
  while ((Get-Date) -lt $deadline) {
    if ((Get-SessionCount) -gt $baseline) { $found = $true; break }
    Start-Sleep -Seconds 2
  }
  if (-not $found) {
    throw "等待 $TimeoutSeconds 秒仍未查到登录会话，请确认已在手机上登录成功"
  }

  $credentials = Receive-Credentials
  Send-Credentials $credentials
  if ($NoLaunch) {
    Write-Say "打开 SF Novel Flow 即会自动完成登录。"
    return
  }
  Start-Me $AppPackage
  Write-Say "已启动 SF Novel Flow，应用会自动读取并完成登录。"
}

function Receive-Credentials {
  $names = @(Get-LogFiles)
  if ($names.Count -eq 0) { throw "未找到登录信息，请确认官方 App 已登录成功" }

  # 日志留在本机解析，手机上只留一份几百字节的凭据。
  $tempDir = Join-Path ([IO.Path]::GetTempPath()) ("sfacg-login-" + [guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
  try {
    $chunks = foreach ($name in $names) {
      $local = Join-Path $tempDir $name
      $pull = Invoke-Adb @("pull", "$LogDir/$name", $local)
      if ($pull.Code -ne 0 -or -not (Test-Path -LiteralPath $local)) {
        throw "拉取日志失败：$name"
      }
      Get-Content -Raw -Encoding UTF8 -LiteralPath $local
    }
    $credentials = Convert-LogToCredentials ($chunks -join "`n")
    if (-not $credentials) {
      throw "日志里没能解析出会话：没有 session_APP，或缺少配套的设备编号"
    }
    return $credentials
  } finally {
    Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
  }
}

function Invoke-Push {
  $credentials = ConvertTo-Credential (Read-InputText $InputFile)
  if (-not (Test-AppFilesReady)) {
    throw "应用专属目录还不存在，请先在手机上打开一次 SF Novel Flow"
  }
  Send-Credentials $credentials
  if ($NoLaunch) {
    Write-Say "打开 SF Novel Flow 即会自动完成登录。"
    return
  }
  Start-Me $AppPackage
  Write-Say "已启动 SF Novel Flow，应用会自动读取并完成登录。"
}

function Invoke-Parse {
  $credentials = Convert-LogToCredentials (Read-InputText $InputFile)
  if (-not $credentials) { throw "没有可用的会话（缺 session_APP 或设备编号）" }
  $credentials | ConvertTo-Json -Compress
}

function Invoke-List {
  $devices = @(Get-Devices)
  if ($devices.Count -eq 0) { throw "没有已连接的设备，请先执行 adb devices" }
  foreach ($device in $devices) {
    if ($device.State -ne "device") {
      Write-Say "$($device.Serial)（$($device.State)）未授权或离线"
      continue
    }
    $script:SelectedSerial = $device.Serial
    $name = ((Invoke-Adb @("shell", "getprop ro.product.model")).Output -join "").Trim()
    if (-not $name) { $name = $device.Model }
    if (-not $name) { $name = $device.Serial }
    $release = ((Invoke-Adb @("shell", "getprop ro.build.version.release")).Output -join "").Trim()
    $packages = (Invoke-Adb @("shell", "pm list packages")).Output -join "`n"
    $official = if ($packages -match [regex]::Escape("package:$OfficialPackage")) { "已安装" } else { "未安装" }
    $app = if ($packages -match [regex]::Escape("package:$AppPackage")) { "已安装" } else { "未安装" }
    $ready = if (Test-AppFilesReady) { "凭据目录就绪" } else { "尚未启动过" }
    Write-Say "$name（$($device.Serial)）Android $release"
    Write-Say "  官方 App（$OfficialPackage）：$official"
    Write-Say "  SF Novel Flow（$AppPackage）：$app，$ready"
  }
}

# ---------------------------------------------------------------- 入口

if ($Help) {
  Get-Help $PSCommandPath -Detailed
  exit 0
}

try {
  if ($Command -eq "parse") {
    Invoke-Parse
    exit 0
  }
  $script:AdbPath = Resolve-Adb
  if ($Command -ne "list") {
    $script:SelectedSerial = Select-Serial $Serial
  }
  switch ($Command) {
    "sync" { Invoke-Sync }
    "push" { Invoke-Push }
    "list" { Invoke-List }
  }
} catch {
  Write-Host "错误：$($_.Exception.Message)" -ForegroundColor Red
  exit 1
}
