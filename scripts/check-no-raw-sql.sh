#!/usr/bin/env bash
# 业务 crate 原生 SQL 归零断言。违反则退出非零,供 CI / 本地使用。
#
# 归零范围:solidaxum-rs/crates/*/src/ 的业务代码。
# 排除(归零范围外,故意保留原生 SQL):
#   - db_meta.rs        达梦/MySQL 元数据查询(方言相关,归达梦设计处理)
#   - crates/dm/        达梦方言改写层(MERGE INTO 拼接等)
#   - app-data testkit  DDL/种子字符串脚本与 execute_raw 的栖身处(设计 9.4);
#                        admin testkit 换轨后不再
#                       持有原生 SQL,豁免随之下线(收窄前是通用
#                       'testkit\.rs' 行)
#   - tests/            各 crate 集成测试
#   - app-data dialect  数据访问漏斗的方言出口(db_meta 的元数据
#                       SQL 收编于此,见数据访问 crate 设计 9 节;"dialect
#                       合法持有原生 SQL,它就是收编点")。文件/目录两种形态都盖。
#   - app-data context  漏斗本体的 ContextCore 实现 ConnectionTrait 时的
#                       execute_unprepared 委托(转发,不是 SQL 构造点)。
#
# Windows Git-Bash 注意:本机 grep 可能是 Windows 原生版(ugrep WIN64),
# 递归输出反斜杠路径(实测如 D:/.../crates\app-data\src\...),'/tests/' 类
# 过滤会被 '\tests\' 绕过(机器假红)——过滤前先 tr 归一化为 '/'(处置与
# 坑注释照抄 check-legacy-usage.sh)。
#
# 用法: check-no-raw-sql.sh [solidaxum-rs-path]   (默认: 脚本同级的 ../solidaxum-rs)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$SCRIPT_DIR/../solidaxum-rs}"

if [ ! -d "$ROOT/crates" ]; then
  echo "❌ 找不到 $ROOT/crates —— 请传入正确的 solidaxum-rs 路径" >&2
  exit 2
fi

# 路径归一化(反斜杠 -> 正斜杠)后再套排除过滤
hits=$(grep -rnE 'Statement::from_sql_and_values|Statement::from_string|Statement::from_sql_and_tuples|execute_unprepared' \
        "$ROOT/crates" --include='*.rs' \
    | tr '\\' '/' \
    | grep -v '/tests/' \
    | grep -v '/crates/app-data/src/testkit\.rs' \
    | grep -v '/codegen/service/db_meta\.rs' \
    | grep -v '/crates/dm/' \
    | grep -v '/crates/app-data/src/dialect\.\|/crates/app-data/src/dialect/' \
    | grep -v '/crates/app-data/src/context\.rs' \
    || true)

if [ -n "$hits" ]; then
  echo "❌ 业务 crate 仍有原生 SQL(应转 SeaORM 结构化 API):" >&2
  echo "$hits" >&2
  exit 1
fi

echo "✅ 业务 crate 原生 SQL 归零(db_meta / crates/dm / app-data testkit / tests 已排除)"
