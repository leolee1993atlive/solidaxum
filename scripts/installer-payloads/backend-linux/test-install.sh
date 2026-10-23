#!/usr/bin/env bash
# install.sh 的纯函数单测。
# 不需要 root、不碰 systemd、不需要后端二进制。
# 用法: ./test-install.sh
#
# ⚠ 只 source install.sh。uninstall.sh 有两个同名函数(刻意的副本),
#   同时 source 会让后加载的覆盖先加载的,测试就测错了对象。
#   uninstall.sh 由 test-uninstall.sh 单独测。

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=./test-lib.sh
source "$HERE/test-lib.sh"

# install.sh 顶部有 set -Eeuo pipefail;source 进来后测试要能跑过断言失败,
# 所以随即 set +e。ERR trap 只在 main() 内安装,不会被 source 带进来。
# shellcheck source=./install.sh
source "$HERE/install.sh"
set +e

# install.sh 是「原地安装」模式:PREFIX = 脚本所在目录,REPO_NAME = 其 basename。
# 测试在源码树的 backend-linux/ 里跑,basename 会得到 "backend-linux",与实测
# 场景(包目录叫 solidaxum)不符 —— 覆盖成真实包名。
# svc_name / list_instances / run_backend 都在调用时才读 REPO_NAME,覆盖即生效。
REPO_NAME="solidaxum"

# ---------- 后缀校验 ----------
section "suffix_is_valid"
assert_ok   "空后缀合法(正式环境)"        suffix_is_valid ""
assert_ok   "纯小写字母"                  suffix_is_valid "proj"
assert_ok   "小写字母+数字"               suffix_is_valid "proj1"
assert_ok   "含连字符"                    suffix_is_valid "proj-a-1"
assert_ok   "纯数字"                      suffix_is_valid "123"
assert_fail "大写字母不合法"              suffix_is_valid "Proj"
assert_fail "下划线不合法"                suffix_is_valid "proj_1"
assert_fail "空格不合法"                  suffix_is_valid "proj 1"
assert_fail "点号不合法"                  suffix_is_valid "proj.1"
assert_fail "斜杠不合法"                  suffix_is_valid "proj/1"
assert_fail "中文不合法"                  suffix_is_valid "项目一"

# ---------- 整数 / 端口 ----------
section "is_uint"
assert_ok   "0 是非负整数"          is_uint "0"
assert_ok   "3000 是非负整数"       is_uint "3000"
assert_fail "空串不是整数"          is_uint ""
assert_fail "字母不是整数"          is_uint "abc"
assert_fail "尾随字母不是整数"      is_uint "3000x"
assert_fail "负号不接受"            is_uint "-1"
assert_fail "前导空格不接受"        is_uint " 3000"
assert_fail "小数不接受"            is_uint "30.0"

section "port_is_valid"
assert_ok   "1 是合法端口"          port_is_valid "1"
assert_ok   "3000 是合法端口"       port_is_valid "3000"
assert_ok   "65535 是合法端口"      port_is_valid "65535"
assert_ok   "前导零按十进制解析"    port_is_valid "08080"
assert_fail "0 不是合法端口"        port_is_valid "0"
assert_fail "65536 越界"            port_is_valid "65536"
assert_fail "空串不是合法端口"      port_is_valid ""
assert_fail "非数字不是合法端口"    port_is_valid "abc"
assert_fail "6位数超出范围"         port_is_valid "999999"
assert_fail "2^64环绕陷阱防护"      port_is_valid "18446744073709559696"

section "db_type_is_valid"
assert_ok   "mysql"                 db_type_is_valid "mysql"
assert_ok   "postgres"              db_type_is_valid "postgres"
assert_ok   "sqlite"                db_type_is_valid "sqlite"
assert_fail "大写 MySQL 不接受"     db_type_is_valid "MySQL"
assert_fail "postgresql 不接受"     db_type_is_valid "postgresql"
assert_fail "空串不接受"            db_type_is_valid ""
assert_ok   "dm(达梦)"    db_type_is_valid "dm"
assert_fail "oracle 不支持" db_type_is_valid "oracle"

section "not_empty"
assert_ok   "非空串通过"            not_empty "x"
assert_fail "空串不通过"            not_empty ""
assert_fail "纯空格不通过"          not_empty "   "

# ---------- TOML 转义 ----------
section "toml_escape / toml_quote"
assert_eq "普通串不变"        'abc'         "$(toml_escape 'abc')"
assert_eq "反斜杠翻倍"        'a\\b'        "$(toml_escape 'a\b')"
assert_eq "引号加反斜杠"      'a\"b'        "$(toml_escape 'a"b')"
assert_eq "先转反斜杠再转引号" 'a\\\"b'     "$(toml_escape 'a\"b')"
assert_eq "toml_quote 加外层引号" '"abc"'   "$(toml_quote 'abc')"
assert_eq "toml_quote 转义内容"   '"a\\b"'  "$(toml_quote 'a\b')"

