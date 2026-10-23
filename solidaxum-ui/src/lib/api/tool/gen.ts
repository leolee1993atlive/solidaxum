import { apiClient, TOKEN_KEY, getTokenClientId } from '$lib/api/client.js'
import { filenameFromDisposition } from './gen-download.js'

export interface GenTableVo {
  tableId: string
  dataName?: string
  tableName?: string
  tableComment?: string
  className?: string
  createTime?: string
  updateTime?: string
}

export interface DbTableVo {
  tableName?: string
  tableComment?: string
  createTime?: string
  updateTime?: string
}

/** 列配置(对齐后端 GenColumnUpdateBo,`crates/system/src/codegen/model/gen_column.rs:96-117`)。 */
export interface GenColumnRow {
  columnId: string
  tableId: string
  columnName: string
  columnComment?: string
  columnType: string
  javaType: string
  javaField: string
  /** '1' 是 / '0' 否 */
  isPk: string
  isIncrement: string
  isRequired: string
  isInsert: string
  isEdit: string
  isList: string
  isQuery: string
  /** EQ NE GT GE LT LE LIKE BETWEEN */
  queryType: string
  /** input inputNumber textarea select radio checkbox switch datetime imageUpload fileUpload editor */
  htmlType: string
  dictType?: string
  sort: number
}

/** 表配置(对齐后端 GenTableUpdateBo,`gen_table.rs:96-129`)。UI 未触达的字段原值透传。 */
export interface GenTableInfo {
  tableId?: string
  tableName?: string
  tableComment?: string
  className?: string
  /** crud 单表 / tree 树表 */
  tplCategory?: string
  frontendType?: string
  packageName?: string
  moduleName?: string
  businessName?: string
  functionName?: string
  functionAuthor?: string
  genType?: string
  genPath?: string
  options?: string
  remark?: string
  parentMenuId?: string
  parentMenuName?: string
  enableExport?: boolean
  enableStatus?: boolean
  statusField?: string
  enableUnique?: boolean
  uniqueFields?: string[]
  enableSort?: boolean
  sortField?: string
  treeCode?: string
  treeParentCode?: string
  treeName?: string
  treeRootValue?: string
  treeAncestorsField?: string
  treeOrderField?: string
}

/** 详情：info=表配置，rows=列配置，tables=可选主子表列表。 */
export interface GenTableDetail {
  info: GenTableInfo
  rows: GenColumnRow[]
  tables?: Array<Record<string, unknown>>
}

/** 预览：模板名 -> 代码文本。 */
export type GenPreview = Record<string, string>

/**
 * 把列配置转成下拉候选 —— 增强选项的 statusField/uniqueFields/sortField
 * 与树表的 treeCode/treeParentCode/treeName 都从本表字段里选。
 * 值用 javaField(后端模板消费的是 java 属性名)。
 */
export function columnFieldOptions(
  rows: GenColumnRow[]
): { value: string; label: string }[] {
  return rows.map((r) => ({
    value: r.javaField,
    label: r.columnComment ? `${r.columnComment} (${r.columnName})` : r.columnName,
  }))
}

function pageParams(
  page: { pageNum: number; pageSize: number },
  extra: Record<string, string | number | undefined | null> = {}
) {
  const p: Record<string, string> = {
    pageNum: String(page.pageNum),
    pageSize: String(page.pageSize),
  }
  for (const [k, v] of Object.entries(extra)) {
    if (v != null && v !== '') p[k] = String(v)
  }
  return p
}

function tableQuery(tables: string, dataName?: string): string {
  return `tables=${encodeURIComponent(tables)}&dataName=${encodeURIComponent(dataName ?? '')}`
}

import { downloadBlob } from '$lib/api/download.js'

export const genApi = {
  /** 已导入表分页 */
  list: (q: { pageNum: number; pageSize: number }, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<GenTableVo>('/tool/gen/list', pageParams(q, extra)),
  /** 数据库表分页（导入用） */
  dbList: (q: { pageNum: number; pageSize: number }, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<DbTableVo>('/tool/gen/db/list', pageParams(q, extra)),
  /** 详情（info+rows） */
  detail: (tableId: string) =>
    apiClient.get<GenTableDetail>(`/tool/gen/${tableId}`),
  /** 删除（可逗号分隔多 id） */
  remove: (ids: string) =>
    apiClient.delete<void>(`/tool/gen/${ids}`),
  /** 导入数据库表 */
  importTable: (tables: string, dataName?: string) =>
    apiClient.post<void>(`/tool/gen/importTable?${tableQuery(tables, dataName)}`, {}),
  /**
   * 数据源名列表。
   * ⚠️ 端点是 /tool/gen/getDataNames(gen_route.rs:20),
   *    不是 datasource/names —— 后者后端没有,曾导致
   *    每次进页面弹一个 404 toast、数据源下拉从未渲染过。
   */
  getDataNames: () =>
    apiClient.get<string[]>('/tool/gen/getDataNames'),
  /** 保存修改 */
  save: (data: Record<string, unknown>) =>
    apiClient.put<void>('/tool/gen', data),
  /** 预览 */
  preview: (tableId: string) =>
    apiClient.get<GenPreview>(`/tool/gen/preview/${tableId}`),
  /** 同步数据库 */
  synchDb: (tableId: string) =>
    apiClient.get<void>(`/tool/gen/synchDb/${tableId}`),
  /**
   * 生成代码并下载 zip。单表与多表**共用同一个端点**(与 Vue 一致:
   * index.vue:270-279 的 handleGenTable(row?) 两条路径都走 batchGenCode)。
   * ⚠️ 查询参数名是 tableIdStr(gen_handler.rs:225),值是逗号分隔的**表 ID**;
   *    发 tables=<表名> 会解析出空 ids 并生成一个空 zip,不报错。
   * ⚠️ downloadBlob 抛裸 Error,调用方 catch 里必须自己 toast.error。
   */
  genCode: (tableIds: string) =>
    downloadBlob(
      `/tool/gen/batchGenCode?tableIdStr=${encodeURIComponent(tableIds)}`,
      'ruoyi.zip'
    ),
}

/**
 * 预览 tab 的确定性排序。
 * 后端 preview() 返回 HashMap<String,String>(gen_handler.rs:45),
 * Object.keys() 顺序不保证 → 默认 tab 每次可能不一样。
 * 按「后端 → 前端模板 → api 类型」分档,档内字典序。
 * 字典序天然让 model 文件({business}.rs)排在 {business}_handler.rs 之前
 * ('.' 0x2E < '_' 0x5F),即默认 tab 落在最像 Vue 版 domain 的那个文件上。
 */
export function sortPreviewNames(names: string[]): string[] {
  const rank = (n: string): number => {
    if (n.endsWith('.rs')) return 0
    if (n.endsWith('.vue') || n.endsWith('.svelte')) return 1
    if (n.endsWith('.ts')) return 2
    return 3
  }
  return [...names].sort((a, b) => rank(a) - rank(b) || (a < b ? -1 : a > b ? 1 : 0))
}
