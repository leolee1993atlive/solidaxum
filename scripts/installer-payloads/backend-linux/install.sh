#!/usr/bin/env bash
# @REPO_NAME@ backend - Linux 交互式安装
# 用法: sudo ./install.sh
#
# 服务以"本脚本所在目录"为 WorkingDirectory 注册到 systemd,不再向 /opt
# 等位置 copy 文件。要换位置请先 uninstall,mv 整个目录到目标路径后
# 再次运行 install.sh。
#
# 交互式流程:收集端口/DB/Redis → 调用后端 --write-config 渲染 config.toml
# (CSPRNG 自动生成 jwt.secret / security.aes_key)→ --check-config 实测
# DB+Redis 连通 → 注册 systemd → 启动 → 探活。
#
# 本脚本可被 source(用于单测),此时不执行 main —— 见文件末尾的守卫。

set -Eeuo pipefail

# PAYLOAD_DIR = 包内文件来源(脚本所在目录,原地安装)。
PAYLOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="$PAYLOAD_DIR"
REPO_NAME="$(basename "$PREFIX")"

# 单测覆盖此变量指向临时目录,避免碰真的 /etc
SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"

# ---------- 输入校验 ----------

# 后缀只允许小写字母、数字、连字符(对齐 .iss 的 SuffixIsValid)。
# 空后缀合法 = 正式环境单实例。
suffix_is_valid() {
  [[ "$1" =~ ^[a-z0-9-]*$ ]]
}

# 非负整数。不接受空串、前导 +/-、空白、小数点。
is_uint() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

