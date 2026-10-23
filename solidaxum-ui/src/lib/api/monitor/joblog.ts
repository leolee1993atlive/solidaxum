import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

// snake_case —— 与 job entity 相同, 后端无 #[serde(rename_all="camelCase")]。
export interface SysJobLog {
  job_log_id: string
  job_name: string
  job_group: string
  invoke_target: string
  job_params?: string
  job_message?: string
  status: string // 1 成功 / 0 失败
  exception_info?: string
  start_time?: string
  stop_time?: string
  create_time?: string
}

/**
 * 纯函数: 任务日志执行状态 label。
 * 后端真值(勿改反): scheduler.rs 写入 Ok(_) => "1"(成功) / Err(_) => "0"(失败),
 * 与 DDL(lib.rs:33)、entity(sys_job_log.rs:17-18) 注释一致。
 */
export function jobLogStatusLabel(v?: string): string {
  if (v === '1') return '成功'
  if (v === '0') return '失败'
  return v ?? '-'
}

export const jobLogApi = {
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<SysJobLog>('/monitor/jobLog/list', toPageQuery(q, extra)),
  clean: () => apiClient.delete<void>('/monitor/jobLog/clean'),
}
