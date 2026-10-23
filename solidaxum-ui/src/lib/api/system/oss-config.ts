import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/**
 * 列表行 / 详情 OssConfigVo —— 对齐后端 (camelCase)。
 * 注意:接口前缀为 /resource/oss/config(非 /system)。secretKey 在 GET 时返回明文,
 * 修改时若值为 "******" 后端会跳过更新。
 */
export interface OssConfigVo {
  ossConfigId: string
  configKey: string
  accessKey: string
  secretKey: string
  bucketName: string
  prefix?: string
  endpoint: string
  domainUrl?: string
  isHttps?: string
  region?: string
  status: string
  accessPolicy?: string
  remark?: string
}

/** POST /resource/oss/config body */
export interface OssConfigCreateBo {
  configKey: string
  accessKey: string
  secretKey: string
  bucketName: string
  prefix?: string
  endpoint: string
  domainUrl?: string
  isHttps?: string
  region?: string
  status?: string
  accessPolicy?: string
  remark?: string
}

/** PUT /resource/oss/config body */
export interface OssConfigUpdateBo extends OssConfigCreateBo {
  ossConfigId: string
}

export const ossConfigApi = {
  /** 分页列表 —— 后端 R<TableDataInfo<OssConfigVo>>,用 getList 解嵌套。 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<OssConfigVo>('/resource/oss/config/list', toPageQuery(q, extra)),
  /** 详情 */
  getById: (id: string) => apiClient.get<OssConfigVo>(`/resource/oss/config/${id}`),
  /** 新建 */
  create: (data: OssConfigCreateBo) => apiClient.post<void>('/resource/oss/config', data),
  /** 修改 */
  update: (data: OssConfigUpdateBo) => apiClient.put<void>('/resource/oss/config', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/resource/oss/config/${id}`),
  /** 切换状态 */
  changeStatus: (ossConfigId: string, status: string) =>
    apiClient.put<void>('/resource/oss/config/changeStatus', { ossConfigId, status }),
}