# 端口 1-65535。10# 前缀强制十进制,否则 "08080" 会被当八进制而报错。
port_is_valid() {
  is_uint "$1" || return 1
  # Bash 算术展开对超过 int64 的十进制字面量按模 2^64 环绕而非报错。
  # 端口最大值 65535 是 5 位数,所以字符串超过 5 位时一律拒绝,防止环绕陷阱。
  [[ ${#1} -le 5 ]] || return 1
  (( 10#$1 >= 1 && 10#$1 <= 65535 ))
}

db_type_is_valid() {
  case "$1" in
    mysql|postgres|sqlite|dm) return 0 ;;
    *) return 1 ;;
  esac
}

# schema 名不得含双引号:它会被双引号包裹后拼进 SET SCHEMA,内嵌引号会截断
# 标识符。后端(dm crate)也会拒,但在提问处挡掉体验更好。
schema_is_valid() {
  [[ "$1" != *'"'* ]]
}

# 去掉首尾空白后非空
not_empty() {
  local s="${1#"${1%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  [[ -n "$s" ]]
}

# 去掉首尾空白并输出。给「trim 之后才能用」的场景(schema),其余字段沿用
# 原样搬运的老规矩,不要顺手往它们身上加。
trim_ws() {
  local s="${1#"${1%%[![:space:]]*}"}"
  printf '%s' "${s%"${s##*[![:space:]]}"}"
}

# ---------- TOML 原文处理 ----------
#
# 设计要点(见 spec「显示与写入走两条独立路径」):
#   提取 = awk 取 = 右侧原文,不解析
#   显示 = 去引号 + 反转义,只用于打印 [默认值]
#   写入 = 回车不改则原文原样搬运;输入新值则 toml_quote
# 原样搬运是恒等变换,因为 answers.toml 与 config.toml 用同一种 TOML 基本
# 字符串转义。不解析就不会解析错。

# TOML 基本字符串转义。顺序关键:必须先转反斜杠,否则后转的 \" 里那个
# 反斜杠会被二次转义。
toml_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  printf '%s' "$s"
}

# 转义并加上外层引号 = 一个完整的 TOML 字符串字面量(即「原文」)
toml_quote() {
  printf '"%s"' "$(toml_escape "$1")"
}

# 反转义。顺序关键:先还原 \" 再还原 \\,反过来会把 \\" 错解成 \ + "。
#
# ⚠ 只能用于显示。写入路径永远走原样搬运或 toml_quote,不经过本函数 ——
#   这样即使本函数有瑕疵也污染不到配置文件。
toml_unescape_for_display() {
  local s="$1"
  s="${s//\\\"/\"}"
  s="${s//\\\\/\\}"
  printf '%s' "$s"
}

# 取出 <section>.<key> 的原文(= 右侧,含引号,不解析)。找不到输出空串。
#
# 限制:不处理行尾注释(如 `port = 3000 # 注释`)。config.toml.tmpl 的注释
# 都在独立行上,不会触发。即便触发,原样搬进 answers.toml 后 TOML 解析器
# 也会正确忽略尾注释,只有显示会略难看。
toml_raw_value() {
  local file="$1" section="$2" key="$3"
  [[ -f "$file" ]] || return 0
  awk -v sect="$section" -v k="$key" '
    {
      line = $0
      sub(/^[ \t]+/, "", line)
      sub(/[ \t]+$/, "", line)
    }
    line ~ /^\[/ {
      cur = line
      sub(/^\[[ \t]*/, "", cur)
      sub(/[ \t]*\].*$/, "", cur)
      next
    }
    line ~ /^#/ { next }
    line == ""  { next }
    cur == sect {
      eq = index(line, "=")
      if (eq == 0) next
      name = substr(line, 1, eq - 1)
      sub(/[ \t]+$/, "", name)
      if (name != k) next
      val = substr(line, eq + 1)
      sub(/^[ \t]+/, "", val)
      print val
      exit
    }
  ' "$file"
}

# 原文 → 可显示的默认值。仅用于打印,见 toml_unescape_for_display 的警告。
toml_display_value() {
  local raw="$1"
  if [[ ${#raw} -ge 2 && "${raw:0:1}" == '"' && "${raw: -1}" == '"' ]]; then
    toml_unescape_for_display "${raw:1:${#raw}-2}"
  else
    printf '%s' "$raw"
  fi
}

# ---------- answers.toml ----------

# 按库类型把无意义的字段归一。注意 answers.toml 的 port 必须是合法整数,
# 否则后端 Answers 反序列化直接失败 —— 这是本函数存在的硬理由,不只是整洁。
normalize_db_fields() {
  case "$DB_TYPE" in
    sqlite)
      # sqlite 只用「库名」当文件路径,其余字段无意义
      DB_URL_RAW='""'
      DB_PORT_RAW='0'
      DB_USERNAME_RAW='""'
      DB_PASSWORD_RAW='""'
      DB_SCHEMA_RAW='""'
      ;;
    dm)
      # 达梦走 ODBC:SERVER 与 TCP_PORT 由已注册的数据源提供,这两个字段无处可用。
      # db_name(数据源名)/ username / password / schema 都要保留。
      DB_URL_RAW='""'
      DB_PORT_RAW='0'
      ;;
    mysql)
      # mysql 不认 schema;切库类型后必须清掉,否则残留值语义错乱
      DB_SCHEMA_RAW='""'
      ;;
    postgres)
      # 全部字段都有意义(schema = search_path)
      :
      ;;
  esac
}

# 把收集到的原文值拼成 answers.toml。
# 所有 *_RAW 已是 TOML 字面量(字符串带引号且已转义,整数裸写),这里只拼装,
# 不再做任何转义 —— 二次转义会改写密码。
write_answers() {
  local out="$1"
  cat > "$out" <<EOF
[server]
port = $PORT_RAW

[db]
db_type = "$DB_TYPE"
url = $DB_URL_RAW
port = $DB_PORT_RAW
db_name = $DB_NAME_RAW
username = $DB_USERNAME_RAW
password = $DB_PASSWORD_RAW
schema = $DB_SCHEMA_RAW

[redis]
url = $REDIS_URL_RAW
port = $REDIS_PORT_RAW
password = $REDIS_PASSWORD_RAW
db = $REDIS_DB_RAW
EOF
}

# ---------- 提问原语 ----------
#
# read -p 的提示写到 stderr(bash 行为),所以单测捕获 stdout 时看不到提示。

# ask <变量名> <提示> <默认值> [校验函数] [错误提示]
# 回车 = 接受默认值。校验失败原地重问,不往下走。EOF 立即中止。
ask() {
  local __var="$1" __prompt="$2" __default="$3"
  local __validator="${4:-}" __errmsg="${5:-输入不合法,请重新输入}"
  local __input
  while true; do
    # read 在 EOF 时返回非零。不用 || true 而是显式检查,确保 EOF 立即中止。
    if [[ -n "$__default" ]]; then
      if ! read -r -p "$__prompt [$__default]: " __input; then
        printf '输入意外结束(EOF),安装中止。\n' >&2
        exit 1
      fi
    else
      if ! read -r -p "$__prompt: " __input; then
        printf '输入意外结束(EOF),安装中止。\n' >&2
        exit 1
      fi
    fi
    [[ -n "$__input" ]] || __input="$__default"
    if [[ -n "$__validator" ]] && ! "$__validator" "$__input"; then
      printf '  ✗ %s\n' "$__errmsg" >&2
      continue
    fi
    printf -v "$__var" '%s' "$__input"
    return 0
  done
}

# ask_secret <变量名> <提示>
# 不回显。留空的语义由调用方决定(升级时留空 = 保持原值)。EOF 立即中止。
ask_secret() {
  local __var="$1" __prompt="$2" __input
  if ! read -r -s -p "$__prompt: " __input; then
    printf '\n' >&2
    printf '输入意外结束(EOF),安装中止。\n' >&2
    exit 1
  fi
  printf '\n' >&2
  printf -v "$__var" '%s' "$__input"
}

# ---------- 后端二进制调用 ----------
#
# 连通性检查在 apply 之前,二进制还在 PAYLOAD_DIR(脚本目录)。
# apply 之后不再调后端。
#
# 本二进制动态链接 unixODBC(libodbc.so.2)—— dm crate 是 vendored sea-orm 的
# 无条件依赖,与实际用哪种数据库无关。达梦驱动由驱动管理器按 odbcinst.ini
# 定位,故这里仍不需要设 LD_LIBRARY_PATH。
run_backend() {
  "$PAYLOAD_DIR/$REPO_NAME" "$@"
}

# 二进制起不来时的人话翻译。
#
# dm crate 是 vendored sea-orm 的无条件依赖,odbc-sys 又动态链接驱动管理器 ——
# 所以**任何**库类型的部署都要求本机有 unixODBC 运行库,与客户用哪种数据库无关。
# 命中返回 0(并打印),否则返回 1。
translate_backend_error() {
  local err="$1"
  if [[ "$err" == *"error while loading shared libraries"* \
        && "$err" == *"libodbc"* ]]; then
    printf '本机缺少 unixODBC 运行库(libodbc.so.2),后端二进制无法启动。\n'
    printf '请先安装:yum install unixODBC   或   apt install unixodbc\n'
    return 0
  fi
  return 1
}

# 生成或外科更新 config.toml。
# 返回 0 成功 / 2 二进制过旧(不含安装器子命令) / 1 其他失败。
write_config_via_backend() {
  local cfg="$1" answers="$2" err rc=0
  err="$(run_backend --write-config "$cfg" --answers "$answers" 2>&1 >/dev/null)" || rc=$?
  (( rc == 0 )) && return 0

  # 二进制过旧时给人话错误,而不是把 Rust 的原始报错甩给用户
  if [[ "$err" == *"未知参数"* ]]; then
    printf '本包的后端版本过旧,不支持交互式安装(缺 --write-config 子命令)。\n' >&2
    printf '请改用支持交互式安装的安装包。\n' >&2
    return 2
  fi
  # 缺运行库属不可恢复,重问参数没用 —— 与「二进制过旧」同样返回 2
  if translate_backend_error "$err" >&2; then
    return 2
  fi
  printf '%s\n' "$err" >&2
  return 1
}

# 实测连一次 DB 和 Redis。错误原文走 stdout 返回给调用方打印,退出码透传。
check_config_via_backend() {
  local cfg="$1" err rc=0
  err="$(run_backend --check-config "$cfg" 2>&1 >/dev/null)" || rc=$?
  printf '%s' "$err"
  return $rc
}

# ---------- 服务名与升级检测 ----------

svc_name() {
  printf '%s%s' "$REPO_NAME" "${1:+-$1}"
}

svc_unit_file() {
  printf '%s/%s.service' "$SYSTEMD_DIR" "$(svc_name "$1")"
}

# 已注册实例的 WorkingDirectory。unit 不存在或没有该字段则输出空串。
unit_working_dir() {
  local unit="$1"
  [[ -f "$unit" ]] || return 0
  awk -F= '/^WorkingDirectory=/ {print $2; exit}' "$unit"
}

# unit 里 [Service] 段的 User= / Group=。读不到输出空串。
# 升级时据此预填服务账号,避免把用户原本用的账号悄悄改回默认值。
unit_user() {
  local unit="$1"
  [[ -f "$unit" ]] || return 0
  awk -F= '/^[[:space:]]*User=/ {print $2; exit}' "$unit"
}

unit_group() {
  local unit="$1"
  [[ -f "$unit" ]] || return 0
  awk -F= '/^[[:space:]]*Group=/ {print $2; exit}' "$unit"
}

# 列出所有已注册实例,每行 "<服务名>\t<WorkingDirectory>"
list_instances() {
  local unit name wd
  for unit in "$SYSTEMD_DIR"/${REPO_NAME}*.service; do
    # 无匹配时 glob 原样返回字面量,-f 挡掉(不用 nullglob,那会改全局 shell 选项)
    [[ -f "$unit" ]] || continue
    name="$(basename "$unit" .service)"
    wd="$(unit_working_dir "$unit")"
    printf '%s\t%s\n' "$name" "${wd:-(未知)}"
  done | sort
}

# ---------- 环境探测(逐级降级) ----------
#
# 精简版 openEuler / RHEL 常常没有 net-tools,甚至没有 curl。
# 探测不到工具时一律按「未占用 / 无法判定」处理并提示,绝不能因为探不到
# 就把安装堵死。

port_probe_tool() {
  if command -v ss >/dev/null 2>&1; then
    printf 'ss'
  elif command -v netstat >/dev/null 2>&1; then
    printf 'netstat'
  fi
}

# 0 = 确实被监听;1 = 未被监听,或没有探测工具
port_in_use() {
  local port="$1" tool
  tool="$(port_probe_tool)"
  case "$tool" in
    ss)      ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]${port}\$" ;;
    netstat) netstat -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]${port}\$" ;;
    *)       return 1 ;;
  esac
}

