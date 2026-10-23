import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import { exportBlob } from '$lib/api/download.js'

/**
 * 请假申请 LeaveVo —— 对齐后端 (camelCase)。
 * leaveType 值域见 leave-options.ts(1事假/2调休/3病假/4婚假)。
 * status: wf_business_status 字典(draft/waiting/finish/cancel/back/invalid/termination)。
 */
export interface LeaveVo {
  id: string
  applyCode?: string
  leaveType: string
  startDate?: string
  endDate?: string
  leaveDays?: number
  remark?: string
  status?: string
  createTime?: string
}

/**
 * POST /workflow/leave body。
 * ⚠️ `leaveDays` **必填**(后端 dto.rs:885 `#[validate(range(min = 1))]`,类型是裸 i32)——
 *    少这个字段一律 400 `missing field leaveDays`。
 * ⚠️ **没有 startDate/endDate** —— 后端按 `start = now` / `end = now + leaveDays`
 *    自行推算(leave_service.rs:66-78),发过去也会被忽略。
 */
export interface LeaveAddBo {
  /**
   * 🔴 仅 `submitAndFlowStart` 消费,且**从「修改」页提交时必须带上**:
   * 后端有 id 就地更新既有单据、无 id 则新建(对齐 Java `insertOrUpdate`)。
   * 漏了它 = 点一次提交凭空多出一条单据,原草稿仍停在 draft。
   * 新增(`POST /workflow/leave`)不消费此字段,后端自己发雪花 id。
   */
  id?: string
  leaveType: string
  leaveDays: number
  remark?: string
  status?: string
  /** 流程定义编码,仅 submitAndFlowStart 消费;为空后端回退 'leave1'。 */
  flowCode?: string
}

/** PUT /workflow/leave body。后端 LeaveEditBo **无 flowCode、无日期**(dto.rs:893-903)。 */
export interface LeaveEditBo {
  id: string
  leaveType: string
  leaveDays: number
  remark?: string
  status?: string
}

export const leaveApi = {
  /** 分页列表 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<LeaveVo>('/workflow/leave/list', toPageQuery(q, extra)),
  /** 详情 */
  getById: (id: string) => apiClient.get<LeaveVo>(`/workflow/leave/${id}`),
  /** 新增 (保存草稿) —— 后端返回 R<LeaveVo> */
  create: (data: LeaveAddBo) => apiClient.post<LeaveVo>('/workflow/leave', data),
  /** 修改 —— 后端返回 R<LeaveVo> */
  update: (data: LeaveEditBo) => apiClient.put<LeaveVo>('/workflow/leave', data),
  /** 删除 (支持逗号分隔的多 id) */
  delete: (ids: string) => apiClient.delete<void>(`/workflow/leave/${ids}`),
  /** 提交并启动流程 (submit + start) —— 后端返回 R<LeaveVo> */
  submitAndStart: (data: LeaveAddBo) =>
    apiClient.post<LeaveVo>('/workflow/leave/submitAndFlowStart', data),
  /**
   * 导出。后端 POST /workflow/leave/export + Form<LeaveQueryBo>。
   * ⚠️ exportBlob 抛裸 Error(不经 apiClient),调用方 catch 里必须自己 toast。
   */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/workflow/leave/export', `leave_${Date.now()}.xlsx`, extra),
}