section "toml_unescape_for_display"
assert_eq "普通串不变"        'abc'         "$(toml_unescape_for_display 'abc')"
assert_eq "还原反斜杠"        'a\b'         "$(toml_unescape_for_display 'a\\b')"
assert_eq "还原引号"          'a"b'         "$(toml_unescape_for_display 'a\"b')"
assert_eq "还原混合"          'a\"b'        "$(toml_unescape_for_display 'a\\\"b')"

# 转义 → 反转义必须回到原点(.iss 注释里警告过的那个陷阱)
section "escape/unescape 互逆"
for s in 'abc' 'a\b' 'a"b' 'a\"b' '\\' '""' 'p@ss\w"rd' 'C:\path\to\x'; do
  assert_eq "互逆: $s" "$s" "$(toml_unescape_for_display "$(toml_escape "$s")")"
done

# ---------- TOML 原文提取 ----------
section "toml_raw_value"
TOML_FIXTURE="$(mktemp)"
cat > "$TOML_FIXTURE" <<'FIXTURE'
[server]
port = 3000
host = "localhost"

[db]
# 支持的 db_type: "mysql" | "postgres" | "sqlite"
db_type = "mysql"
url = "127.0.0.1"
port = 3306
db_name = "solidaxum"
username = "root"
password = "p@ss\\w\"rd"

[redis]
url = "localhost"
port = 6379
password = ""
db = 10

[jwt]
secret = "0123456789abcdef0123456789abcdef"

[security]
aes_key = "fedcba9876543210fedcba9876543210"
FIXTURE

assert_eq "取整数原文"          '3000'          "$(toml_raw_value "$TOML_FIXTURE" server port)"
assert_eq "取字符串原文含引号"  '"localhost"'   "$(toml_raw_value "$TOML_FIXTURE" server host)"
assert_eq "同名 key 按 section 区分" '3306'     "$(toml_raw_value "$TOML_FIXTURE" db port)"
assert_eq "redis 段的 port"     '6379'          "$(toml_raw_value "$TOML_FIXTURE" redis port)"
assert_eq "带转义的密码取原文"  '"p@ss\\w\"rd"' "$(toml_raw_value "$TOML_FIXTURE" db password)"
assert_eq "空字符串原文"        '""'            "$(toml_raw_value "$TOML_FIXTURE" redis password)"
assert_eq "不存在的 key 返回空" ''              "$(toml_raw_value "$TOML_FIXTURE" db nosuch)"
assert_eq "不存在的 section 返回空" ''          "$(toml_raw_value "$TOML_FIXTURE" nosuch port)"
assert_eq "注释行不被当成 key"  '"mysql"'       "$(toml_raw_value "$TOML_FIXTURE" db db_type)"
assert_eq "文件不存在返回空"    ''              "$(toml_raw_value /nonexistent/x.toml db port)"
# 升级保密钥的前提:这两个字段必须能被原样读出(实际由后端 toml_edit 保留,
# 这里验证提取器不会在它们上面出错)
assert_eq "jwt.secret 可提取" '"0123456789abcdef0123456789abcdef"' \
  "$(toml_raw_value "$TOML_FIXTURE" jwt secret)"
assert_eq "security.aes_key 可提取" '"fedcba9876543210fedcba9876543210"' \
  "$(toml_raw_value "$TOML_FIXTURE" security aes_key)"

section "toml_display_value"
assert_eq "去引号并反转义"  'p@ss\w"rd' "$(toml_display_value '"p@ss\\w\"rd"')"
assert_eq "整数原样"        '3000'      "$(toml_display_value '3000')"
assert_eq "空字符串"        ''          "$(toml_display_value '""')"

# 显示值再转义回去必须等于原文 —— 证明显示层没有信息损失,
# 也就证明了「回车不改」时原样搬运与「重新输入同样的值」结果一致。
section "显示→转义回原文"
for key in url db_name username password; do
  raw="$(toml_raw_value "$TOML_FIXTURE" db "$key")"
  assert_eq "显示→转义回原文: db.$key" "$raw" "$(toml_quote "$(toml_display_value "$raw")")"
done

rm -f "$TOML_FIXTURE"

# ---------- answers.toml 生成 ----------
section "write_answers (mysql)"
PORT_RAW='3000'
DB_TYPE='mysql'
DB_URL_RAW='"127.0.0.1"'
DB_PORT_RAW='3306'
DB_NAME_RAW='"solidaxum"'
DB_USERNAME_RAW='"root"'
DB_PASSWORD_RAW='"p@ss\\w\"rd"'
DB_SCHEMA_RAW='""'
REDIS_URL_RAW='"localhost"'
REDIS_PORT_RAW='6379'
REDIS_PASSWORD_RAW='""'
REDIS_DB_RAW='10'

ANSWERS_OUT="$(mktemp)"
write_answers "$ANSWERS_OUT"

