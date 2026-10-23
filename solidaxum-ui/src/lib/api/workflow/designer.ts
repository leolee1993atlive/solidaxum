import { apiClient } from '$lib/api/client.js'
import type { DefJson } from '$lib/flow/types.js'

/**
 * 办理人列表查询。handlerType 是 handler-type 返回的 tab 文本(如 "用户"),
 * 原样回传,后端按文本映射到 user/role/dept。
 * 后端只实现 handlerType/pageNum/pageSize/status(warm_flow_ui_handler.rs:150-160);
 * Vue 参考 selectUser.vue 还会发 handlerCode/handlerName/groupId,被静默忽略,故类型不收。
 */
export interface HandlerResultQuery {
  handlerType: string
  pageNum: number
  pageSize: number
  /** 状态过滤,对齐 Vue 参考(selectUser.vue:299 固定传 "0")。 */
  status?: string
}

export interface HandlerAuthRow {
  /** 形如 user:1 / role:2 / dept:3,写入 permissionFlag 时以 "@@" 拼接 */
  storageId: string
  handlerCode: string
  handlerName: string
  groupName?: string
  createTime?: string
}

export interface HandlerTreeNode {
  id: string
  name: string
  children?: HandlerTreeNode[]
}

export interface HandlerResult {
  handlerAuths: { rows: HandlerAuthRow[]; total: number }
  treeSelections: HandlerTreeNode[]
}

export interface HandlerFeedbackItem {
  storageId: string
  handlerName: string
}

/** 后端 query_def 的 id 可选,缺省/0 返回空模板,故空值不走路径段。 */
function queryDefUrl(id?: string): string {
  return id && id !== '0'
    ? `/warm-flow/query-def/${id}`
    : '/warm-flow/query-def'
}

function handlerResultParams(q: HandlerResultQuery): Record<string, string> {
  const params: Record<string, string> = {
    handlerType: q.handlerType,
    pageNum: String(q.pageNum),
    pageSize: String(q.pageSize),
  }
  if (q.status) params.status = q.status
  return params
}

/**
 * 设计器专用端点(/warm-flow/*,全部走 auth)。
 * 定义级增删改查在 definition.ts,这里只管 DefJson 的读写与面板辅助数据。
 */
export const designerApi = {
  /**
   * 保存定义。RuoYi 集成下设计器只编辑节点/跳转,定义级字段由列表页维护,
   * 因此默认 onlyNodeSkip=true,后端只更新节点与跳转。
   */
  saveJson: (def: DefJson, onlyNodeSkip = true) =>
    apiClient.post<void>('/warm-flow/save-json', def, {
      headers: { onlyNodeSkip: String(onlyNodeSkip) },
    }),
  queryDef: (id?: string) => apiClient.get<DefJson>(queryDefUrl(id)),
  /** 实例流程图:节点/跳转带 status,外加 chartStatusColor */
  queryFlowChart: (instanceId: string) =>
    apiClient.get<DefJson>(`/warm-flow/query-flow-chart/${instanceId}`),
  listenerList: () => apiClient.get<string[]>('/warm-flow/listener-list'),
  /** 选人弹窗 tab 文本列表,元素原样作为 handlerResult 的 handlerType */
  handlerType: () => apiClient.get<string[]>('/warm-flow/handler-type'),
  handlerResult: (q: HandlerResultQuery) =>
    apiClient.get<HandlerResult>(
      '/warm-flow/handler-result',
      handlerResultParams(q)
    ),
  /** 办理人名称回显;后端兼容逗号拼接与重复键,这里固定逗号拼接 */
  handlerFeedback: (storageIds: string[]) =>
    apiClient.get<HandlerFeedbackItem[]>('/warm-flow/handler-feedback', {
      storageIds: storageIds.join(','),
    }),
  // 以下三个后端目前返回空数组,面板据此隐藏对应入口;形状待后端实现后再收紧。
  handlerDict: () => apiClient.get<unknown[]>('/warm-flow/handler-dict'),
  nodeExt: () => apiClient.get<unknown[]>('/warm-flow/node-ext'),
  publishedForm: () => apiClient.get<unknown[]>('/warm-flow/published-form'),
}