http_probe_tool() {
  if command -v curl >/dev/null 2>&1; then
    printf 'curl'
  elif command -v wget >/dev/null 2>&1; then
    printf 'wget'
  fi
}

# 探后端自己的 /api/health 比探 TCP 准:端口开着不代表路由就绪。
# 没有 HTTP 工具时退化成 TCP 端口探测(bash 内建 /dev/tcp,无外部依赖)。
health_ok() {
  local port="$1" tool
  tool="$(http_probe_tool)"
  case "$tool" in
    curl) curl -s -o /dev/null --max-time 2 "http://localhost:${port}/api/health" ;;
    wget) wget -q -O /dev/null --timeout=2 "http://localhost:${port}/api/health" ;;
    *)    (exec 3<>"/dev/tcp/127.0.0.1/${port}") 2>/dev/null ;;
  esac
}

# ---------- 全局状态 ----------
SUFFIX=""
SVC_USER="app"
SVC_GROUP="app"
SVC_NAME=""
SVC_FILE=""
IS_UPGRADE=false
OLD_PREFIX=""
TMP_DIR=""

# 回滚用
CREATED_DIRS=()
CREATED_FILES=()
SERVICE_REGISTERED=false
CONFIG_WRITTEN=false
OLD_SERVICE_STOPPED=false
CONFIG_BACKUP=""
# 升级时 apply 会用 sed 原地覆盖 SVC_FILE,失败回滚得有原件可恢复。
# (全新安装没有「旧 unit」,此值为空)
OLD_UNIT_BACKUP=""

# 答案(TOML 原文形式)。默认值对齐 config.toml.example。
PORT_RAW='3000'
DB_TYPE='mysql'
DB_URL_RAW='"127.0.0.1"'
DB_PORT_RAW='3306'
DB_NAME_RAW='"solidaxum"'
DB_USERNAME_RAW='"root"'
DB_PASSWORD_RAW='""'
DB_SCHEMA_RAW='""'
REDIS_URL_RAW='"localhost"'
REDIS_PORT_RAW='6379'
REDIS_PASSWORD_RAW='""'
REDIS_DB_RAW='10'

