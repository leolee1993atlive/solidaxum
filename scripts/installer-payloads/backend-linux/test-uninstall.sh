#!/usr/bin/env bash
# uninstall.sh 的纯函数单测。
# 不需要 root、不碰 systemd、不需要后端二进制。
# 用法: ./test-uninstall.sh
#
# ⚠ 单独测,不与 test-install.sh 合并:uninstall.sh 与 install.sh 各有一份
#   unit_working_dir / list_instances(刻意的副本),同时 source 会让后加载的
#   覆盖先加载的,测的就错了对象。test-install.sh 的头部同样注明了这点。

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=./test-lib.sh
source "$HERE/test-lib.sh"

# uninstall.sh 顶部有 set -euo pipefail;source 进来后测试要能跑过断言失败,
# 所以随即 set +e。main 守卫保证 source 时不执行 main。
# shellcheck source=./uninstall.sh
source "$HERE/uninstall.sh"
set +e

# uninstall.sh 的 REPO_NAME 取自脚本所在目录名(原地安装模式:包目录就叫
# solidaxum)。测试是在源码树的 backend-linux/ 里跑的,basename 会得到
# "backend-linux",与实测场景不符 —— 覆盖成真实包名。
REPO_NAME="solidaxum"

# ---------- unit_working_dir / list_instances ----------
# 这两个函数与 install.sh 是刻意重复的副本,单独验一遍保证它没漂移。
section "uninstall: unit_working_dir"
FAKE_SYSTEMD="$(mktemp -d)"
SAVED_SYSTEMD_DIR="$SYSTEMD_DIR"
SYSTEMD_DIR="$FAKE_SYSTEMD"

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

section "uninstall: list_instances"
# proj1 文件名在 glob 排序里本该排在 solidaxum.service 之前('-' < '.'),
# list_instances 必须 sort 修正 —— 这条正是回归保护。
cat > "$FAKE_SYSTEMD/solidaxum-proj1.service" <<'UNIT'
[Service]
WorkingDirectory=/srv/axum-proj1
UNIT
assert_eq "两个实例按服务名排序(proj1 不应排在前面)" \
  "$(printf 'solidaxum\t/opt/solidaxum\nsolidaxum-proj1\t/srv/axum-proj1')" \
  "$(list_instances)"

rm -f "$FAKE_SYSTEMD"/*.service
assert_eq "无实例时输出空" "" "$(list_instances)"

# ---------- pick_instance ----------
section "uninstall: pick_instance"
# 重新放回两个实例,测多实例选择
cat > "$FAKE_SYSTEMD/solidaxum.service" <<'UNIT'
[Service]
WorkingDirectory=/opt/solidaxum
UNIT
cat > "$FAKE_SYSTEMD/solidaxum-proj1.service" <<'UNIT'
[Service]
WorkingDirectory=/srv/axum-proj1
UNIT

pick_instance < <(printf '2\n')
assert_eq "选第 2 个实例" "solidaxum-proj1" "$PICKED_NAME"
assert_eq "带出其 WorkingDirectory" "/srv/axum-proj1" "$PICKED_PREFIX"

pick_instance < <(printf '9\nx\n1\n')
assert_eq "越界与非数字都重问,最终选到第 1 个" "solidaxum" "$PICKED_NAME"

# EOF 必须立即中止,不能死循环刷"请输入编号"。
#
# timeout 是这条测试的关键,不是装饰:若有人把 read 改回 `|| true`,EOF 下
# choice 恒空、校验恒失败,这里会**永久挂住**而不是报一条失败 —— 一条专为
# 防死循环写的回归测试,遇到死循环时自己先死了。有 timeout 才能把它变成
# 「退出码 124 ≠ 期望的 1」的正常失败。
#
# 另起 bash 而不是用子 shell:需要 SYSTEMD_DIR 传进去(否则新进程读默认的
# /etc/systemd/system,实例数为 0 会走「未发现实例」那条 exit 1 —— 同样返回
# 1,测试会因为错误的原因通过)。REPO_NAME 同理。
rc=0
SYSTEMD_DIR="$FAKE_SYSTEMD" REPO_NAME="solidaxum" \
  timeout 5 bash -c '
    source "$1"
    REPO_NAME="solidaxum"
    pick_instance
  ' _ "$HERE/uninstall.sh" < /dev/null > /dev/null 2>&1 || rc=$?
assert_eq "EOF 立即中止(回归保护:曾因 read||true 死循环;124=挂死)" "1" "$rc"

# 去掉 proj1 后只剩一个,验证单实例直选(无需输入编号)
rm -f "$FAKE_SYSTEMD/solidaxum-proj1.service"
pick_instance < <(printf '\n')
assert_eq "只剩一个实例时无需选择" "solidaxum" "$PICKED_NAME"

SYSTEMD_DIR="$SAVED_SYSTEMD_DIR"
rm -rf "$FAKE_SYSTEMD"

# ---------- 汇总 ----------
summary
