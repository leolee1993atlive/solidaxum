import { apiClient } from '$lib/api/client.js'
import { downloadBlob } from '$lib/api/download.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/**
 * 流程定义 DefinitionListVo —— 对齐后端 (camelCase)。
 * isPublish: 0 未发布 / 1 已发布
 * activityStatus: 0 未激活 / 1 已激活
 */
export interface DefinitionListVo {
  id: string
  flowCode: string
  flowName: string
  version: number
  isPublish: number
  activityStatus: number
  category?: string
  categoryName?: string
  createTime?: string
}

/**
 * 纯函数: 激活/挂起流程定义的 URL。
 * 后端 ActiveQuery.active 为必填 bool(definition_handler.rs:119-121),
 * 缺该查询参数即 400 —— 勿去掉 ?active=。
 */
export function activeUrl(id: string, active: boolean): string {
  return `/workflow/definition/active/${id}?active=${active}`
}

/** 新增/编辑流程定义的表单模型(对齐 Vue index.vue:389-397 的 initFormData)。 */
export interface DefinitionForm {
  /** 空串 = 新增 */
  id: string
  flowCode: string
  flowName: string
  category: string
  /** CLASSICS 经典模式 / MIMIC 仿钉钉模式,编辑时禁改 */
  modelValue: string
  /** Y 动态表单 / N 否 */
  formCustom: string
  formPath: string
  /** JSON 字符串,当前只放 { autoPass } */
  ext: string
}

/** GET /workflow/definition/{id} 的详情(字段是 list 的超集,这里只取表单要用的)。 */
export type DefinitionDetailVo = Partial<DefinitionForm>

export const EMPTY_DEFINITION_FORM: DefinitionForm = {
  id: '',
  flowCode: '',
  flowName: '',
  category: '',
  modelValue: 'CLASSICS',
  formCustom: 'N',
  formPath: '',
  ext: '',
}

/**
 * 详情 → 表单 + autoPass。
 * ext 是自由 JSON,坏数据不能让弹窗打不开 —— 解析失败一律当 autoPass=false。
 * 对齐 Vue `handleUpdate`(index.vue:624-637)。
 */
export function buildDefinitionForm(vo: DefinitionDetailVo): {
  form: DefinitionForm
  autoPass: boolean
} {
  const form: DefinitionForm = {
    ...EMPTY_DEFINITION_FORM,
    id: vo.id ?? '',
    flowCode: vo.flowCode ?? '',
    flowName: vo.flowName ?? '',
    category: vo.category ?? '',
    modelValue: vo.modelValue ?? 'CLASSICS',
    formCustom: vo.formCustom ?? 'N',
    formPath: vo.formPath ?? '',
    ext: vo.ext ?? '',
  }
  let autoPass = false
  if (form.ext) {
    try {
      autoPass = JSON.parse(form.ext)?.autoPass === true
    } catch {
      autoPass = false
    }
  }
  return { form, autoPass }
}

/**
 * 表单 + autoPass → 提交体。autoPass 序列化进 ext(对齐 Vue handleSubmit,
 * index.vue:643-646)。新增时**不带 id**,否则后端会走更新分支。
 */
export function toDefinitionBody(
  form: DefinitionForm,
  autoPass: boolean
): Record<string, unknown> {
  const { id, ...rest } = form
  const body: Record<string, unknown> = {
    ...rest,
    ext: JSON.stringify({ autoPass }),
  }
  if (id) body.id = id
  return body
}

export const definitionApi = {
  /** 分页列表 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<DefinitionListVo>('/workflow/definition/list', toPageQuery(q, extra)),
  /** 未发布列表(新建/复制/导入的产物只在这里可见) */
  unPublishList: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<DefinitionListVo>(
      '/workflow/definition/unPublishList',
      toPageQuery(q, extra)
    ),
  /** 详情(编辑弹窗回填) */
  getById: (id: string) =>
    apiClient.get<DefinitionDetailVo>(`/workflow/definition/${id}`),
  /** 新增 */
  add: (body: Record<string, unknown>) =>
    apiClient.post<DefinitionListVo>('/workflow/definition', body),
  /** 修改 */
  edit: (body: Record<string, unknown>) =>
    apiClient.put<DefinitionListVo>('/workflow/definition', body),
  /** 发布 */
  publish: (id: string) => apiClient.put<void>(`/workflow/definition/publish/${id}`, {}),
  /** 取消发布 */
  unPublish: (id: string) => apiClient.put<void>(`/workflow/definition/unPublish/${id}`, {}),
  /** 复制 */
  copy: (id: string) => apiClient.post<void>(`/workflow/definition/copy/${id}`, {}),
  /** 激活 / 挂起。active=true 激活,false 挂起。 */
  active: (id: string, active: boolean) => apiClient.put<void>(activeUrl(id, active), {}),
  /** 获取流程 XML 字符串 (R<String>) */
  getXml: (id: string) => apiClient.get<string>(`/workflow/definition/xmlString/${id}`),
  /** 删除 (支持逗号分隔的多 id) */
  delete: (ids: string) => apiClient.delete<void>(`/workflow/definition/${ids}`),
  /**
   * 部署流程文件。multipart:file(必需) + category(Vue 也发,但当前 Rust 后端
   * 不消费 —— import_def 只取第一个带 filename 的字段,见 definition_handler.rs:403)。
   * 分类 tree-select 当前在后端下不生效,导入后分类取自 JSON 文件内容。
   */
  importDef: (file: File, category: string) => {
    const fd = new FormData()
    fd.append('file', file)
    fd.append('category', category)
    return apiClient.postForm<void>('/workflow/definition/importDef', fd)
  },
  /**
   * 导出流程定义为 JSON。⚠️ 后端是 POST(flow_route.rs:150),
   * 不是 GET —— downloadBlob 默认 GET,需传 method='POST'。
   */
  exportDef: (id: string, flowCode: string) =>
    downloadBlob(`/workflow/definition/exportDef/${id}`, `${flowCode}.json`, 'POST'),
}