# ---------- 预检 ----------
preflight() {
  echo "[1/6] 预检..."

  [[ $EUID -eq 0 ]] || { echo "需要 root 权限,请 sudo 运行" >&2; exit 1; }

  # 只做交互式,无 TTY 直接报错,不挂在读输入上
  if [[ ! -t 0 || ! -t 1 ]]; then
    echo "本安装程序需要交互式终端(TTY)。" >&2
    echo "检测到标准输入或输出不是终端 —— 请在终端里直接运行 sudo ./install.sh," >&2
    echo "不要通过管道、重定向或无 TTY 的自动化工具调用。" >&2
    exit 1
  fi

  command -v systemctl >/dev/null || { echo "未检测到 systemd" >&2; exit 1; }

  [[ -f "$PAYLOAD_DIR/$REPO_NAME" ]] || { echo "未找到 $PAYLOAD_DIR/$REPO_NAME" >&2; exit 1; }
  [[ -f "$PAYLOAD_DIR/templates/systemd/${REPO_NAME}.service" ]] \
    || { echo "未找到 systemd 模板" >&2; exit 1; }
  [[ -d "$PAYLOAD_DIR/web" ]] || { echo "未找到 web/(请用含前端的完整包)" >&2; exit 1; }
}

# ---------- 回滚 ----------
cleanup_tmp() {
  if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
}

cleanup_on_fail() {
  local rc=$?
  # 回滚本身必须"尽力而为、逐步推进":某一步失败(比如只读文件系统导致
  # rm 失败)不能连累后面几步不执行,尤其是恢复旧服务那一步。set -e 对
  # trap handler 同样生效,不显式关掉的话,回滚可能自己也会被 errexit 中途打断。
  set +e
  echo "" >&2
  echo "✗ 安装失败 (exit code $rc),开始回滚..." >&2

  if [[ "$SERVICE_REGISTERED" == "true" ]]; then
    if [[ "$IS_UPGRADE" == "true" ]]; then
      # 升级路径:SVC_FILE 是用户原有的 unit,apply 用 sed 原地覆盖过它。
      # 删不得 —— 删了用户的升级前服务就彻底没了。恢复备份的原件即可。
      # 不需要重新 enable:升级路径全程没 disable 过,enable 符号链接一直指向
      # SVC_FILE,把内容换回来就回到了升级前的状态,下面的 OLD_SERVICE_STOPPED
      # 分支能直接把它拉起来。
      if [[ -n "$OLD_UNIT_BACKUP" && -f "$OLD_UNIT_BACKUP" ]]; then
        cp "$OLD_UNIT_BACKUP" "$SVC_FILE"
        systemctl daemon-reload 2>/dev/null || true
        echo "  已恢复升级前的 systemd unit" >&2
      else
        echo "  ⚠ 无升级前 unit 的备份,无法自动恢复 $SVC_FILE" >&2
      fi
    else
      # 全新安装:这个 unit 是本次新建的,直接撤掉。
      systemctl disable "$SVC_NAME" 2>/dev/null || true
      rm -f "$SVC_FILE"
      systemctl daemon-reload 2>/dev/null || true
    fi
  fi

  # 全新安装:删掉刚写的 config.toml。
  # 升级路径下绝不删 —— 那是用户的现有配置。
  if [[ "$CONFIG_WRITTEN" == "true" && "$IS_UPGRADE" != "true" ]]; then
    rm -f "$PREFIX/config.toml"
  fi

  local f d
  for f in ${CREATED_FILES[@]+"${CREATED_FILES[@]}"}; do rm -f "$f"; done
  for d in ${CREATED_DIRS[@]+"${CREATED_DIRS[@]}"}; do rmdir "$d" 2>/dev/null || true; done

  # 升级时停过旧服务而后续失败:把它拉回来
  if [[ "$OLD_SERVICE_STOPPED" == "true" ]]; then
    echo "正在恢复旧服务 $SVC_NAME ..." >&2
    systemctl start "$SVC_NAME" 2>/dev/null \
      || echo "  ⚠ 旧服务未能自动恢复,请手工 systemctl start $SVC_NAME" >&2
  fi

  if [[ -n "$CONFIG_BACKUP" ]]; then
    echo "  现有配置的备份仍在: $CONFIG_BACKUP" >&2
  fi

  # 本次新建的系统用户 / 组刻意不删:删一个可能已拥有其他文件的系统账号,
  # 比留下一个闲置账号危险得多。
  echo "✗ 回滚完成,请检查错误信息后重试" >&2

  # 必须显式 exit:上面的 set +e 是进程级开关,trap 返回后依然生效,
  # 若不在这里显式终止,errexit 原本"trap 跑完就收尾退出"的隐式行为会
  # 被一并关掉 —— main() 会带着 $SVC_FILE 已被删、config.toml 已被删这类
  # 半途状态继续往下跑完剩下的步骤,比"根本没回滚"更危险。EXIT trap
  # (cleanup_tmp)仍会照常触发,临时目录不会漏清。
  exit "$rc"
}

# ---------- 提问编排 ----------
ask_suffix() {
  echo ""
  echo "── 实例后缀 ──────────────────────────────────"
  echo "正式环境请直接回车留空。"
  echo "同一台机器上装多套(测试环境按客户区分)时填一个标识,"
  echo "只允许小写字母、数字、连字符。后缀决定服务名,装好后不可更改。"
  ask SUFFIX "后缀" "" suffix_is_valid "后缀只能包含小写字母、数字和连字符"

  SVC_NAME="$(svc_name "$SUFFIX")"
  SVC_FILE="$(svc_unit_file "$SUFFIX")"

  echo "  服务名将自动定为: $SVC_NAME"
}

ask_service_identity() {
  echo ""
  echo "── 服务账号 ──────────────────────────────────"
  echo "服务以该系统账号身份运行,不存在则自动创建(--system,不可登录)。"
  ask SVC_USER  "服务用户" "$SVC_USER"  not_empty "服务用户不能为空"
  ask SVC_GROUP "服务组"   "$SVC_GROUP" not_empty "服务组不能为空"
}

