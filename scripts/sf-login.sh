#!/usr/bin/env bash
#
# SF Novel Flow 多端登录辅助脚本（Linux / macOS / Windows Git Bash）。
#
# 为什么需要脚本
# --------------
# Android 11 起应用读不到其他应用的 Android/data：该目录属主是官方 App、属组是
# 1078(ext_data_rw)，应用进程既拿不到属组也绕不开 SELinux，adb 也无法给已安装的
# 应用补组。所以整条链路交给 adb 代办：
#
#   pm clear 官方 App → 等用户在手机上登录 → 取出含 session_APP 的日志
#   → 在本机解析出 Cookie 与设备编号 → 投递到应用专属目录 → 冷启动本应用
#
# 凭据落在 Android/data/<本应用>/files/login-sync.json，不是「下载」：应用读写
# 自己的外部目录不需要任何权限，其他应用在 Android 11+ 上也看不到该目录。应用启动
# 时读取一次并立即删除（consume_delivered_login）。
#
# 官方 App 日志是追加写的，登录响应的 Set-Cookie 只出现一次，所以 sync 默认先清
# 官方 App 的数据，让本次登录成为日志里唯一的会话。
#
# 子命令
# ------
#   sync   完整流程（默认）：清官方 App 数据 → 等手机登录 → 拉日志解析 → 投递 → 冷启动
#   push   把已有凭据投递到设备：用一台设备的会话登录另一台，或在电脑上粘一份 JSON
#   parse  只解析本机日志文件并打印凭据 JSON，不连接设备
#   list   列出 adb 设备，以及两端应用的安装与凭据目录状态
#
# adb
# ---
# scripts/tools/adb/ 内置 Windows 与 Linux 的 platform-tools（adb 1.0.41 / 37.0.0，
# Apache-2.0，见 tools/adb/NOTICE.txt）。查找顺序：--adb → --fetch-adb → 内置 →
# PATH → ANDROID_HOME / ANDROID_SDK_ROOT 与常见 SDK 目录。
#
# 示例
# ----
#   ./scripts/sf-login.sh                      # 完整同步（会先清官方 App 数据）
#   ./scripts/sf-login.sh sync --skip-clear --yes
#   ./scripts/sf-login.sh list
#   ./scripts/sf-login.sh parse ./com.sfacg.log.0.txt
#   ./scripts/sf-login.sh push --input creds.json
#   pbpaste | ./scripts/sf-login.sh push       # 从剪贴板投递

set -euo pipefail

OFFICIAL_PACKAGE="com.sfacg"
APP_PACKAGE="com.sansan.sf_novel_flow"
LOG_DIR="/sdcard/Android/data/${OFFICIAL_PACKAGE}/files/boluobao/log"
DELIVER_DIR="/sdcard/Android/data/${APP_PACKAGE}/files"
DELIVER_FILE="${DELIVER_DIR}/login-sync.json"
UUID_RE='[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOLS_DIR="${SCRIPT_DIR}/tools/adb"

COMMAND=sync
SERIAL=""
TIMEOUT=300
SKIP_CLEAR=0
NO_LAUNCH=0
ASSUME_YES=0
INPUT_PATH=""
ADB_OVERRIDE=""
FETCH_ADB=0
ADB=""

say() { printf '%s\n' "$*"; }
warn() { printf '警告：%s\n' "$*" >&2; }
die() { printf '错误：%s\n' "$*" >&2; exit 1; }

usage() {
  awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "${BASH_SOURCE[0]}"
}

# ---------------------------------------------------------------- adb 定位

platform_key() {
  case "$(uname -s 2>/dev/null | tr '[:upper:]' '[:lower:]')" in
    linux*) printf 'linux' ;;
    darwin*) printf 'darwin' ;;
    mingw* | msys* | cygwin*) printf 'windows' ;;
    *) printf 'unknown' ;;
  esac
}

adb_name() {
  if [ "$(platform_key)" = "windows" ]; then printf 'adb.exe'; else printf 'adb'; fi
}

