import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import type { FlowHisTaskVo } from './mytask.js'

/**
 * 待办/已办任务 FlowTaskVo —— 对齐后端 (camelCase)。
 * flowStatus / flowStatusName: 当前节点的流程状态及展示名。
 * nodeType: 节点类型 (0 开始 / 1 中间 / 2 结束 / ... 按后端约定)。
 */
export interface FlowTaskVo {
  id: string
  instanceId: string
  nodeId?: string
  nodeCode?: string
  nodeName?: string
  nodeType: number
  flowName?: string
  flowCode?: string
  flowStatus: string
  flowStatusName?: string
  businessId?: string
  businessCode?: string
  businessTitle?: string
  createByName?: string
  createTime?: string
  /* 以下五项后端 dto.rs:390-398 都在。此前未声明,列绑定只能走下面的索引签名 ——
     那正是 applyCode 恒显示 '-' 好几个月却没人发现的成因。 */
  category?: string
  categoryName?: string
  version?: string
  assigneeIds?: string
  /** 办理人(逗号分隔的名字)。已办行的对应字段是 approverName,见 FlowHisTaskVo。 */
  assigneeNames?: string
  /** 业务表单路径与自定义标记,buildFormUrl 跳转用。 */
  formCustom?: string
  formPath?: string
}

/** POST /workflow/task/completeTask body */
export interface CompleteTaskBo {
  taskId: string
  message?: string
  flowCopyList?: { userId: string; userName?: string }[]
  variables?: Record<string, unknown>
}

/** 下一节点(POST /workflow/task/getNextNodeList 的返回元素)。 */
export interface FlowNextNodeVo {
  nodeCode: string
  nodeName: string
  nodeType?: number
}

/** 抄送人 —— 对齐后端 FlowCopyBo(dto.rs:641-647)。 */
export interface FlowCopyItem {
  userId: string
  userName?: string
}

/**
 * 任务操作类型。取值必须与后端 `task_service.rs:1254-1284` 的 match 分支一致,
 * 写错不会报错、只会走到 fallback 分支。
 */
export const TASK_OPS = {
  transfer: 'transferTask',
  delegate: 'delegateTask',
  addSign: 'addSignature',
  reductionSign: 'reductionSignature',
} as const

export type TaskOp = (typeof TASK_OPS)[keyof typeof TASK_OPS]

/** 转办/委托/加签/减签的公共 body。message 为空时不发该键。 */
export function buildOperationBody(
  taskId: string,
  userIds: string[],
  message?: string
): Record<string, unknown> {
  const body: Record<string, unknown> = { taskId, userIds }
  if (message) body.message = message
  return body
}

export const taskApi = {
  /** 全部待办 (分页) */
  pageByAllTaskWait: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<FlowTaskVo>('/workflow/task/pageByAllTaskWait', toPageQuery(q, extra)),
  /** 全部已办 (分页) —— 返回 FlowHisTaskVo,含 flowTaskStatus。区别于「我的已办」pageByTaskFinish。 */
  pageByAllTaskFinish: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) => apiClient.getList<FlowHisTaskVo>('/workflow/task/pageByAllTaskFinish', toPageQuery(q, extra)),
  /** 我的待办 (分页) */
  pageWait: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<FlowTaskVo>('/workflow/task/pageByTaskWait', toPageQuery(q, extra)),
  /** 我的已办 (分页) */
  pageFinish: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<FlowTaskVo>('/workflow/task/pageByTaskFinish', toPageQuery(q, extra)),
  /** 通过/完成任务 */
  complete: (data: CompleteTaskBo) => apiClient.post<void>('/workflow/task/completeTask', data),
  /** 任务详情 */
  getById: (taskId: string) => apiClient.get<FlowTaskVo>(`/workflow/task/getTask/${taskId}`),
  /** 下一节点列表(提交前预览) */
  getNextNodeList: (taskId: string) =>
    apiClient.post<FlowNextNodeVo[]>('/workflow/task/getNextNodeList', { taskId }),
  /** 退回 */
  backProcess: (body: Record<string, unknown>) =>
    apiClient.post<void>('/workflow/task/backProcess', body),
  /** 可退回的节点 */
  getBackTaskNode: (taskId: string, nowNodeCode: string) =>
    apiClient.get<FlowNextNodeVo[]>(
      `/workflow/task/getBackTaskNode/${taskId}/${nowNodeCode}`
    ),
  /** 终止 */
  termination: (taskId: string, comment?: string) =>
    apiClient.post<void>('/workflow/task/terminationTask', { taskId, comment }),
  /** 转办/委托/加签/减签 */
  operation: (op: TaskOp, body: Record<string, unknown>) =>
    apiClient.post<void>(`/workflow/task/taskOperation/${op}`, body),
  /**
   * 改派(把任务的办理人换成 userId)。
   * ⚠️ 后端 update_assignee(task_handler.rs:257-262) 的 body 是 AppJson<Vec<i64>> ——
   * **裸 JSON 数组**,不能包成 { taskIds }。包了会反序列化失败 → 400。
   */
  updateAssignee: (userId: string, taskIds: string[]) =>
    apiClient.put<void>(`/workflow/task/updateAssignee/${userId}`, taskIds),
  /** 当前任务的全部办理人(减签用) */
  currentTaskAllUser: (taskId: string) =>
    apiClient.get<Array<Record<string, unknown>>>(
      `/workflow/task/currentTaskAllUser/${taskId}`
    ),
  /**
   * 催办单条任务。
   * ⚠️ 后端 FlowUrgeTaskBo(dto.rs:708-713) 只有 taskId(必填单值) + message;
   * Vue 发的是 taskIdList 数组 + messageType,照抄会因 taskId 缺键而 400。
   * messageType(站内信/邮件/短信)后端无对应字段,发了会被静默忽略。
   *
   * `silent` 只由 urgeMany 传:循环里逐条失败若各弹一个 toast 会刷屏,交给调用方汇总。
   */
  urge: (taskId: string, message?: string, silent = false) => {
    const body = message ? { taskId, message } : { taskId }
    return silent
      ? apiClient.post<void>('/workflow/task/urgeTask', body, { silent: true })
      : apiClient.post<void>('/workflow/task/urgeTask', body)
  },
  /**
   * 批量催办:后端无批量端点,由前端逐条发。任一失败不中断,返回 {ok,fail} 供 UI 报告。
   * 逐条请求走 silent —— 失败计数由调用方一次性提示,不逐条弹 toast。
   */
  urgeMany: async (
    taskIds: string[],
    message?: string
  ): Promise<{ ok: number; fail: number }> => {
    let ok = 0
    let fail = 0
    for (const id of taskIds) {
      try {
        await taskApi.urge(id, message, true)
        ok++
      } catch {
        fail++
      }
    }
    return { ok, fail }
  },
}
