#!/usr/bin/env bash
# @REPO_NAME@ backend - Linux 交互式卸载
# 用法: sudo ./uninstall.sh
#
# 只注销 systemd 服务,不删除安装目录里的任何文件
# (二进制 / web / data / logs / config.toml 全部保留,自行 rm)
#
# 本脚本可被 source(用于单测),此时不执行 main —— 见文件末尾的守卫。

set -euo pipefail

# 单测覆盖此变量指向临时目录,避免碰真的 /etc
SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"

PREFIX="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_NAME="$(basename "$PREFIX")"
PICKED_NAME=""
PICKED_PREFIX=""

# 已注册实例的 WorkingDirectory。unit 不存在或没有该字段则输出空串。
#
# 注:与 install.sh 里的同名函数重复。刻意不 source install.sh ——
# 卸载不该依赖安装脚本存在。
unit_working_dir() {
  local unit="$1"
  [[ -f "$unit" ]] || return 0
  awk -F= '/^WorkingDirectory=/ {print $2; exit}' "$unit"
}

# 列出所有已注册实例,每行 "<服务名>\t<WorkingDirectory>"
# sort:glob 按完整文件名排序,'-'(0x2D) < '.'(0x2E),不 sort 的话
# @REPO_NAME@-proj1.service 会排在 @REPO_NAME@.service 前面,
# 多实例选择编号会错位。(同 install.sh 的 list_instances)
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

# 交互选择要卸载的实例,结果写入 PICKED_NAME / PICKED_PREFIX。
# 零个实例 → 退出;一个 → 直接选中;多个 → 编号让用户选。
pick_instance() {
  local lines=() line n choice
  while IFS= read -r line; do
    lines+=("$line")
  done < <(list_instances)

  n=${#lines[@]}
  if (( n == 0 )); then
    echo "未发现已注册的 $REPO_NAME 实例。" >&2
    exit 1
  fi

  if (( n == 1 )); then
    PICKED_NAME="${lines[0]%%$'\t'*}"
    PICKED_PREFIX="${lines[0]#*$'\t'}"
    return 0
  fi

  echo "检测到多个已注册实例:"
  local i
  for (( i = 0; i < n; i++ )); do
    printf '  %d) %s  (%s)\n' "$((i + 1))" \
      "${lines[i]%%$'\t'*}" "${lines[i]#*$'\t'}"
  done

  while true; do
    # read 在 EOF 时返回非零。不能 || true 吞掉:那样 EOF 下 choice 恒空、
    # 校验恒失败,会无限循环刷"请输入编号"。(install.sh 的 ask 原语同理)
    if ! read -r -p "要卸载哪一个? [1-$n]: " choice; then
      printf '输入意外结束(EOF),卸载中止。\n' >&2
      exit 1
    fi
    if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= n )); then
      PICKED_NAME="${lines[choice - 1]%%$'\t'*}"
      PICKED_PREFIX="${lines[choice - 1]#*$'\t'}"
      return 0
    fi
    echo "  ✗ 请输入 1 到 $n 之间的编号。" >&2
  done
}

main() {
  [[ $EUID -eq 0 ]] || { echo "需要 root 权限,请 sudo 运行" >&2; exit 1; }

  if [[ ! -t 0 || ! -t 1 ]]; then
    echo "本卸载程序需要交互式终端(TTY)。" >&2
    exit 1
  fi

  pick_instance

  echo ""
  echo "将注销 systemd 服务: $PICKED_NAME"
  echo "  服务目录: $PICKED_PREFIX"
  echo "  该目录下的文件(二进制 / web / config.toml / data / logs)不会被删除。"
  local answer
  read -r -p "确认卸载? (yes/no) [no]: " answer || true
  case "${answer:-no}" in
    y|Y|yes|YES) ;;
    *) echo "已取消。"; exit 0 ;;
  esac

  echo ""
  echo "停止并注销 $PICKED_NAME ..."
  systemctl stop "$PICKED_NAME" 2>/dev/null || true
  systemctl disable "$PICKED_NAME" 2>/dev/null || true
  rm -f "$SYSTEMD_DIR/$PICKED_NAME.service"
  systemctl daemon-reload

  cat <<EOF

✓ 已注销 systemd 服务 $PICKED_NAME
  原服务目录: $PICKED_PREFIX

该目录及其下的文件(二进制 / web / config.toml / data / logs)未被删除。
要彻底清理:
  sudo rm -rf $PICKED_PREFIX
注意 data/ 里是业务数据,删前请确认无需保留。
EOF
}

# ---------- 入口 ----------
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
