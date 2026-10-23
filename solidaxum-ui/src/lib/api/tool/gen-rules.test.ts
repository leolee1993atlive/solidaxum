import { describe, expect, it } from 'vitest'
import { validateGenTable } from './gen-rules.js'

const OK = {
  tableName: 'sys_foo',
  tableComment: '示例表',
  className: 'SysFoo',
  functionAuthor: 'ry',
  tplCategory: 'crud',
  frontendType: 'vue',
  packageName: 'system',
  moduleName: 'system',
  businessName: 'foo',
  functionName: '示例',
}

const fields = (info: Record<string, unknown>) =>
  validateGenTable(info).map((e) => e.field)

describe('validateGenTable', () => {
  it('全部填齐时无错误', () => {
    expect(validateGenTable(OK)).toEqual([])
  })

  it('九项必填逐个拦住', () => {
    for (const k of [
      'tableName', 'tableComment', 'className', 'functionAuthor',
      'tplCategory', 'packageName', 'moduleName', 'businessName', 'functionName',
    ]) {
      expect(fields({ ...OK, [k]: '' })).toContain(k)
    }
  })

  it('frontendType 必填', () => {
    expect(fields({ ...OK, frontendType: '' })).toContain('frontendType')
  })

  it('frontendType 只允许字母数字下划线中划线', () => {
    expect(fields({ ...OK, frontendType: 'vue 3' })).toContain('frontendType')
    expect(fields({ ...OK, frontendType: 'my-tpl_1' })).not.toContain('frontendType')
  })

  it('enableStatus 为真时 statusField 必填', () => {
    expect(fields({ ...OK, enableStatus: true })).toContain('statusField')
    expect(fields({ ...OK, enableStatus: true, statusField: 'status' })).not.toContain('statusField')
    // 开关关着就不该拦
    expect(fields({ ...OK, enableStatus: false })).not.toContain('statusField')
  })

  it('enableUnique 为真时 uniqueFields 不能为空数组', () => {
    expect(fields({ ...OK, enableUnique: true })).toContain('uniqueFields')
    expect(fields({ ...OK, enableUnique: true, uniqueFields: [] })).toContain('uniqueFields')
    expect(fields({ ...OK, enableUnique: true, uniqueFields: ['name'] })).not.toContain('uniqueFields')
  })

  it('enableSort 为真时 sortField 必填', () => {
    expect(fields({ ...OK, enableSort: true })).toContain('sortField')
    expect(fields({ ...OK, enableSort: true, sortField: 'orderNum' })).not.toContain('sortField')
  })

  it('树表时 treeRootValue 与三项树字段都必填', () => {
    const f = fields({ ...OK, tplCategory: 'tree' })
    expect(f).toContain('treeRootValue')
    expect(f).toContain('treeCode')
    expect(f).toContain('treeParentCode')
    expect(f).toContain('treeName')
  })

  it('非树表时不拦树字段', () => {
    const f = fields(OK)
    expect(f).not.toContain('treeRootValue')
    expect(f).not.toContain('treeCode')
  })

  it('每条错误都带中文 message', () => {
    const errs = validateGenTable({ ...OK, tableName: '' })
    expect(errs[0].message).toBe('请输入表名称')
  })

  it('只填空白字符等于没填', () => {
    expect(fields({ ...OK, tableName: '   ' })).toContain('tableName')
  })
})