ask_port() {
  local port
  echo ""
  echo "── 服务端口 ──────────────────────────────────"
  echo "同机装多个实例时,各实例端口必须错开。"
  while true; do
    ask port "端口" "$(toml_display_value "$PORT_RAW")" port_is_valid \
      "端口必须是 1-65535 之间的数字"
    # 升级时跳过占用探测:旧服务正监听本端口是预期的,apply 会先停它
    if [[ "$IS_UPGRADE" != "true" ]] && port_in_use "$port"; then
      echo "  ✗ 端口 $port 已被占用,请换一个(同机多实例必须错开)。" >&2
      echo "    若刚卸载过旧实例,稍等几秒让端口释放后重试。" >&2
      continue
    fi
    break
  done
  PORT_RAW="$port"
}

ask_db() {
  local v secret
  echo ""
  echo "── 数据库 ────────────────────────────────────"
  echo "请填一个已存在的空库,以及对它有读写权限的账号。"

  ask DB_TYPE "类型 (mysql/postgres/sqlite/dm)" "$DB_TYPE" db_type_is_valid \
    "只能是 mysql、postgres、sqlite 或 dm"

  if [[ "$DB_TYPE" == "sqlite" ]]; then
    echo "  sqlite 模式:只需填数据库文件路径。"
    ask v "数据库文件路径" "$(toml_display_value "$DB_NAME_RAW")" not_empty \
      "路径不能为空"
    DB_NAME_RAW="$(toml_quote "$v")"
    normalize_db_fields
    return 0
  fi

  if [[ "$DB_TYPE" == "dm" ]]; then
    echo "  达梦走 ODBC,连接参数取自系统里已注册的数据源,安装程序无法代建。"
    echo "  前置条件(缺一不可):"
    echo "    · 达梦 ODBC 驱动已注册到 /etc/odbcinst.ini"
    echo "    · 数据源已注册到 /etc/odbc.ini,并配好 SERVER / TCP_PORT"
    echo "    · 建表脚本 solidaxum_init.sql 已在目标 schema 执行过"
    echo "      (脚本在源码仓库 sql/dm/ 目录,不随本安装包交付)"
    echo "  已注册的数据源可用 odbcinst -q -s 列出。"

    ask v "ODBC 数据源名(不是库名)" "$(toml_display_value "$DB_NAME_RAW")" \
      not_empty "数据源名不能为空"
    DB_NAME_RAW="$(toml_quote "$v")"

    ask v "用户名" "$(toml_display_value "$DB_USERNAME_RAW")" not_empty \
      "用户名不能为空"
    DB_USERNAME_RAW="$(toml_quote "$v")"

    ask_password DB_PASSWORD_RAW "密码"

    echo "  schema:裸表名在哪个 schema 下解析。schema 即建表账号名(如 RUOYI)。"
    echo "  用建表账号本身登录可留空;登录账号与建表账号不同则必须填。"
    ask_schema DB_SCHEMA_RAW "schema(可留空,输入 - 清空已有值)"

    normalize_db_fields
    return 0
  fi

  # postgres 的默认端口与 mysql 不同,仅在用户还没改过时替换默认值
  if [[ "$DB_TYPE" == "postgres" && "$DB_PORT_RAW" == "3306" ]]; then
    DB_PORT_RAW='5432'
  fi

  ask v "地址" "$(toml_display_value "$DB_URL_RAW")" not_empty "地址不能为空"
  DB_URL_RAW="$(toml_quote "$v")"

  ask v "端口" "$(toml_display_value "$DB_PORT_RAW")" port_is_valid \
    "端口必须是 1-65535 之间的数字"
  DB_PORT_RAW="$v"

  ask v "库名" "$(toml_display_value "$DB_NAME_RAW")" not_empty "库名不能为空"
  DB_NAME_RAW="$(toml_quote "$v")"

  ask v "用户名" "$(toml_display_value "$DB_USERNAME_RAW")" not_empty \
    "用户名不能为空"
  DB_USERNAME_RAW="$(toml_quote "$v")"

  ask_password DB_PASSWORD_RAW "密码"

  # PG 的 schema = search_path。共库分 schema 部署时用得上,留空则用 PG 默认。
  if [[ "$DB_TYPE" == "postgres" ]]; then
    ask_schema DB_SCHEMA_RAW \
      "schema(等价于 search_path,可留空,输入 - 清空已有值)"
  fi

  normalize_db_fields
}

ask_redis() {
  local v
  echo ""
  echo "── Redis ─────────────────────────────────────"
  echo "同机多实例时,各实例的「库编号」必须错开,否则会话与缓存会串。"

  ask v "地址" "$(toml_display_value "$REDIS_URL_RAW")" not_empty "地址不能为空"
  REDIS_URL_RAW="$(toml_quote "$v")"

  ask v "端口" "$(toml_display_value "$REDIS_PORT_RAW")" port_is_valid \
    "端口必须是 1-65535 之间的数字"
  REDIS_PORT_RAW="$v"

  ask_password REDIS_PASSWORD_RAW "密码(无则直接回车)"

  ask v "库编号" "$(toml_display_value "$REDIS_DB_RAW")" is_uint \
    "库编号必须是非负整数"
  REDIS_DB_RAW="$v"
}

# 密码提问。升级时留空 = 保持原值(原文原样保留,不经过任何解析)。
ask_password() {
  local __var="$1" prompt="$2" input
  if [[ "$IS_UPGRADE" == "true" ]]; then
    prompt="$prompt(回车保持不变)"
  fi
  ask_secret input "$prompt"
  if [[ -z "$input" && "$IS_UPGRADE" == "true" ]]; then
    return 0   # 保持 *_RAW 里已预填的原文不动
  fi
  printf -v "$__var" '%s' "$(toml_quote "$input")"
}

