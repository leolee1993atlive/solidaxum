import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/** 在线用户 UserOnline —— 对齐后端 (camelCase)。tokenId 为字符串。 */
export interface UserOnline {
  tokenId: string
  userName: string
  clientKey?: string
  /** sys_device_type 字典码:pc / android / ios / xcx */
  deviceType?: string
  nickName?: string
  deptName?: string
  ipaddr: string
  loginLocation?: string
  browser: string
  os: string
  loginTime?: string
}

export const onlineApi = {
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<UserOnline>('/monitor/online/list', toPageQuery(q, extra)),
  forceLogout: (tokenId: string) => apiClient.delete<void>(`/monitor/online/${tokenId}`),
  /** 当前登录用户的在线会话信息(个人中心「在线设备」用) */
  currentUserInfo: () => apiClient.get<UserOnline[]>('/monitor/online'),
  /** 下线自己的某个会话(不是强退别人,权限不同) */
  deleteMySession: (tokenId: string) =>
    apiClient.delete<void>(`/monitor/online/myself/${tokenId}`),
}

/**
 * 纯函数: 前端切片分页。
 * 后端 list_online_users 从不分页(handler.rs:14-19 的 OnlineUserQuery 无分页字段,
 * service.rs:64-123 全量返回),Vue 侧同样是前端切片(index.vue:46-49)。
 * useCrudTable 走 manualPagination 不会自己切,必须在 fetchPage 里切。
 */
export function slicePage<T>(all: T[], pageNo: number, pageSize: number): T[] {
  const start = (pageNo - 1) * pageSize
  return (all ?? []).slice(start, start + pageSize)
}