assert_eq "server.port"    '3000'            "$(toml_raw_value "$ANSWERS_OUT" server port)"
assert_eq "db.db_type"     '"mysql"'         "$(toml_raw_value "$ANSWERS_OUT" db db_type)"
assert_eq "db.url"         '"127.0.0.1"'     "$(toml_raw_value "$ANSWERS_OUT" db url)"
assert_eq "db.port"        '3306'            "$(toml_raw_value "$ANSWERS_OUT" db port)"
assert_eq "db.db_name"     '"solidaxum"' "$(toml_raw_value "$ANSWERS_OUT" db db_name)"
assert_eq "db.username"    '"root"'          "$(toml_raw_value "$ANSWERS_OUT" db username)"
assert_eq "密码原文逐字节搬运" '"p@ss\\w\"rd"' "$(toml_raw_value "$ANSWERS_OUT" db password)"
assert_eq "redis.password 空串" '""'         "$(toml_raw_value "$ANSWERS_OUT" redis password)"
assert_eq "redis.db"       '10'              "$(toml_raw_value "$ANSWERS_OUT" redis db)"
assert_eq "db.schema 空串" '""' "$(toml_raw_value "$ANSWERS_OUT" db schema)"
rm -f "$ANSWERS_OUT"

section "write_answers (dm)"
PORT_RAW='3000'
DB_TYPE='dm'
DB_URL_RAW='""'
DB_PORT_RAW='0'
DB_NAME_RAW='"DM8"'
DB_USERNAME_RAW='"SYSDBA"'
DB_PASSWORD_RAW='"p@ss;word"'
DB_SCHEMA_RAW='"solidaxum-demo"'
REDIS_URL_RAW='"localhost"'
REDIS_PORT_RAW='6379'
REDIS_PASSWORD_RAW='""'
REDIS_DB_RAW='10'

ANSWERS_OUT="$(mktemp)"
write_answers "$ANSWERS_OUT"
assert_eq "dm: db_type"  '"dm"'   "$(toml_raw_value "$ANSWERS_OUT" db db_type)"
assert_eq "dm: 数据源名" '"DM8"'  "$(toml_raw_value "$ANSWERS_OUT" db db_name)"
assert_eq "dm: port 为 0" '0'     "$(toml_raw_value "$ANSWERS_OUT" db port)"
assert_eq "dm: 含分号的密码逐字节搬运" '"p@ss;word"' \
  "$(toml_raw_value "$ANSWERS_OUT" db password)"
assert_eq "dm: 带连字符的 schema 原样搬运" '"solidaxum-demo"' \
  "$(toml_raw_value "$ANSWERS_OUT" db schema)"
rm -f "$ANSWERS_OUT"

# ---------- schema 名校验 ----------
# 双引号会截断 SET SCHEMA 的标识符,dm crate 也会拒;在提问处就挡掉体验更好。
section "schema_is_valid"
assert_ok   "留空合法"        schema_is_valid ""
assert_ok   "常规名"          schema_is_valid "RUOYI"
assert_ok   "带连字符(实机存在)" schema_is_valid "solidaxum-demo"
assert_fail "含双引号不合法"  schema_is_valid 'a" ; DROP TABLE sys_user; --'

section "trim_ws"
assert_eq "首尾空白去掉"   "RUOYI" "$(trim_ws "  RUOYI  ")"
assert_eq "纯空白得空串"   ""      "$(trim_ws "   ")"
assert_eq "制表/换行也算空白" ""   "$(trim_ws "$(printf '\t\n')")"
assert_eq "中间空白保留"   "a b"   "$(trim_ws " a b ")"

# schema 提问原语。守两条:纯空白不能变成「有效 schema」(PG 的
# SET search_path = " " 不报错,检测能过、查询才炸);升级时要有办法把
# 已预填的 schema 改回不设置(ask 的空输入 = 取默认值,回车做不到)。
section "ask_schema"
DB_SCHEMA_RAW='"RUOYI"'
ask_schema DB_SCHEMA_RAW "schema" < <(printf '\n')
assert_eq "回车沿用已有值" '"RUOYI"' "$DB_SCHEMA_RAW"

DB_SCHEMA_RAW='"RUOYI"'
ask_schema DB_SCHEMA_RAW "schema" < <(printf -- '-\n')
assert_eq "输入 - 清空已有值" '""' "$DB_SCHEMA_RAW"

DB_SCHEMA_RAW='""'
ask_schema DB_SCHEMA_RAW "schema" < <(printf '   \n')
assert_eq "纯空格 trim 后视为未设置" '""' "$DB_SCHEMA_RAW"

DB_SCHEMA_RAW='""'
ask_schema DB_SCHEMA_RAW "schema" < <(printf '  RUOYI  \n')
assert_eq "首尾空白被剥掉" '"RUOYI"' "$DB_SCHEMA_RAW"