# schema 提问。dm 与 postgres 共用。两件事必须在这里做掉:
#
#  1. **trim**。不 trim 的话「打个空格当清空」会写出 schema = " " —— 那是个
#     非空字符串,后端会当成有效配置下发。PG 的 `SET search_path = " "` 不报错
#     (PG 只在解析对象时才报),于是 --check-config 通过、服务也起得来,之后
#     每条裸表名查询才炸。后端 dsn::schema_search_path 也 trim,两头都堵。
#
#  2. **`-` 哨兵**。ask 的语义是「空输入 = 取默认值」,升级时 schema 已从旧
#     config 预填,回车只会把旧值原样留下 —— 没有任何输入能表达「改回不设置」。
#     达梦那边「改用建表账号本人登录」是文档记载的正常解法,必须给得出这个入口。
ask_schema() {
  local __var="$1" __prompt="$2" v
  ask v "$__prompt" "$(toml_display_value "${!__var}")" schema_is_valid \
    "schema 名不能包含双引号"
  v="$(trim_ws "$v")"
  if [[ "$v" == "-" ]]; then
    v=""
  fi
  printf -v "$__var" '%s' "$(toml_quote "$v")"
}

ask_all() {
  echo "[2/6] 收集配置..."
  ask_service_identity
  ask_port
  ask_db
  ask_redis
}

# ---------- 配置生成与实测 ----------
# 检查不通过则回到数据库 / Redis 提问重来,不允许跳过。
generate_and_verify_config() {
  local answers cfg err rc
  TMP_DIR="$(mktemp -d)"
  chmod 700 "$TMP_DIR"
  answers="$TMP_DIR/answers.toml"
  cfg="$TMP_DIR/config.toml"

  while true; do
    echo ""
    echo "[3/6] 生成配置并实测连接..."

    normalize_db_fields
    write_answers "$answers"
    chmod 600 "$answers"

    # 升级:以现有 config.toml 为起点,--write-config 只做外科更新,
    # security.aes_key / jwt.secret 与手改的高级项因此得以保留。
    rm -f "$cfg"
    if [[ "$IS_UPGRADE" == "true" && -f "$OLD_PREFIX/config.toml" ]]; then
      cp "$OLD_PREFIX/config.toml" "$cfg"
    fi

    rc=0
    write_config_via_backend "$cfg" "$answers" || rc=$?
    if (( rc == 2 )); then
      exit 1   # 二进制过旧,已打印人话错误,重试也没用
    elif (( rc != 0 )); then
      echo "  ✗ 生成配置失败,请检查上面的错误。" >&2
      ask_db
      ask_redis
      continue
    fi

    rc=0
    err="$(check_config_via_backend "$cfg")" || rc=$?
    if (( rc == 0 )); then
      echo "  ✓ 数据库与 Redis 均连接成功"
      return 0
    fi

    echo "" >&2
    echo "  ✗ 连接检查未通过:" >&2
    printf '%s\n' "$err" | sed 's/^/    /' >&2
    echo "" >&2
    echo "  请修正后重填(想放弃按 Ctrl-C,此时尚未改动任何系统配置)。" >&2
    ask_db
    ask_redis
  done
}

# ---------- 摘要确认 ----------
confirm_summary() {
  local answer schema_disp
  echo ""
  echo "[4/6] 请确认以下配置"
  echo "──────────────────────────────────────────────"
  printf '  安装目录    : %s\n'   "$PREFIX"
  printf '  服务名      : %s\n'   "$SVC_NAME"
  printf '  服务账号    : %s:%s\n' "$SVC_USER" "$SVC_GROUP"
  printf '  监听端口    : %s\n'   "$(toml_display_value "$PORT_RAW")"
  printf '  数据库类型  : %s\n'   "$DB_TYPE"
  if [[ "$DB_TYPE" == "sqlite" ]]; then
    printf '  数据库文件  : %s\n' "$(toml_display_value "$DB_NAME_RAW")"
  elif [[ "$DB_TYPE" == "dm" ]]; then
    printf '  ODBC 数据源 : %s\n' "$(toml_display_value "$DB_NAME_RAW")"
    printf '  数据库用户  : %s(密码 ********)\n' \
      "$(toml_display_value "$DB_USERNAME_RAW")"
  else
    printf '  数据库      : %s:%s/%s\n' \
      "$(toml_display_value "$DB_URL_RAW")" \
      "$(toml_display_value "$DB_PORT_RAW")" \
      "$(toml_display_value "$DB_NAME_RAW")"
    printf '  数据库用户  : %s(密码 ********)\n' \
      "$(toml_display_value "$DB_USERNAME_RAW")"
  fi
  schema_disp="$(toml_display_value "$DB_SCHEMA_RAW")"
  # ⚠ 必须用 if 块:本脚本是 set -Eeuo pipefail + ERR trap,
  #   以失败的 [[ ]] 测试收尾会触发 cleanup_on_fail 回滚。
  if [[ -n "$schema_disp" ]]; then
    printf '  schema      : %s\n' "$schema_disp"
  fi
  printf '  Redis       : %s:%s db=%s(密码 ********)\n' \
    "$(toml_display_value "$REDIS_URL_RAW")" \
    "$(toml_display_value "$REDIS_PORT_RAW")" \
    "$(toml_display_value "$REDIS_DB_RAW")"
  if [[ "$IS_UPGRADE" == "true" ]]; then
    printf '  模式        : 升级(现有 security.aes_key 与 jwt.secret 保持不变)\n'
  else
    printf '  模式        : 全新安装(自动生成 jwt.secret 与 security.aes_key)\n'
  fi
  echo "──────────────────────────────────────────────"

  ask answer "确认无误并开始安装? (yes/no)" "yes"
  case "$answer" in
    y|Y|yes|YES) return 0 ;;
    *) echo "已取消,未改动任何系统配置。"; exit 0 ;;
  esac
}

