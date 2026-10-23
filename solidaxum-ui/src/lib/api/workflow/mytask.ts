import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/** 待办/抄送行 —— 对齐后端 FlowTaskVo。
 *  注意:businessCode/businessTitle/categoryName/version 在后端带
 *  skip_serializing_if=Option::is_none，值为空时 JSON 里整个键不出现。 */
export interface FlowTaskVo {
  /** 任务 id(办理时作为 taskId 传)。 */
  id?: string
  definitionId?: string
  instanceId?: string
  nodeCode?: string
  nodeName?: string
  flowName?: string
  flowCode?: string
  /** 流程状态原码(draft/waiting/finish/...)，配 wf_business_status 字典。 */
  flowStatus?: string
  /** 后端产出的中文 label，字典不可用时的兜底。 */
  flowStatusName?: string
  category?: string
  categoryName?: string
  /** 业务表单路径与自定义标记,跳转用。 */
  formCustom?: string
  formPath?: string
  version?: string
  assigneeIds?: string
  /** 办理人(逗号分隔的名字)。 */
  assigneeNames?: string
  createByName?: string
  /** 业务主键,跳转用。 */
  businessId?: string
  /** 业务编码 —— 「业务编号」列的正确绑定字段(不是 applyCode,后者后端不存在)。 */
  businessCode?: string
  businessTitle?: string
  createTime?: string
  createBy?: string
  /**
   * 是否挂起。⚠️ **只有 `myDocument`(打到 `/workflow/instance/pageByCurrent`)下发** ——
   * 该端点返回的其实是 `FlowInstanceVo`(dto.rs:236 `is_suspended: bool`)而非 FlowTaskVo;
   * 另外三个 `/workflow/task/*` 端点没有这个字段。「我发起的」页的挂起/激活列绑的就是它。
   */
  isSuspended?: boolean
}

/** 已办行 —— 对齐后端 FlowHisTaskVo(dto.rs 约 :448 起)。
 *  与 FlowTaskVo 的两处关键差异:办理人是 approverName(不是 assigneeNames)，
 *  且独有 flowTaskStatus(wf_task_status 字典)。 */
export interface FlowHisTaskVo {
  id?: string
  definitionId?: string
  instanceId?: string
  /** 关联的原任务 id。 */
  taskId?: string
  nodeCode?: string
  nodeName?: string
  flowName?: string
  flowCode?: string
  flowStatus?: string
  flowStatusName?: string
  /** 任务状态原码(pass/back/cancel/...)，配 wf_task_status 字典。 */
  flowTaskStatus?: string
  cooperateType?: number
  cooperateTypeName?: string
  /** 办理人 —— 已办页「办理人」列绑这个,不是 assigneeNames。 */
  approver?: string
  approverName?: string
  message?: string
  runDuration?: string
  category?: string
  categoryName?: string
  formCustom?: string
  formPath?: string
  version?: string
  assigneeIds?: string
  assigneeNames?: string
  createByName?: string
  businessId?: string
  businessCode?: string
  businessTitle?: string
  createTime?: string
  createBy?: string
}

export const myTaskApi = {
  /** 我发起的 */
  myDocument: (q: PageQuery, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<FlowTaskVo>('/workflow/instance/pageByCurrent', toPageQuery(q, extra)),
  /** 我的待办 */
  taskWaiting: (q: PageQuery, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<FlowTaskVo>('/workflow/task/pageByTaskWait', toPageQuery(q, extra)),
  /** 我的已办 —— 返回 FlowHisTaskVo,不是 FlowTaskVo。 */
  taskFinish: (q: PageQuery, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<FlowHisTaskVo>('/workflow/task/pageByTaskFinish', toPageQuery(q, extra)),
  /** 我的抄送 */
  taskCopyList: (q: PageQuery, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<FlowTaskVo>('/workflow/task/pageByTaskCopy', toPageQuery(q, extra)),
}