DB_SCHEMA_RAW='""'
ask_schema DB_SCHEMA_RAW "schema" < <(printf 'solidaxum-demo\n')
assert_eq "带连字符的名字原样保留" '"solidaxum-demo"' "$DB_SCHEMA_RAW"

# ---------- 按库类型归一 ----------
section "normalize_db_fields (sqlite)"
DB_TYPE='sqlite'
DB_URL_RAW='"127.0.0.1"'
DB_PORT_RAW=''
DB_USERNAME_RAW='"root"'
DB_PASSWORD_RAW='"x"'
DB_SCHEMA_RAW='"RUOYI"'
DB_NAME_RAW='"/var/lib/solidaxum/app.db"'
normalize_db_fields
assert_eq "sqlite: url 归一为空串" '""' "$DB_URL_RAW"
assert_eq "sqlite: port 补 0"      '0'  "$DB_PORT_RAW"
assert_eq "sqlite: username 空串"  '""' "$DB_USERNAME_RAW"
assert_eq "sqlite: password 空串"  '""' "$DB_PASSWORD_RAW"
assert_eq "sqlite: schema 清空"    '""' "$DB_SCHEMA_RAW"
assert_eq "sqlite: db_name 保留"   '"/var/lib/solidaxum/app.db"' "$DB_NAME_RAW"

section "normalize_db_fields (dm)"
DB_TYPE='dm'
DB_URL_RAW='"127.0.0.1"'
DB_PORT_RAW='3306'
DB_NAME_RAW='"DM8"'
DB_USERNAME_RAW='"SYSDBA"'
DB_PASSWORD_RAW='"x"'
DB_SCHEMA_RAW='"RUOYI"'
normalize_db_fields
assert_eq "dm: url 归一为空串(SERVER 由数据源提供)" '""' "$DB_URL_RAW"
assert_eq "dm: port 补 0"          '0'         "$DB_PORT_RAW"
assert_eq "dm: 数据源名保留"        '"DM8"'     "$DB_NAME_RAW"
assert_eq "dm: 用户名保留"          '"SYSDBA"'  "$DB_USERNAME_RAW"
assert_eq "dm: schema 保留"        '"RUOYI"'   "$DB_SCHEMA_RAW"

section "normalize_db_fields (mysql)"
DB_TYPE='mysql'
DB_URL_RAW='"127.0.0.1"'
DB_PORT_RAW='3306'
DB_SCHEMA_RAW='"RUOYI"'
normalize_db_fields
assert_eq "mysql: url 不改"    '"127.0.0.1"' "$DB_URL_RAW"
assert_eq "mysql: port 不改"   '3306'        "$DB_PORT_RAW"
assert_eq "mysql: schema 清空(切库后不残留)" '""' "$DB_SCHEMA_RAW"

section "normalize_db_fields (postgres)"
DB_TYPE='postgres'
DB_URL_RAW='"127.0.0.1"'
DB_PORT_RAW='5432'
DB_SCHEMA_RAW='"app"'
normalize_db_fields
assert_eq "postgres: url 不改"     '"127.0.0.1"' "$DB_URL_RAW"
assert_eq "postgres: port 不改"    '5432'        "$DB_PORT_RAW"
assert_eq "postgres: schema 保留"  '"app"'       "$DB_SCHEMA_RAW"

# ---------- 后端报错翻译 ----------
section "translate_backend_error"
assert_ok   "缺 libodbc 命中翻译" translate_backend_error \
  "error while loading shared libraries: libodbc.so.2: cannot open shared object file"
assert_fail "其他错误不翻译"       translate_backend_error "connection refused"
assert_fail "只含 libodbc 不够"   translate_backend_error "libodbc.so.2 loaded"

# ---------- 提问原语 ----------
section "ask"
ask GOT "端口" "3000" < <(printf '\n')
assert_eq "回车接受默认值" "3000" "$GOT"

ask GOT "端口" "3000" < <(printf '8080\n')
assert_eq "输入覆盖默认值" "8080" "$GOT"

ask GOT "地址" "" < <(printf '\n')
assert_eq "无默认值时回车得空串" "" "$GOT"

ask GOT "端口" "3000" port_is_valid "端口必须是 1-65535" < <(printf 'abc\n99999\n8080\n')
assert_eq "校验失败循环重问直到合法" "8080" "$GOT"

ask GOT "类型" "mysql" db_type_is_valid "只能是 mysql/postgres/sqlite" < <(printf '\n')
assert_eq "默认值也过校验" "mysql" "$GOT"

section "ask_secret"
ask_secret SECRET "密码" < <(printf 'p@ss\\w"rd\n')
assert_eq "密码原样读入(含反斜杠与引号)" 'p@ss\w"rd' "$SECRET"

ask_secret SECRET "密码" < <(printf '\n')
assert_eq "密码可为空" "" "$SECRET"

section "变量名碰撞检测"
ask_secret input "密码" < <(printf 'testvalue\n')
assert_eq "ask_secret: 变量名 input 不碰撞" "testvalue" "$input"

