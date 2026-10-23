/**
 * gen-edit 的前端校验规则（GE8）。
 * 逐条移植 Vue 的 basicInfoForm.vue:45-50 与 genInfoForm.vue:349-408。
 * 抽成纯函数是为了能被单测锁定 —— 校验规则是最容易在后续改动里悄悄漏掉的东西。
 */
export type GenRuleError = { field: string; message: string }

const REQUIRED: Array<[string, string]> = [
  ['tableName', '请输入表名称'],
  ['tableComment', '请输入表描述'],
  ['className', '请输入实体类名称'],
  ['functionAuthor', '请输入作者'],
  ['tplCategory', '请选择生成模板'],
  ['frontendType', '请选择前端模板'],
  ['packageName', '请输入生成包路径'],
  ['moduleName', '请输入生成模块名'],
  ['businessName', '请输入生成业务名'],
  ['functionName', '请输入生成功能名'],
]

/** Vue 的 frontendType 正则(genInfoForm.vue:353)。 */
const FRONTEND_TYPE_PATTERN = /^[A-Za-z0-9_-]+$/

function blank(v: unknown): boolean {
  return typeof v !== 'string' || v.trim() === ''
}

export function validateGenTable(
  info: Record<string, unknown>
): GenRuleError[] {
  const errors: GenRuleError[] = []

  for (const [field, message] of REQUIRED) {
    if (blank(info[field])) errors.push({ field, message })
  }

  // 非空但形状不对时才补这一条,避免同一字段报两遍。
  if (
    !blank(info.frontendType) &&
    !FRONTEND_TYPE_PATTERN.test(String(info.frontendType))
  ) {
    errors.push({
      field: 'frontendType',
      message: '前端模板仅支持字母、数字、下划线和中划线',
    })
  }

  if (info.enableStatus === true && blank(info.statusField)) {
    errors.push({ field: 'statusField', message: '请选择状态字段' })
  }
  if (
    info.enableUnique === true &&
    (!Array.isArray(info.uniqueFields) || info.uniqueFields.length === 0)
  ) {
    errors.push({ field: 'uniqueFields', message: '请选择唯一字段' })
  }
  if (info.enableSort === true && blank(info.sortField)) {
    errors.push({ field: 'sortField', message: '请选择排序字段' })
  }

  if (info.tplCategory === 'tree') {
    if (blank(info.treeRootValue)) {
      errors.push({ field: 'treeRootValue', message: '请输入根节点值' })
    }
    // 后端 gen_table_service.rs:157-179 对 tree 强制校验这三项,
    // 不在前端拦就是「选了树表 → 保存 → 400」。
    if (blank(info.treeCode)) {
      errors.push({ field: 'treeCode', message: '请选择树编码字段' })
    }
    if (blank(info.treeParentCode)) {
      errors.push({ field: 'treeParentCode', message: '请选择树父编码字段' })
    }
    if (blank(info.treeName)) {
      errors.push({ field: 'treeName', message: '请选择树名称字段' })
    }
  }

  return errors
}