# Git Bash / MSYS 下 adb 是 Windows 程序：既不能被转换 /sdcard 这类远端路径，
# 也读不懂 /d/... 这类本机路径，进出两头都要各自处理。
native_path() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -w -- "$1"; else printf '%s' "$1"; fi
}

fetch_adb() {
  local key name target url tmp
  key="$(platform_key)"
  name="$(adb_name)"
  target="${TOOLS_DIR}/${key}/${name}"
  if [ -f "$target" ]; then printf '%s' "$target"; return 0; fi
  [ "$key" = "unknown" ] && die "暂不支持当前系统：$(uname -s)"
  url="https://dl.google.com/android/repository/platform-tools-latest-${key}.zip"
  say "内置 adb 缺失，正在下载 ${url}"
  command -v curl >/dev/null 2>&1 || die "需要 curl 才能下载 platform-tools"
  command -v unzip >/dev/null 2>&1 || die "需要 unzip 才能解压 platform-tools"
  tmp="$(mktemp -d)"
  curl -fsSL "$url" -o "${tmp}/platform-tools.zip" || die "下载失败：${url}"
  unzip -q -o "${tmp}/platform-tools.zip" -d "${tmp}/unpacked" || die "解压失败"
  mkdir -p "${TOOLS_DIR}/${key}"
  cp "${tmp}/unpacked/platform-tools/${name}" "$target"
  [ -f "${tmp}/unpacked/platform-tools/NOTICE.txt" ] &&
    cp "${tmp}/unpacked/platform-tools/NOTICE.txt" "${TOOLS_DIR}/NOTICE.txt"
  # Windows 版 adb 依赖这两个 DLL，缺了会在执行时直接报错。
  for dll in AdbWinApi.dll AdbWinUsbApi.dll; do
    [ -f "${tmp}/unpacked/platform-tools/${dll}" ] &&
      cp "${tmp}/unpacked/platform-tools/${dll}" "${TOOLS_DIR}/${key}/"
  done
  chmod +x "$target"
  rm -rf "$tmp"
  [ -f "$target" ] || die "解压后没有找到 ${name}，请手动放到 ${TOOLS_DIR}/${key}"
  printf '%s' "$target"
}

resolve_adb() {
  local candidate
  if [ -n "$ADB_OVERRIDE" ]; then
    [ -f "$ADB_OVERRIDE" ] || die "--adb 指向的文件不存在：${ADB_OVERRIDE}"
    printf '%s' "$ADB_OVERRIDE"; return 0
  fi
  if [ "$FETCH_ADB" -eq 1 ]; then fetch_adb; return 0; fi
  candidate="${TOOLS_DIR}/$(platform_key)/$(adb_name)"
  if [ -f "$candidate" ]; then
    # 二进制放进 Git 后可能丢掉可执行位，这里兜一层。
    [ -x "$candidate" ] || chmod +x "$candidate" 2>/dev/null || true
    printf '%s' "$candidate"; return 0
  fi
  if command -v adb >/dev/null 2>&1; then command -v adb; return 0; fi
  for candidate in \
    "${ANDROID_HOME:-}/platform-tools/$(adb_name)" \
    "${ANDROID_SDK_ROOT:-}/platform-tools/$(adb_name)" \
    "${HOME}/Android/Sdk/platform-tools/$(adb_name)" \
    "${HOME}/Library/Android/sdk/platform-tools/$(adb_name)" \
    "/usr/lib/android-sdk/platform-tools/$(adb_name)" \
    "/opt/android-sdk/platform-tools/$(adb_name)"; do
    if [ -f "$candidate" ]; then printf '%s' "$candidate"; return 0; fi
  done
  die "找不到 adb：内置目录 scripts/tools/adb 缺失，PATH 与 Android SDK 里也没有。
可加 --fetch-adb 现场下载，或用 --adb 指定 adb 可执行文件。"
}

# ---------------------------------------------------------------- adb 调用

adb_do() {
  if [ -n "$SERIAL" ]; then "$ADB" -s "$SERIAL" "$@"; else "$ADB" "$@"; fi
}

adb_shell() {
  adb_do shell "$1" 2>/dev/null | tr -d '\r'
}