# ---------- 落地 ----------
apply() {
  echo ""
  echo "[5/6] 安装..."

  # 服务用户必须先于任何 chown 存在
  if ! getent passwd "$SVC_USER" >/dev/null; then
    echo "  创建系统用户 $SVC_USER"
    useradd --system --no-create-home --shell /usr/sbin/nologin "$SVC_USER"
  fi
  getent group "$SVC_GROUP" >/dev/null || groupadd --system "$SVC_GROUP"

  # 落配置(测通的那一份原样搬过来)
  cp "$TMP_DIR/config.toml" "$PREFIX/config.toml"
  CONFIG_WRITTEN=true
  chown "$SVC_USER:$SVC_GROUP" "$PREFIX/config.toml"
  chmod 600 "$PREFIX/config.toml"

  # 运行目录与权限
  chmod a+rx "$PREFIX"
  chmod 755 "$PREFIX/$REPO_NAME"

  local d
  for d in data logs; do
    if [[ ! -d "$PREFIX/$d" ]]; then
      install -d -m 750 -o "$SVC_USER" -g "$SVC_GROUP" "$PREFIX/$d"
      CREATED_DIRS+=("$PREFIX/$d")
    else
      chown "$SVC_USER:$SVC_GROUP" "$PREFIX/$d"
    fi
  done

  chown -R "$SVC_USER:$SVC_GROUP" "$PREFIX/web"

  # 渲染并安装 systemd unit
  echo "  注册 systemd 服务 $SVC_NAME"
  # 先登记再落地,顺序不能反:下面这行的 > 重定向在 sed 本身跑起来之前就会
  # 创建/截断目标文件,一旦 sed 或紧跟着的 chmod 失败,文件已经落地但还没
  # 登记,回滚的通用清理循环就漏了它。
  # 只在非升级路径登记:升级时 SVC_FILE 是用户原有的 unit,不是本次新建,
  # 回滚不该删它 —— 与 config.toml 的 CONFIG_WRITTEN && IS_UPGRADE != true
  # 是同一道理。
  if [[ "$IS_UPGRADE" != "true" ]]; then
    CREATED_FILES+=("$SVC_FILE")
  else
    # 升级:SVC_FILE 是用户原有的 unit,下面 sed 会原地覆盖它。
    # 先把原件存到 TMP_DIR,失败回滚(cleanup_on_fail)靠它恢复 ——
    # 没有原件就无法把服务还原到升级前的状态。
    cp "$SVC_FILE" "$TMP_DIR/old-unit.service"
    OLD_UNIT_BACKUP="$TMP_DIR/old-unit.service"
  fi
  sed -e "s|@PREFIX@|$PREFIX|g" \
      -e "s|@USER@|$SVC_USER|g" \
      -e "s|@GROUP@|$SVC_GROUP|g" \
      -e "s|@SVC_NAME@|$SVC_NAME|g" \
      -e "s|@REPO_NAME@|$REPO_NAME|g" \
      "$PAYLOAD_DIR/templates/systemd/${REPO_NAME}.service" > "$SVC_FILE"
  chmod 644 "$SVC_FILE"

  systemctl daemon-reload
  systemctl enable "$SVC_NAME" >/dev/null
  SERVICE_REGISTERED=true
}

# ---------- 启动与探活 ----------
# 探活失败刻意不回滚:服务确实注册上了,只是没在 30 秒内就绪。
# 此时删掉服务反而让用户失去排查现场。
start_and_probe() {
  local i port
  port="$(toml_display_value "$PORT_RAW")"

  echo ""
  echo "[6/6] 启动服务..."
  systemctl start "$SVC_NAME"

  for i in $(seq 1 30); do
    if health_ok "$port"; then
      echo "  ✓ 服务已就绪"
      return 0
    fi
    sleep 1
  done

  echo ""
  echo "⚠ 服务 $SVC_NAME 已注册并启动,但 30 秒内未响应健康检查。" >&2
  echo "  配置已保存到 $PREFIX/config.toml" >&2
  echo "  排查:" >&2
  echo "    journalctl -u $SVC_NAME -n 100 --no-pager" >&2
  echo "    ls $PREFIX/logs/" >&2
  return 0
}

print_next_steps() {
  local port
  port="$(toml_display_value "$PORT_RAW")"
  echo ""
  echo "✓ 安装完成"
  echo "  服务目录    : $PREFIX"
  echo "  服务名      : $SVC_NAME"
  echo "  systemd unit: $SVC_FILE"
  echo "  访问地址    : http://localhost:${port}/"
  if [[ -n "$CONFIG_BACKUP" ]]; then
    echo "  配置备份    : $CONFIG_BACKUP"
  fi
  echo ""
  echo "  提示:config.toml 的 [server].host 默认为 localhost,只监听回环。"
  echo "  要从其他机器访问,改成 0.0.0.0 后 systemctl restart $SVC_NAME。"
  echo ""
  echo "  常用命令:"
  echo "    systemctl status $SVC_NAME"
  echo "    journalctl -u $SVC_NAME -f"
  echo "    sudo $PREFIX/uninstall.sh"
  echo ""
  echo "  ⚠ 不要 mv / rm 当前目录,systemd 用绝对路径引用它。"
  echo "    要换位置:先 sudo $PREFIX/uninstall.sh,再 mv,再 install.sh。"
}

# ---------- 升级检测与预填 ----------

