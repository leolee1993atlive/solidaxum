import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import { exportBlob } from '$lib/api/download.js'

/**
 * 列表行 / 详情 ClientListVo —— 对齐后端 (camelCase)。
 * clientSecret 在 GET 时返回 "******"(脱敏)。
 * grantTypeList 为授权类型数组,提交时由前端把逗号分隔文本切分而来。
 */
export interface ClientListVo {
  id: string
  clientId: string
  clientKey: string
  clientSecret: string
  grantType: string
  grantTypeList: string[]
  deviceType?: string
  accessPath?: string
  accessPathList?: string[]
  ipWhitelist?: string
  ipWhitelistList?: string[]
  activeTimeout: number
  timeout: number
  status: string
}

/** POST /system/client body */
export interface ClientCreateBo {
  clientKey: string
  clientSecret: string
  grantType?: string
  grantTypeList?: string[]
  deviceType?: string
  accessPath?: string
  ipWhitelist?: string
  activeTimeout: number
  timeout: number
  status?: string
}

/** PUT /system/client body */
export interface ClientUpdateBo extends ClientCreateBo {
  id: string
  clientId: string
}

export const clientApi = {
  /** 分页列表 —— 后端 R<TableDataInfo<ClientListVo>>,用 getList 解嵌套。 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<ClientListVo>('/system/client/list', toPageQuery(q, extra)),
  /** 详情 */
  getById: (id: string) => apiClient.get<ClientListVo>(`/system/client/${id}`),
  /** 新建 */
  create: (data: ClientCreateBo) => apiClient.post<void>('/system/client', data),
  /** 修改 */
  update: (data: ClientUpdateBo) => apiClient.put<void>('/system/client', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/client/${id}`),
  /** 切换状态 */
  changeStatus: (clientId: string, status: string) =>
    apiClient.put<void>('/system/client/changeStatus', { clientId, status }),

  /**
   * 导出客户端列表(xlsx)。后端 POST /system/client/export,Form<ClientPageQuery> 提取。
   * 后端 ClientExportRow 刻意不含 clientSecret。
   */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/system/client/export', 'client.xlsx', extra),
}
