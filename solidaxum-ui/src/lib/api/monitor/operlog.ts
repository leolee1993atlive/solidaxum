import { apiClient } from '$lib/api/client.js'
import { exportBlob } from '$lib/api/download.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/** 操作日志 OperLog —— 对齐后端 (camelCase)。businessType 数值含义见 businessTypeLabel。 */
export interface OperLog {
  operId: string
  title: string
  businessType: number // 1 add / 2 update / 3 delete / ...
  method?: string
  requestMethod: string
  operName: string
  operIp: string
  operLocation?: string
  operUrl?: string
  operParam?: string
  jsonResult?: string
  status: number // 0 success / 1 fail
  errorMsg?: string
  operTime?: string
  costTime: number
  /** ⚠️ 后端 oper_log.rs 中间件把 dept_name 写死成 None,当前恒空。 */
  deptName?: string
  clientKey?: string
  /** sys_device_type 字典码:pc / android / ios / xcx */
  deviceType?: string
  browser?: string
  os?: string
}

export function prettyJson(raw?: string): string {
  if (!raw) return '-'
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

export const operLogApi = {
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<OperLog>('/monitor/operlog/list', toPageQuery(q, extra)),
  delete: (operIds: string) => apiClient.delete<void>(`/monitor/operlog/${operIds}`),
  clean: () => apiClient.delete<void>('/monitor/operlog/clean'),
  /**
   * 导出 xlsx。后端 POST /monitor/operlog/export 用 Form<OperLogQuery> 提取,
   * 收 form-urlencoded 不是 JSON;走 list_all 吃筛选条件。
   * ⚠️ exportBlob 抛裸 Error,调用方 catch 里必须自己 toast.error。
   */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/monitor/operlog/export', 'operlog.xlsx', extra),
}