# 判据:unit 文件是否存在。
detect_upgrade() {
  IS_UPGRADE=false
  OLD_PREFIX=""

  [[ -f "$SVC_FILE" ]] || return 0

  IS_UPGRADE=true
  OLD_PREFIX="$(unit_working_dir "$SVC_FILE")"

  # 从现有 unit 读服务账号作为 ask_service_identity 的默认值。
  # 不读的话,用户一路回车会把原本的账号悄悄改回默认 app,新 unit 的 User=
  # 与旧文件 owner 不符,服务起不来或读不到数据。
  # 放在 OLD_PREFIX 解析之前:即使 WorkingDirectory 解析不出来(下面那条
  # 「按全新配置处理」的分支),账号也要继承。
  local u g
  u="$(unit_user "$SVC_FILE")"
  g="$(unit_group "$SVC_FILE")"
  [[ -n "$u" ]] && SVC_USER="$u"
  [[ -n "$g" ]] && SVC_GROUP="$g"

  echo ""
  echo "检测到已注册的实例 $SVC_NAME"
  echo "  原服务目录: ${OLD_PREFIX:-(未知)}"

  if [[ -z "$OLD_PREFIX" ]]; then
    echo "  ⚠ 无法从 unit 文件解析出 WorkingDirectory,将按全新配置处理。" >&2
    echo "    现有的 security.aes_key 不会被继承 —— 若库里已有加密的 OSS 配置," >&2
    echo "    请先 Ctrl-C,手工备份好旧 config.toml 再继续。" >&2
    local go
    ask go "确认继续? (yes/no)" "no"
    case "$go" in y|Y|yes|YES) ;; *) echo "已取消。"; exit 0 ;; esac
    return 0
  fi

  # 换目录:原地安装模式下 PREFIX = 脚本目录,可能与原 unit 的不同
  if [[ "$OLD_PREFIX" != "$PREFIX" ]]; then
    echo ""
    echo "  ⚠ 本次安装目录与原目录不同:"
    echo "      原目录: $OLD_PREFIX"
    echo "      新目录: $PREFIX"
    echo "    继续将把 systemd 服务重新指向新目录。注意:"
    echo "      · 原目录的 data/ 不会跟过来"
    echo "      · 原目录的 config.toml 会被读作预填来源,"
    echo "        security.aes_key 由它延续(否则已加密的 OSS 配置会解不开)"
    local go
    ask go "确认继续? (yes/no)" "no"
    case "$go" in y|Y|yes|YES) ;; *) echo "已取消。"; exit 0 ;; esac
  fi

  prefill_from_existing
}

# 从现有 config.toml 预填。取的是原文(含引号,不解析) ——
# 用户回车不改时原样搬进 answers.toml,是恒等变换;
# 显示默认值时才做 best-effort 反转义。见 spec「显示与写入走两条独立路径」。
prefill_from_existing() {
  local cfg="$OLD_PREFIX/config.toml" v
  if [[ ! -f "$cfg" ]]; then
    echo "  ⚠ 原目录没有 config.toml,按全新配置提问。" >&2
    IS_UPGRADE=true   # 仍是升级(服务已注册),只是无从预填
    return 0
  fi

  echo "  已从 $cfg 读取现有配置作为默认值"

  v="$(toml_raw_value "$cfg" server port)";   [[ -n "$v" ]] && PORT_RAW="$v"
  v="$(toml_raw_value "$cfg" db db_type)"
  [[ -n "$v" ]] && DB_TYPE="$(toml_display_value "$v")"
  v="$(toml_raw_value "$cfg" db url)";        [[ -n "$v" ]] && DB_URL_RAW="$v"
  v="$(toml_raw_value "$cfg" db port)";       [[ -n "$v" ]] && DB_PORT_RAW="$v"
  v="$(toml_raw_value "$cfg" db db_name)";    [[ -n "$v" ]] && DB_NAME_RAW="$v"
  v="$(toml_raw_value "$cfg" db username)";   [[ -n "$v" ]] && DB_USERNAME_RAW="$v"
  v="$(toml_raw_value "$cfg" db password)";   [[ -n "$v" ]] && DB_PASSWORD_RAW="$v"
  v="$(toml_raw_value "$cfg" db schema)";     [[ -n "$v" ]] && DB_SCHEMA_RAW="$v"

  v="$(toml_raw_value "$cfg" redis url)";      [[ -n "$v" ]] && REDIS_URL_RAW="$v"
  v="$(toml_raw_value "$cfg" redis port)";     [[ -n "$v" ]] && REDIS_PORT_RAW="$v"
  v="$(toml_raw_value "$cfg" redis password)"; [[ -n "$v" ]] && REDIS_PASSWORD_RAW="$v"
  v="$(toml_raw_value "$cfg" redis db)";       [[ -n "$v" ]] && REDIS_DB_RAW="$v"

  return 0
}

# 落配置前:停旧服务并备份现有配置。
#
# 备份是无条件的,成功也保留 —— security.aes_key 一旦丢失,库里所有已加密的
# OSS 配置(access_key/secret_key)永久解不开。一次 cp 换这个保险很划算。
stop_old_service_and_backup() {
  [[ "$IS_UPGRADE" == "true" ]] || return 0

  echo ""
  echo "停止旧服务 $SVC_NAME ..."
  # Linux 允许覆盖运行中的可执行文件,不需要 Windows 那套轮询到 STOPPED 的逻辑。
  if systemctl is-active --quiet "$SVC_NAME"; then
    systemctl stop "$SVC_NAME"
    OLD_SERVICE_STOPPED=true
  fi

  local src="$OLD_PREFIX/config.toml"
  if [[ -f "$src" ]]; then
    CONFIG_BACKUP="${src}.bak-$(date +%Y%m%d%H%M%S)"
    cp -p "$src" "$CONFIG_BACKUP"
    echo "  现有配置已备份到 $CONFIG_BACKUP"
  fi
}

# ---------- 入口 ----------
main() {
  trap 'cleanup_on_fail' ERR
  trap 'cleanup_tmp' EXIT

  preflight
  ask_suffix
  detect_upgrade
  ask_all
  generate_and_verify_config
  confirm_summary
  stop_old_service_and_backup
  apply
  start_and_probe
  print_next_steps
}

# 被 source 时不执行(单测需要只拿函数)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