section "EOF 处理"
# EOF 时应立即退出,不应死循环。在子 shell 中运行并用 timeout 防止无限循环。
# 如果 fix 有问题(仍然死循环),timeout 会在 5 秒后杀掉进程返回 124。
status=0
(
  timeout 5 bash -c '
    source "$1"
    ask GOT "端口" "" port_is_valid "error" < <(printf "invalid\n")
  ' _ "$HERE/install.sh" 2>/dev/null
) || status=$?
# 期望:立即 exit 1(而不是 124,那是 timeout 杀掉的)
assert_eq "ask: EOF 时校验失败立即退出而非死循环" "1" "$status"

# ---------- 后端调用封装 ----------
section "run_backend / write_config_via_backend / check_config_via_backend"

STUB_DIR="$(mktemp -d)"
# 桩二进制的文件名必须等于 REPO_NAME —— run_backend 调的是 $PAYLOAD_DIR/$REPO_NAME。
cat > "$STUB_DIR/$REPO_NAME" <<'STUB'
#!/usr/bin/env bash
# 假二进制:按 STUB_MODE 决定行为,并把收到的参数写到 $STUB_DIR/args.txt
printf '%s\n' "$*" > "$(dirname "$0")/args.txt"
case "${STUB_MODE:-ok}" in
  ok)      exit 0 ;;
  oldbin)  echo "未知参数: --write-config" >&2; exit 1 ;;
  dbfail)  echo "DB connect timeout: 127.0.0.1:3306" >&2; exit 1 ;;
esac
STUB
chmod +x "$STUB_DIR/$REPO_NAME"

SAVED_PAYLOAD="$PAYLOAD_DIR"
PAYLOAD_DIR="$STUB_DIR"

# stub 是子进程,STUB_MODE 必须 export 才看得到。
# 不用 `STUB_MODE=ok assert_ok ...` 这种命令前缀写法 —— 对函数调用而言
# 该赋值是否导出给孙子进程,bash 各版本行为不一致。
export STUB_MODE
STUB_MODE=ok
assert_ok "write-config 成功" \
  write_config_via_backend "$STUB_DIR/config.toml" "$STUB_DIR/answers.toml"
assert_eq "传给二进制的参数正确" \
  "--write-config $STUB_DIR/config.toml --answers $STUB_DIR/answers.toml" \
  "$(cat "$STUB_DIR/args.txt")"

STUB_MODE=oldbin
rc=0
write_config_via_backend "$STUB_DIR/config.toml" "$STUB_DIR/answers.toml" 2>/dev/null || rc=$?
assert_eq "二进制过旧返回码 2" "2" "$rc"

STUB_MODE=dbfail
rc=0
ERR_TEXT="$(check_config_via_backend "$STUB_DIR/config.toml")" || rc=$?
assert_eq "check-config 失败返回非零" "1" "$rc"
assert_eq "错误原文透传到 stdout" \
  "DB connect timeout: 127.0.0.1:3306" "$ERR_TEXT"

STUB_MODE=ok
assert_ok "check-config 成功" check_config_via_backend "$STUB_DIR/config.toml"

PAYLOAD_DIR="$SAVED_PAYLOAD"
unset STUB_MODE
rm -rf "$STUB_DIR"

# ---------- 服务名与升级检测 ----------
section "svc_name / svc_unit_file"
assert_eq "无后缀"     "solidaxum"       "$(svc_name '')"
assert_eq "有后缀"     "solidaxum-proj1" "$(svc_name 'proj1')"

FAKE_SYSTEMD="$(mktemp -d)"
SAVED_SYSTEMD_DIR="$SYSTEMD_DIR"
SYSTEMD_DIR="$FAKE_SYSTEMD"

assert_eq "unit 路径" "$FAKE_SYSTEMD/solidaxum-proj1.service" \
  "$(svc_unit_file 'proj1')"

section "unit_working_dir"
cat > "$FAKE_SYSTEMD/solidaxum.service" <<'UNIT'
[Unit]
Description=solidaxum service

[Service]
User=app
WorkingDirectory=/opt/solidaxum
ExecStart=/opt/solidaxum/solidaxum
UNIT

assert_eq "取到 WorkingDirectory" "/opt/solidaxum" \
  "$(unit_working_dir "$FAKE_SYSTEMD/solidaxum.service")"
assert_eq "unit 不存在返回空" "" \
  "$(unit_working_dir "$FAKE_SYSTEMD/nosuch.service")"

section "unit_user / unit_group"
# 上面那个 unit 有 User=app,无 Group= —— 同时验读到与读不到。
# 模板渲染出的 unit 形如 User=app / Group=app(等号紧贴,无空格),
# 这两个函数就是为读它而写的。
assert_eq "取到 User" "app" \
  "$(unit_user "$FAKE_SYSTEMD/solidaxum.service")"
assert_eq "无 Group= 时返回空" "" \
  "$(unit_group "$FAKE_SYSTEMD/solidaxum.service")"
