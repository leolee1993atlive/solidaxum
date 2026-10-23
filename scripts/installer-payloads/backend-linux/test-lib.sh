#!/usr/bin/env bash
# 安装脚本单测用的断言框架。由 test-install.sh / test-uninstall.sh source。
# 本文件只定义函数,不执行测试。

PASS=0
FAIL=0

ok() {
  PASS=$((PASS + 1))
  printf '  ✓ %s\n' "$1"
}

no() {
  FAIL=$((FAIL + 1))
  printf '  ✗ %s\n    期望: %s\n    实际: %s\n' "$1" "$2" "$3"
}

assert_eq() {   # assert_eq <描述> <期望> <实际>
  if [[ "$2" == "$3" ]]; then ok "$1"; else no "$1" "$2" "$3"; fi
}

assert_ok() {   # assert_ok <描述> <命令...>
  local desc="$1"; shift
  local rc=0
  "$@" || rc=$?
  if [[ $rc -eq 0 ]]; then ok "$desc"; else no "$desc" "退出码 0" "退出码 $rc"; fi
}

assert_fail() { # assert_fail <描述> <命令...>
  local desc="$1"; shift
  local rc=0
  "$@" || rc=$?
  if [[ $rc -ne 0 ]]; then ok "$desc"; else no "$desc" "非零退出码" "退出码 0"; fi
}

section() { printf '\n%s\n' "$1"; }

# 每个测试文件末尾调用一次。退出码即整体结果。
summary() {
  printf '\n%d 通过, %d 失败\n' "$PASS" "$FAIL"
  [[ $FAIL -eq 0 ]]
}
