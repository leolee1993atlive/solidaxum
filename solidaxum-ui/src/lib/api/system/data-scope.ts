/**
 * 角色数据范围值域 —— **单一事实来源**,下拉与列表列都从这里取。
 *
 * 值以后端 `crates/system/src/permission/service.rs:151-164` 的 match 分支为准
 * (与 Vue `views/system/role/index.vue:402-409` 一致):
 *   1=全部 · 2=自定义(另带 dept_ids) · 3=本部门 · 4=本部门及以下
 *   5=`RoleScope::SelfOnly`(仅本人) · 6=`DeptAndBelow` + `SelfOnly`(部门及以下或本人)
 *
 * ⚠️ **5 与 6 不可互换**:两者标签曾写反,「仅本人」落库成 6,
 * 该角色实际拿到本部门及以下的全部数据 —— 权限静默放大,UI 上看不出来。
 * 改这张表前先去读那段 Rust;`data-scope.test.ts` 会把值域钉住。
 */
type DataScopeDef = {
  value: string
  /** 下拉里的完整措辞(对齐 Vue dataScopeOptions) */
  label: string
  /** 表格列里的短标签 */
  shortLabel: string
}

const DATA_SCOPE_DEFS: readonly DataScopeDef[] = [
  { value: '1', label: '全部数据权限', shortLabel: '全部' },
  { value: '2', label: '自定义数据权限', shortLabel: '自定义' },
  { value: '3', label: '本部门数据权限', shortLabel: '本部门' },
  { value: '4', label: '本部门及以下数据权限', shortLabel: '本部门及以下' },
  { value: '5', label: '仅本人数据权限', shortLabel: '仅本人' },
  { value: '6', label: '部门及以下或本人数据权限', shortLabel: '部门及以下或本人' },
]

export type DataScopeOption = { value: string; label: string }

/** 「分配权限 → 数据权限」下拉选项。 */
export const DATA_SCOPE_OPTIONS: readonly DataScopeOption[] =
  DATA_SCOPE_DEFS.map(({ value, label }) => ({ value, label }))

/** 表格列文本;未知值原样回显,空值给占位符。 */
export function dataScopeLabel(v?: string): string {
  return DATA_SCOPE_DEFS.find((d) => d.value === v)?.shortLabel ?? v ?? '-'
}
