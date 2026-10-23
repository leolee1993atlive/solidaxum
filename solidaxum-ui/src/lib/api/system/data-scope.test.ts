import { describe, expect, it } from 'vitest'
import { DATA_SCOPE_OPTIONS, dataScopeLabel } from './data-scope.js'

/**
 * 值域护栏 —— 出过一次事故:'5' 与 '6' 的标签写反,
 * 「仅本人数据权限」落库成 6,角色实际拿到本部门及以下的全部数据(权限静默放大)。
 * 下面这张表以**后端** `permission/service.rs:151-164` 的 match 分支为准
 * (与 Vue `role/index.vue:402-409` 一致),改动前请先去看那段 Rust。
 */
const BACKEND_VALUE_DOMAIN: ReadonlyArray<readonly [string, string]> = [
  ['1', '全部数据权限'],
  ['2', '自定义数据权限'],
  ['3', '本部门数据权限'],
  ['4', '本部门及以下数据权限'],
  ['5', '仅本人数据权限'], // RoleScope::SelfOnly
  ['6', '部门及以下或本人数据权限'], // DeptAndBelow + SelfOnly
]

describe('DATA_SCOPE_OPTIONS', () => {
  it('六项值与标签逐一对齐后端值域', () => {
    expect(DATA_SCOPE_OPTIONS.map((o) => [o.value, o.label])).toEqual(
      BACKEND_VALUE_DOMAIN.map(([value, label]) => [value, label])
    )
  })

  it("'5' 是仅本人 —— 不得含任何部门范围措辞", () => {
    const five = DATA_SCOPE_OPTIONS.find((o) => o.value === '5')
    expect(five?.label).toBe('仅本人数据权限')
    expect(five?.label).not.toContain('及以下')
  })

  it("'6' 是部门及以下 + 本人 —— 两个语素都要在", () => {
    const six = DATA_SCOPE_OPTIONS.find((o) => o.value === '6')
    expect(six?.label).toContain('及以下')
    expect(six?.label).toContain('本人')
  })

  it('值不重复且恰好六项(漏项会让某个数据范围永远选不到)', () => {
    const values = DATA_SCOPE_OPTIONS.map((o) => o.value)
    expect(values).toHaveLength(6)
    expect(new Set(values).size).toBe(6)
  })
})

describe('dataScopeLabel', () => {
  it('每个下拉值都有对应的列标签(两处同源,不会一处改了另一处漏改)', () => {
    for (const { value } of DATA_SCOPE_OPTIONS) {
      const short = dataScopeLabel(value)
      expect(short).not.toBe(value)
      expect(short).not.toBe('-')
    }
  })

  it("'5' 与 '6' 的列标签同样不能互换", () => {
    expect(dataScopeLabel('5')).toBe('仅本人')
    expect(dataScopeLabel('6')).toBe('部门及以下或本人')
  })

  it('未知值原样回显,空值给占位符', () => {
    expect(dataScopeLabel('9')).toBe('9')
    expect(dataScopeLabel(undefined)).toBe('-')
  })
})
