import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

// snake_case —— 后端 job entity 没有 #[serde(rename_all="camelCase")],
// 是后端唯一以 snake_case 序列化的模块。故 TS 字段也保持 snake_case。
export interface SysJob {
  job_id: string
  job_name: string
  job_group: string
  invoke_target: string
  cron_expression: string
  job_params?: string
  misfire_policy: number // 1 立即执行 / 2 执行一次 / 3 放弃执行
  concurrent: number // 0 禁止(取分布式锁) / 1 允许(跳锁) —— scheduler.rs:116-117 实际行为
  status: string // 0 暂停 / 1 正常(运行,会被调度) —— job/handler.rs 调度分支
  remark?: string
  create_time?: string
}

export const jobApi = {
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<SysJob>('/job/list', toPageQuery(q, extra)),
  getById: (jobId: string) => apiClient.get<SysJob>(`/job/${jobId}`),
  // NOTE: create/update 走带尾斜杠的端点!
  // 路径不能带尾斜杠:axum 0.8 nest("/job") + route("/") 只匹配 /job,
  // POST /job/ 会 404(走查实测)。后端 update/create 同挂此路径。
  create: (data: Partial<SysJob>) => apiClient.post<void>('/job', data),
  update: (data: Partial<SysJob>) => apiClient.put<void>('/job', data),
  delete: (jobId: string) => apiClient.delete<void>(`/job/${jobId}`),
  changeStatus: (jobId: string, status: string) =>
    apiClient.put<void>('/job/changeStatus', { jobId, status }),
  run: (jobId: string) => apiClient.put<void>('/job/run', { jobId }),
}

/**
 * 纯函数: 定时任务状态 label。
 * 后端真值(勿改反): handler.rs 中 status=="1" 才 schedule_job(排入调度),
 * "0" 则 remove_job(移出调度)。与 DDL(lib.rs:16)、entity(sys_job.rs:21-22) 一致。
 */
export function jobStatusLabel(v?: string): string {
  if (v === '1') return '正常'
  if (v === '0') return '暂停'
  return v ?? '-'
}

/** 新建任务的默认状态: 必须是会被调度的 "1",否则建出来的任务永不执行。 */
export const DEFAULT_JOB_STATUS = '1'

/**
 * 纯函数: 并发执行 label。
 * 后端行为(勿改反): scheduler.rs:116-117 —— concurrent==0 时取分布式锁(禁止并发),
 * ==1 跳过锁(允许并发)。注意与 Java 语义(0 允许 1 禁止)相反,以 Rust 实际行为为准。
 */
export function jobConcurrentLabel(v?: number): string {
  if (v === 0) return '禁止'
  if (v === 1) return '允许'
  return v == null ? '-' : String(v)
}