assert_eq "unit 不存在时 User 返回空" "" \
  "$(unit_user "$FAKE_SYSTEMD/nosuch.service")"
# 升级预填的核心场景:原 unit 用的是非默认账号,必须读得出来
cat > "$FAKE_SYSTEMD/custom-acct.service" <<'UNIT'
[Service]
User=svcacct
Group=ops
WorkingDirectory=/opt/solidaxum
UNIT
assert_eq "读到非默认 User" "svcacct" \
  "$(unit_user "$FAKE_SYSTEMD/custom-acct.service")"
assert_eq "读到非默认 Group" "ops" \
  "$(unit_group "$FAKE_SYSTEMD/custom-acct.service")"
rm -f "$FAKE_SYSTEMD/custom-acct.service"

section "list_instances"
assert_eq "单实例" \
  "$(printf 'solidaxum\t/opt/solidaxum')" "$(list_instances)"

cat > "$FAKE_SYSTEMD/solidaxum-proj1.service" <<'UNIT'
[Service]
WorkingDirectory=/srv/axum-proj1
UNIT
assert_eq "两个实例按服务名排序" \
  "$(printf 'solidaxum\t/opt/solidaxum\nsolidaxum-proj1\t/srv/axum-proj1')" \
  "$(list_instances)"

rm -f "$FAKE_SYSTEMD"/*.service
assert_eq "无实例时输出空" "" "$(list_instances)"

# ---------- 升级继承服务账号(回归保护) ----------
# 历史缺陷:升级时只从 config.toml 预填端口/DB/Redis,服务账号没预填,
# 用户一路回车会把原本的账号悄悄改回默认 app,随后 apply 的 chown -R
# 把 data/logs/web 一并改主,旧账号丢失访问权。
#
# 只测 unit_user / unit_group 不够 —— 那只证明「函数写对了」。这里测
# detect_upgrade 真的调用了它们,把「函数对但没人调」这类回归也堵上。
section "detect_upgrade: 升级时继承原有服务账号"

# WorkingDirectory 必须等于 PREFIX,否则会走「换目录」那条要交互确认的分支。
cat > "$FAKE_SYSTEMD/solidaxum.service" <<UNIT
[Service]
User=svcacct
Group=ops
WorkingDirectory=$PREFIX
UNIT

SVC_NAME="solidaxum"
SVC_FILE="$FAKE_SYSTEMD/solidaxum.service"
SVC_USER="app"      # 默认值,应被 unit 里的 User= 覆盖
SVC_GROUP="app"
detect_upgrade >/dev/null 2>&1

assert_eq "识别为升级"                  "true"     "$IS_UPGRADE"
assert_eq "SVC_USER 继承 unit 的 User="  "svcacct"  "$SVC_USER"
assert_eq "SVC_GROUP 继承 unit 的 Group=" "ops"     "$SVC_GROUP"

rm -f "$FAKE_SYSTEMD"/*.service

# ---------- cleanup_on_fail 的升级守卫(回归保护) ----------
# 历史缺陷:apply 里 systemctl enable / SERVICE_REGISTERED=true 不受 IS_UPGRADE
# 守卫,而 cleanup 的 SERVICE_REGISTERED 分支无条件 rm -f SVC_FILE ——
# 升级失败会把用户升级前的 unit 一起删掉。
section "cleanup_on_fail: 升级路径不删原有 unit"

# 用一个有标识内容的 unit 当「升级前的原件」
printf '[Service]\nWorkingDirectory=/opt/solidaxum\n' \
  > "$FAKE_SYSTEMD/solidaxum.service"
SVC_FILE_UNDER_TEST="$FAKE_SYSTEMD/solidaxum.service"

# 子 shell:source install.sh、设升级态、mock systemctl、跑 cleanup。
# cleanup 末尾会 exit,子 shell 退出后外层检查 SVC_FILE 去向。
run_cleanup_upgrade() {
  (
    source "$HERE/install.sh"
    # mock 掉 systemctl,别碰真 systemd
    systemctl() { :; }
    SVC_NAME="solidaxum"
    SVC_FILE="$SVC_FILE_UNDER_TEST"
    IS_UPGRADE=true
    SERVICE_REGISTERED=true
    CONFIG_WRITTEN=false      # 别让 cleanup 去动 $PREFIX/config.toml
    # apply 渲染前备份的「升级前原件」
    TMP_DIR="$(mktemp -d)"
    cp "$SVC_FILE" "$TMP_DIR/old-unit.service"
    OLD_UNIT_BACKUP="$TMP_DIR/old-unit.service"
    # 模拟 apply 的 sed 覆盖:把 unit 改成「新内容」
    printf '[Service]\nNEW-BODY\n' > "$SVC_FILE"
    cleanup_on_fail
  ) >/dev/null 2>&1
}

run_cleanup_upgrade
assert_eq "升级失败后 unit 文件仍在(未被删除)" \
  "yes" "$([ -f "$SVC_FILE_UNDER_TEST" ] && echo yes || echo no)"
assert_eq "升级失败后 unit 内容恢复成备份的原件" \
  "/opt/solidaxum" \
  "$(awk -F= '/^WorkingDirectory=/ {print $2; exit}' "$SVC_FILE_UNDER_TEST")"

# 对照组:全新安装态下 cleanup 应当删掉本次新建的 unit
run_cleanup_fresh() {
  (
    source "$HERE/install.sh"
    systemctl() { :; }
    SVC_NAME="solidaxum"
    SVC_FILE="$SVC_FILE_UNDER_TEST"
    IS_UPGRADE=false
    SERVICE_REGISTERED=true
    CONFIG_WRITTEN=false
    OLD_UNIT_BACKUP=""
    cleanup_on_fail
  ) >/dev/null 2>&1
}
run_cleanup_fresh
assert_eq "全新安装失败后 unit 被删除" \
  "deleted" "$([ -f "$SVC_FILE_UNDER_TEST" ] && echo present || echo deleted)"

SYSTEMD_DIR="$SAVED_SYSTEMD_DIR"
rm -rf "$FAKE_SYSTEMD"

# ---------- 环境探测兜底 ----------
section "探测工具选择与降级"

# 造四个假命令,再逐级收窄 PATH 验证降级顺序。
#
# ⚠ 桩必须在收窄 PATH 之前全部建好:PATH 一旦只剩桩目录,chmod / mktemp
#   这些外部命令就找不到了。
STUB_BIN="$(mktemp -d)"
for c in ss netstat curl wget; do
  printf '#!/bin/sh\nexit 0\n' > "$STUB_BIN/$c"
  chmod +x "$STUB_BIN/$c"
done
ONLY_NETSTAT="$(mktemp -d)"; cp "$STUB_BIN/netstat" "$ONLY_NETSTAT/"
ONLY_WGET="$(mktemp -d)";    cp "$STUB_BIN/wget"    "$ONLY_WGET/"
NOTHING="$(mktemp -d)"

SAVED_PATH="$PATH"

PATH="$NOTHING"
assert_eq "无 ss 无 netstat 时返回空" "" "$(port_probe_tool)"
assert_eq "无 curl 无 wget 时返回空" "" "$(http_probe_tool)"
# 无探测工具时必须「不占用」,否则全新安装会被自己堵死
assert_fail "无探测工具时不报占用" port_in_use 3000

PATH="$ONLY_NETSTAT"
assert_eq "只有 netstat 时选 netstat" "netstat" "$(port_probe_tool)"

PATH="$ONLY_WGET"
assert_eq "只有 wget 时选 wget" "wget" "$(http_probe_tool)"

PATH="$STUB_BIN"
assert_eq "ss 优先于 netstat" "ss"   "$(port_probe_tool)"
assert_eq "curl 优先于 wget"  "curl" "$(http_probe_tool)"

PATH="$SAVED_PATH"
rm -rf "$STUB_BIN" "$ONLY_NETSTAT" "$ONLY_WGET" "$NOTHING"

# 上面只测了 port_probe_tool / http_probe_tool 本身和 port_in_use 的无工具
# 兜底分支;ss/netstat 的手写 awk/grep 解析、以及 health_ok 整个函数还没有
# 真实覆盖过 —— 这两处是风险最高的代码(自己拼解析 + /dev/tcp 兜底),
# 补上真实分支覆盖。
section "port_in_use(ss 分支)"

# port_in_use 的 ss/netstat 分支内部还会再拼一条 awk | grep 管道,光收窄到
# 只放假 ss/netstat 的目录会把 awk/grep 也一起挡在外面(和上一节「桩必须在
# 收窄 PATH 之前建好」是同一类坑的变种)。
# 解法:另建一个「工具链目录」,放两个转发到真实 awk/grep 的 wrapper(路径
# 在收窄 PATH 之前先解析好),连同假 ss/netstat 一起组成完全自控的 PATH ——
# 不并入真实 PATH 的任何一段,这样即便在真安装了 iproute2 的 Linux 上跑,
# 也不会因为宿主机真有 ss 而漏过「只有 netstat」这个场景。
#
# 桩脚本本身也不能用 cat <<EOF 吐输出:PATH 收窄之后 cat 同样找不到。
# 改用 printf(shell 内建,不走 PATH 查找)。
TOOLCHAIN_BIN="$(mktemp -d)"
REAL_AWK="$(command -v awk)"
REAL_GREP="$(command -v grep)"
cat > "$TOOLCHAIN_BIN/awk" <<EOF
#!/bin/sh
exec "$REAL_AWK" "\$@"
EOF
cat > "$TOOLCHAIN_BIN/grep" <<EOF
#!/bin/sh
exec "$REAL_GREP" "\$@"
EOF
chmod +x "$TOOLCHAIN_BIN/awk" "$TOOLCHAIN_BIN/grep"

SS_LISTEN_BIN="$(mktemp -d)"
cat > "$SS_LISTEN_BIN/ss" <<'STUB'
#!/bin/sh
printf '%s\n' \
  'State  Recv-Q Send-Q Local Address:Port Peer Address:Port' \
  'LISTEN 0      128          0.0.0.0:22        0.0.0.0:*' \
  'LISTEN 0      128     127.0.0.1:18092        0.0.0.0:*' \
  'LISTEN 0      128             [::]:3000         [::]:*'
STUB
chmod +x "$SS_LISTEN_BIN/ss"

SAVED_PATH="$PATH"
PATH="$SS_LISTEN_BIN:$TOOLCHAIN_BIN"

assert_ok   "ss: 端口被监听时判定占用"         port_in_use 22
assert_fail "ss: 未被监听的端口判定未占用"     port_in_use 9999
# 18092 在监听,查 8092 绝不能命中 —— 这正是 [:.] 前缀 + \$ 结尾锚点要防
# 的后缀碰撞类 bug,测不出这个这条测试就没有意义。
assert_fail "ss: 18092 监听时 8092 不误判占用(后缀碰撞)" port_in_use 8092
assert_ok   "ss: IPv6 形式 [::]:3000 判定占用" port_in_use 3000

PATH="$SAVED_PATH"
rm -rf "$SS_LISTEN_BIN"

section "port_in_use(netstat 分支)"

NETSTAT_LISTEN_BIN="$(mktemp -d)"
cat > "$NETSTAT_LISTEN_BIN/netstat" <<'STUB'
#!/bin/sh
printf '%s\n' \
  'Active Internet connections (only servers)' \
  'Proto Recv-Q Send-Q Local Address           Foreign Address         State' \
  'tcp        0      0 0.0.0.0:22              0.0.0.0:*               LISTEN' \
  'tcp        0      0 127.0.0.1:18092         0.0.0.0:*               LISTEN'
STUB
chmod +x "$NETSTAT_LISTEN_BIN/netstat"

SAVED_PATH="$PATH"
# 这里没有 ss:目录里只放了 netstat,PATH 也只由这两个自控目录拼成,
# 宿主机真实 PATH 上有没有 ss 完全不影响这条分支的判定。
PATH="$NETSTAT_LISTEN_BIN:$TOOLCHAIN_BIN"

assert_ok   "netstat: 端口被监听时判定占用"     port_in_use 22
assert_fail "netstat: 未被监听的端口判定未占用" port_in_use 9999
assert_fail "netstat: 18092 监听时 8092 不误判占用(后缀碰撞)" port_in_use 8092

PATH="$SAVED_PATH"
rm -rf "$NETSTAT_LISTEN_BIN" "$TOOLCHAIN_BIN"

section "health_ok"

CURL_OK_BIN="$(mktemp -d)"
printf '#!/bin/sh\nexit 0\n' > "$CURL_OK_BIN/curl"
chmod +x "$CURL_OK_BIN/curl"

CURL_FAIL_BIN="$(mktemp -d)"
printf '#!/bin/sh\nexit 1\n' > "$CURL_FAIL_BIN/curl"
chmod +x "$CURL_FAIL_BIN/curl"

# 这两个目录里只有 wget、没有 curl,确保真的选中了 wget 分支。
WGET_OK_BIN="$(mktemp -d)"
printf '#!/bin/sh\nexit 0\n' > "$WGET_OK_BIN/wget"
chmod +x "$WGET_OK_BIN/wget"

WGET_FAIL_BIN="$(mktemp -d)"
printf '#!/bin/sh\nexit 1\n' > "$WGET_FAIL_BIN/wget"
chmod +x "$WGET_FAIL_BIN/wget"

SAVED_PATH="$PATH"

PATH="$CURL_OK_BIN"
assert_ok   "curl 探活成功时健康"   health_ok 3000

PATH="$CURL_FAIL_BIN"
assert_fail "curl 探活失败时不健康" health_ok 3000

PATH="$WGET_OK_BIN"
assert_ok   "无 curl 时走 wget:探活成功时健康"   health_ok 3000

PATH="$WGET_FAIL_BIN"
assert_fail "无 curl 时走 wget:探活失败时不健康" health_ok 3000

PATH="$SAVED_PATH"
rm -rf "$CURL_OK_BIN" "$CURL_FAIL_BIN" "$WGET_OK_BIN" "$WGET_FAIL_BIN"

# /dev/tcp 兜底分支(无 curl 无 wget)需要一个真实可连接的监听 socket。
# 在这套纯桩测试里现造一个稳定可用的本地监听端口不划算也不可靠(依赖
# 沙箱网络权限、且没有 nc/python 之类的现成工具),予以跳过 —— 该分支
# 保持人工验证,不伪造一个总是通过的假测试。

# ---------- 汇总 ----------
# 后续 Task 的测试段一律插在这一行之前
summary
