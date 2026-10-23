import { apiClient } from '$lib/api/client.js'
import { exportBlob } from '$lib/api/download.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/** 登录日志 LoginInfo —— 对齐后端 (camelCase)。后端路径注意大写 I: /monitor/loginInfo。 */
export interface LoginInfo {
  infoId: string
  userName: string
  clientKey?: string
  /** sys_device_type 字典码:pc / android / ios / xcx */
  deviceType?: string
  ipaddr: string
  loginLocation?: string
  browser: string
  os: string
  status: string // "0" success / "1" fail
  msg?: string
  loginTime?: string
}

export const loginInfoApi = {
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<LoginInfo>('/monitor/loginInfo/list', toPageQuery(q, extra)),
  delete: (infoIds: string) => apiClient.delete<void>(`/monitor/loginInfo/${infoIds}`),
  clean: () => apiClient.delete<void>('/monitor/loginInfo/clean'),
  /**
   * 导出 xlsx。后端 POST /monitor/loginInfo/export 用 Form<LoginInfoQuery> 提取,
   * 收 form-urlencoded 不是 JSON。
   * ⚠️ exportBlob 抛裸 Error,调用方 catch 里必须自己 toast.error。
   */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/monitor/loginInfo/export', 'logininfo.xlsx', extra),
  /** 解锁账号 —— 清除用户名在 Redis 里的密码错误计数。后端是 GET */
  unlock: (userName: string) =>
    apiClient.get<void>(`/monitor/loginInfo/unlock/${encodeURIComponent(userName)}`),
}
