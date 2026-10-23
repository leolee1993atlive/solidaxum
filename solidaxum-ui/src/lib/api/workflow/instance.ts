import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/**
 * 流程实例 FlowInstanceVo —— 对齐后端 (camelCase)。
 * flowStatus / flowStatusName: 实例状态及展示名。
 * activityStatus: 流转状态 (按后端约定)。
 *
 * ⚠️ id 口径:后端 `FlowInstanceVo.id` 是裸 `i64`
 * (dto.rs:222,无自定义 serializer),serde_json 以 **number** 下发。
 * 雪花 id > 2^53 时 JS Number 会丢精度 —— 这是后端待整改项(应转 string 下发),
 * 本仓既有 vo 一律按 string 承载,这里沿用。删/作废/查变量传 id 时用 String(r.id),
 * 后端 Path<i64>/Option<i64> 的 from_str 可正常解析。
 */
export interface FlowInstanceVo {
  id: string
  definitionId?: string
  flowName?: string
  flowCode?: string
  businessId: string
  nodeType?: number
  nodeCode?: string
  nodeName?: string
  flowStatus: string
  flowStatusName?: string
  activityStatus?: number
  businessCode?: string
  businessTitle?: string
  createByName?: string
  createTime?: string
  // 补全字段(对齐后端 dto.rs:221-257)
  version?: string
  category?: string
  categoryName?: string
  /** 是否挂起。后端由 activity_status 派生,口径已对齐 Vue。 */
  isSuspended?: boolean
  updateTime?: string
  formCustom?: string
  formPath?: string
}

/** 审批记录条目(FlowHisTaskVo,dto.rs:337-353 camelCase:办理人是 `approver`,时间只有 `createTime`)。 */
export interface FlowHisTaskVo {
  id: string
  nodeName?: string
  approver?: string
  flowStatus?: string
  message?: string
  createTime?: string
}

/**
 * GET /workflow/instance/flowHisTaskList/{businessId} 的 data:
 * 对齐 Java FlwInstanceServiceImpl#flowHisTaskList,返回 {list, instanceId}(不是裸数组);
 * instanceId 给流程图用(/warm-flow/query-flow-chart/{instanceId})。
 */
export interface FlowHisTaskListVo {
  list: FlowHisTaskVo[]
  instanceId?: string | null
}

export const instanceApi = {
  /** 运行中实例 (分页) */
  pageRunning: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<FlowInstanceVo>(
      '/workflow/instance/pageByRunning',
      toPageQuery(q, extra)
    ),
  /** 已完成实例 (分页) */
  pageFinish: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<FlowInstanceVo>(
      '/workflow/instance/pageByFinish',
      toPageQuery(q, extra)
    ),
  /** 我发起的实例 (分页) */
  pageCurrent: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<FlowInstanceVo>(
      '/workflow/instance/pageByCurrent',
      toPageQuery(q, extra)
    ),
  /** 按 businessId 删除 (支持逗号分隔的多 id) */
  deleteByBusinessIds: (ids: string) =>
    apiClient.delete<void>(`/workflow/instance/deleteByBusinessIds/${ids}`),
  /**
   * 按实例 id 批量删除(运行中)。Vue 用的是这个,不是 deleteByBusinessIds。
   * 路径 Path<String>(逗号串,instance_handler.rs:234)。
   */
  deleteByInstanceIds: (ids: string[]) =>
    apiClient.delete<void>(
      `/workflow/instance/deleteByInstanceIds/${ids.join(',')}`
    ),
  /** 按实例 id 批量删除(已完成的历史实例)。路径 Path<String>(:263)。 */
  deleteHisByInstanceIds: (ids: string[]) =>
    apiClient.delete<void>(
      `/workflow/instance/deleteHisByInstanceIds/${ids.join(',')}`
    ),
  /**
   * 作废。comment 为作废原因,后端已支持。
   * 后端 invalid 服务优先按 businessId 查实例(instance_service.rs:1078 `(Some(bid), _)`),
   * businessId 是 String —— 用它传既不走雪花 id 又避免 Number() 截断(全站 id 契约:
   * 不得把 id 转 number,见 id-contract.test.ts)。
   */
  invalid: (body: { businessId: string; comment?: string }) =>
    apiClient.post<void>('/workflow/instance/invalid', body),
  /** 查看实例变量(路径参数,后端 Path<i64> 用 from_str 解析字符串数字,无需转换)。 */
  instanceVariable: (instanceId: string) =>
    apiClient.get<Record<string, unknown>>(
      `/workflow/instance/instanceVariable/${instanceId}`
    ),
  /**
   * 更新实例变量。
   * 后端 FlowVariableBo.instance_id 虽是裸 i64(instance_handler.rs:73),但 handler 用的是
   * AppJson 提取器(:367),它先解成 serde_json::Value 再套 Coerce 反序列化(extract.rs:57),
   * 而 Coerce 的 deserialize_i64 会把 JSON 字符串 parse 成 i64(coerce.rs:64)——
   * 所以传字符串 id 是受支持的,也符合本仓「不把 id 转 number」的契约(id-contract.test.ts)。
   */
  updateVariable: (body: {
    instanceId: string
    variable: Record<string, unknown>
  }) => apiClient.put<void>('/workflow/instance/updateVariable', body),
  /** 某业务单据的审批记录 */
  flowHisTaskList: (businessId: string) =>
    apiClient.get<FlowHisTaskListVo>(
      `/workflow/instance/flowHisTaskList/${businessId}`
    ),
  /**
   * 撤销申请。
   * ⚠️ 后端 cancel_process_apply(flow_route.rs:191) 是 **PUT**(不是 POST),
   * 收 FlowCancelBo { businessId?, instanceId?, message? }。
   * 用 POST 会 405/404 —— 故封装为 PUT 并补 message 参数。
   * Vue handleCancelProcessApply 传 message: '申请人撤销流程！'。
   */
  cancelProcessApply: (businessId: string, message?: string) =>
    apiClient.put<void>(
      '/workflow/instance/cancelProcessApply',
      message ? { businessId, message } : { businessId }
    ),
}