devices_raw() {
  "$ADB" devices -l 2>/dev/null | tr -d '\r' | awk 'NR>1 && NF>=2 {
    model = ""
    for (i = 3; i <= NF; i++) if ($i ~ /^model:/) { sub(/^model:/, "", $i); model = $i }
    print $1, $2, model
  }'
}

pick_serial() {
  local requested=$1 entry state serial model choice index
  local -a ready=() all=()
  while read -r serial state model; do
    [ -n "${serial:-}" ] || continue
    all+=("${serial}（${state}）")
    [ "$state" = "device" ] && ready+=("${serial} ${model}")
  done < <(devices_raw)

  if [ -n "$requested" ]; then
    for entry in ${ready[@]+"${ready[@]}"}; do
      [ "${entry%% *}" = "$requested" ] && { printf '%s' "$requested"; return 0; }
    done
    die "设备 ${requested} 不可用，当前：${all[*]:-无}"
  fi
  if [ "${#ready[@]}" -eq 1 ]; then printf '%s' "${ready[0]%% *}"; return 0; fi
  if [ "${#ready[@]}" -eq 0 ]; then
    die "没有已授权的设备${all[*]+：${all[*]}}，请先执行 adb devices 并在手机上确认 USB 调试"
  fi
  [ -t 0 ] || die "连接了多台设备，请用 --serial 指定：${ready[*]}"
  say "检测到多台设备："
  index=1
  for entry in "${ready[@]}"; do
    say "  ${index}. ${entry}"
    index=$((index + 1))
  done
  read -r -p "选择设备编号（回车取消）：" choice || die "已取消"
  case "$choice" in
    '' | *[!0-9]*) die "已取消" ;;
  esac
  [ "$choice" -ge 1 ] && [ "$choice" -le "${#ready[@]}" ] || die "已取消"
  printf '%s' "${ready[$((choice - 1))]%% *}"
}

resolve_launcher() {
  local output
  output="$(adb_shell "cmd package resolve-activity --brief $1" || true)"
  output="$(printf '%s\n' "$output" | grep '/' | grep -v '=' | tail -n 1 || true)"
  [ -n "$output" ] || die "找不到 $1 的启动入口，请确认应用已安装"
  printf '%s' "$output"
}

app_files_ready() {
  adb_shell "test -d '${DELIVER_DIR}' && echo yes || echo no" | grep -q yes
}

# ---------------------------------------------------------------- 日志解析

session_count() {
  local probe
  probe="$(adb_shell "grep -ho 'session_APP' ${LOG_DIR}/*.txt 2>/dev/null | wc -l" || true)"
  probe="${probe//[^0-9]/}"
  printf '%s' "${probe:-0}"
}

log_files() {
  # 编号越大越旧，按「旧 → 新」输出；拼接后最后一条会话即最新。
  adb_shell "ls ${LOG_DIR}" |
    sed -n 's/^com\.sfacg\.log\.\([0-9][0-9]*\)\.txt$/\1 &/p' |
    sort -rn | cut -d' ' -f2
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

# 从日志文本里取最新一条 App 会话的 Cookie 与设备编号。
#
# 日志把请求头与响应头原样打印：Set-Cookie 给出会话，User-Agent 的第三段或
# SFSecurity 的 devicetoken 给出设备编号。会话不一定来自登录请求——任何一次响应
# 写入 session_APP 都是该设备当前的会话，所以只取最后一条；设备编号取「该会话
# 之前」最近的一次，保证与 Cookie 属于同一台设备。
parse_log() {
  local file=$1 head source session offset community device cookie
  session="$(grep -oE 'session_APP=[0-9A-Za-z]+' "$file" | tail -n 1 || true)"
  [ -n "$session" ] || return 1
  offset="$(grep -obE 'session_APP=[0-9A-Za-z]+' "$file" | tail -n 1)"
  offset="${offset%%:*}"
  community="$(grep -oE '\.SFCommunity=[0-9A-Za-z]+' "$file" | tail -n 1 || true)"

  head="$(mktemp)"
  head -c "$offset" "$file" >"$head" 2>/dev/null || true
  device=""
  for source in "$head" "$file"; do
    device="$(grep -oE "devicetoken=${UUID_RE}" "$source" | tail -n 1 || true)"
    if [ -z "$device" ]; then
      device="$(grep -oE "User-Agent: boluobao/[^/[:space:]]+/[^/[:space:]]+/${UUID_RE}/" "$source" |
        tail -n 1 || true)"
    fi
    [ -n "$device" ] && break
  done
  rm -f "$head"
  [ -n "$device" ] || return 1
  device="$(printf '%s' "$device" | grep -oE "$UUID_RE" | head -n 1 | tr 'A-Z' 'a-z')"

  cookie="session_APP=${session#session_APP=}"
  if [ -n "$community" ]; then
    cookie=".SFCommunity=${community#.SFCommunity=}; ${cookie}"
  fi
  CREDENTIAL_COOKIE="$cookie"
  CREDENTIAL_DEVICE="$device"
  printf '{"cookie":"%s","deviceId":"%s"}\n' "$(json_escape "$cookie")" "$device"
}

# ---------------------------------------------------------------- 凭据投递

read_text() {
  local path=${1:-}
  if [ -z "$path" ] || [ "$path" = "-" ]; then
    [ -t 0 ] && printf '粘贴凭据后按 Ctrl+D（Windows 为 Ctrl+Z 回车）结束：\n' >&2
    cat
  else
    [ -f "$path" ] || die "文件不存在：${path}"
    cat "$path"
  fi
}

json_field() {
  sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$2" | head -n 1
}

line_field() {
  sed -n "s/^[[:space:]]*$1[[:space:]]*[:=][[:space:]]*\\(.*\\)\$/\\1/p" "$2" | head -n 1
}

# 解析应用「复制凭据」得到的 JSON；兼容旧版两行 cookie: / device: 文本。
#
# 后端只收 {cookie, deviceId}，类型由 Cookie 自带：脚本投递会先写设备编号，所以
# 只接受带 session_APP 的 App 会话；网页会话没有配套设备编号，交给应用登录弹窗。
parse_credentials() {
  local file=$1 cookie device_id
  cookie="$(json_field cookie "$file")"
  device_id="$(json_field deviceId "$file")"
  if [ -z "$cookie" ]; then
    cookie="$(line_field cookie "$file")"
    device_id="$(line_field deviceId "$file")"
    [ -n "$device_id" ] || device_id="$(line_field device "$file")"
  fi
  [ -n "$cookie" ] || die "凭据里没有 cookie"
  case "$cookie" in
    *session_APP*) ;;
    *) die "凭据里没有 session_APP：脚本只能投递 App 会话，网页会话请在应用的登录弹窗里粘贴" ;;
  esac
  if ! printf '%s' "$device_id" | grep -qE "^${UUID_RE}\$"; then
    die "凭据里的设备编号不是 UUID，无法投递 App 会话"
  fi
  CREDENTIAL_COOKIE="$cookie"
  CREDENTIAL_DEVICE="$(printf '%s' "$device_id" | tr 'A-Z' 'a-z')"
}

deliver() {
  local tmp json
  tmp="$(mktemp -d)"
  json="${tmp}/login-sync.json"
  printf '{"cookie":"%s","deviceId":"%s"}' \
    "$(json_escape "$CREDENTIAL_COOKIE")" "$CREDENTIAL_DEVICE" >"$json"
  if ! adb_do push "$(native_path "$json")" "$DELIVER_FILE" >/dev/null; then
    rm -rf "$tmp"
    die "投递凭据失败"
  fi
  rm -rf "$tmp"
  say "已投递登录凭据（设备 ${CREDENTIAL_DEVICE}）"
}

launch_app() {
  local activity
  activity="$(resolve_launcher "$1")"
  # 必须冷启动：消费逻辑只在前端初始化时执行，切前台不会重新触发。
  adb_do shell "am force-stop $1" >/dev/null 2>&1 || true
  adb_do shell "am start -n $activity" >/dev/null 2>&1 || true
}

confirm_clear() {
  [ -t 0 ] || die "无法交互确认，非交互运行时请加 --yes"
  local answer=""
  read -r -p "确认清除并重新登录？输入 yes 继续：" answer || die "已取消"
  [ "$answer" = "yes" ] || die "已取消"
}

# ---------------------------------------------------------------- 子命令

cmd_sync() {
  adb_do shell "pm list packages" >/dev/null 2>&1 || die "adb 无法执行 shell 命令，请确认设备已授权"
  local installed
  installed="$(adb_shell "pm list packages")"
  case "$installed" in
    *"package:${OFFICIAL_PACKAGE}"*) ;;
    *) die "手机上没装官方 App（${OFFICIAL_PACKAGE}），无法取日志" ;;
  esac
  case "$installed" in
    *"package:${APP_PACKAGE}"*) ;;
    *) die "手机上没装 SF Novel Flow（${APP_PACKAGE}），请先安装并打开一次" ;;
  esac
  # 目录由应用首次启动时创建；不存在说明还没打开过，此时投递会写进 shell 名下的
  # 目录，应用反而删不掉自己那份凭据。
  app_files_ready ||
    die "应用专属目录还不存在，请先在手机上打开一次 SF Novel Flow，再重新运行本脚本"

  if [ "$SKIP_CLEAR" -eq 0 ]; then
    warn "将清除官方 App（${OFFICIAL_PACKAGE}）的全部本地数据：登录状态与已下载内容都会丢失。"
    [ "$ASSUME_YES" -eq 1 ] || confirm_clear
    local cleared
    cleared="$(adb_shell "pm clear ${OFFICIAL_PACKAGE}")"
    case "$cleared" in
      *Success*) ;;
      *) die "清除官方 App 数据失败：${cleared}" ;;
    esac
    # Android/data 不随 pm clear 清空，旧日志必须一起删掉，否则会读到历史会话。
    adb_do shell "rm -rf ${LOG_DIR}" >/dev/null 2>&1 || true
  fi

  local baseline deadline found=0 activity
  baseline="$(session_count)"
  activity="$(resolve_launcher "$OFFICIAL_PACKAGE")"
  adb_do shell "am start -n $activity" >/dev/null 2>&1 || true
  say "已在手机上打开官方 App，请完成登录（账号密码或验证码）。等待写入会话……"

  deadline=$(($(date +%s) + TIMEOUT))
  printf '等待中'
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ "$(session_count)" -gt "$baseline" ]; then found=1; break; fi
    printf '.'
    sleep 2
  done
  printf '\n'
  [ "$found" -eq 1 ] || die "等待 ${TIMEOUT} 秒仍未查到登录会话，请确认已在手机上登录成功"

  pull_credentials
  deliver
  if [ "$NO_LAUNCH" -eq 1 ]; then
    say "打开 SF Novel Flow 即会自动完成登录。"
    return 0
  fi
  launch_app "$APP_PACKAGE"
  say "已启动 SF Novel Flow，应用会自动读取并完成登录。"
}

pull_credentials() {
  local names tmp name local_file chunks="" combined
  names="$(log_files)"
  [ -n "$names" ] || die "未找到登录信息，请确认官方 App 已登录成功"
  tmp="$(mktemp -d)"
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    local_file="${tmp}/${name}"
    if ! adb_do pull "${LOG_DIR}/${name}" "$(native_path "$local_file")" >/dev/null; then
      rm -rf "$tmp"
      die "拉取日志失败：${name}"
    fi
    chunks="${chunks}
$(cat "$local_file")"
  done <<<"$names"
  combined="${tmp}/combined.log"
  printf '%s\n' "$chunks" >"$combined"
  # 日志留在本机解析，手机上只留一份几百字节的凭据。
  if ! parse_log "$combined" >/dev/null; then
    rm -rf "$tmp"
    die "日志里没能解析出会话：没有 session_APP，或缺少配套的设备编号"
  fi
  rm -rf "$tmp"
}

cmd_push() {
  local tmp
  tmp="$(mktemp)"
  read_text "$INPUT_PATH" >"$tmp"
  parse_credentials "$tmp"
  rm -f "$tmp"
  app_files_ready ||
    die "应用专属目录还不存在，请先在手机上打开一次 SF Novel Flow"
  deliver
  if [ "$NO_LAUNCH" -eq 1 ]; then
    say "打开 SF Novel Flow 即会自动完成登录。"
    return 0
  fi
  launch_app "$APP_PACKAGE"
  say "已启动 SF Novel Flow，应用会自动读取并完成登录。"
}

cmd_parse() {
  local tmp
  [ -n "$INPUT_PATH" ] || INPUT_PATH="-"
  tmp="$(mktemp)"
  read_text "$INPUT_PATH" >"$tmp"
  parse_log "$tmp" || {
    rm -f "$tmp"
    die "没有可用的会话（缺 session_APP 或设备编号）"
  }
  rm -f "$tmp"
}

cmd_list() {
  local serial state model name release packages official app ready
  local printed=0
  while read -r serial state model; do
    [ -n "${serial:-}" ] || continue
    printed=1
    if [ "$state" != "device" ]; then
      say "${serial}（${state}）未授权或离线"
      continue
    fi
    SERIAL="$serial"
    name="$(adb_shell 'getprop ro.product.model')"
    release="$(adb_shell 'getprop ro.build.version.release')"
    packages="$(adb_shell 'pm list packages')"
    case "$packages" in
      *"package:${OFFICIAL_PACKAGE}"*) official="已安装" ;;
      *) official="未安装" ;;
    esac
    case "$packages" in
      *"package:${APP_PACKAGE}"*) app="已安装" ;;
      *) app="未安装" ;;
    esac
    if app_files_ready; then ready="凭据目录就绪"; else ready="尚未启动过"; fi
    say "${name:-$serial}（${serial}）Android ${release}"
    say "  官方 App（${OFFICIAL_PACKAGE}）：${official}"
    say "  SF Novel Flow（${APP_PACKAGE}）：${app}，${ready}"
  done < <(devices_raw)
  [ "$printed" -eq 1 ] || die "没有已连接的设备，请先执行 adb devices"
}

# ---------------------------------------------------------------- 入口

while [ $# -gt 0 ]; do
  case "$1" in
    sync | push | parse | list) COMMAND="$1" ;;
    -h | --help) usage; exit 0 ;;
    --serial) SERIAL=${2:?'--serial 需要一个值'}; shift ;;
    --serial=*) SERIAL=${1#*=} ;;
    --timeout) TIMEOUT=${2:?'--timeout 需要一个值'}; shift ;;
    --timeout=*) TIMEOUT=${1#*=} ;;
    --input) INPUT_PATH=${2:?'--input 需要一个值'}; shift ;;
    --input=*) INPUT_PATH=${1#*=} ;;
    --adb) ADB_OVERRIDE=${2:?'--adb 需要一个值'}; shift ;;
    --adb=*) ADB_OVERRIDE=${1#*=} ;;
    --skip-clear) SKIP_CLEAR=1 ;;
    --no-launch) NO_LAUNCH=1 ;;
    --yes) ASSUME_YES=1 ;;
    --fetch-adb) FETCH_ADB=1 ;;
    -*) die "未知参数：$1（--help 查看用法）" ;;
    *)
      if [ "$COMMAND" = "parse" ] && [ -z "$INPUT_PATH" ]; then
        INPUT_PATH="$1"
      else
        die "未知参数：$1（--help 查看用法）"
      fi
      ;;
  esac
  shift
done

case "$COMMAND" in
  parse) cmd_parse ;;
  *)
    # Git Bash / MSYS 下不要把 /sdcard/... 转成 Windows 路径，否则远端命令全错。
    case "$(platform_key)" in
      windows) export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' ;;
    esac
    ADB="$(resolve_adb)"
    if [ "$COMMAND" = "list" ]; then
      # list 面向全部设备，不参与单设备选择。
      cmd_list
    else
      SERIAL="$(pick_serial "$SERIAL")"
      case "$COMMAND" in
        sync) cmd_sync ;;
        push) cmd_push ;;
      esac
    fi
    ;;
esac
