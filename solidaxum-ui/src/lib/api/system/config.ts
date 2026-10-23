import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import { exportBlob } from '$lib/api/download.js'

/** 列表行 / 详情 ConfigListVo —— 对齐后端 (camelCase)。configType: Y 系统内置 / N 用户定义。 */
export interface ConfigListVo {
  configId: string
  configName: string
  configKey: string
  configValue: string
  configType?: string
  remark?: string
}

/** POST /system/config body */
export interface ConfigCreateBo {
  configName: string
  configKey: string
  configValue: string
  configType?: string
  remark?: string
}

/** PUT /system/config body */
export interface ConfigUpdateBo extends ConfigCreateBo {
  configId: string
}

/** 系统信息(标题/图标) —— GET /system/config/sysInfo,仅需登录。
 *  title 顶栏标题;icon 侧栏图标 URL(OSS)。存 sys_config 键 sys.title/sys.icon。 */
export interface SysInfoVo {
  title: string
  icon: string
}

export const configApi = {
  /** 系统信息(顶栏标题/侧栏图标)。失败由调用方兜底,不阻断登录。 */
  sysInfo: () => apiClient.get<SysInfoVo>('/system/config/sysInfo'),
  /** 分页列表 —— 后端 R<TableDataInfo<ConfigListVo>>,用 getList 解嵌套。 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<ConfigListVo>('/system/config/list', toPageQuery(q, extra)),
  /** 按 key 取原始字符串值 */
  getByKey: (key: string) => apiClient.get<string>(`/system/config/configKey/${key}`),
  /** 详情 */
  getById: (id: string) => apiClient.get<ConfigListVo>(`/system/config/${id}`),
  /** 新建 */
  create: (data: ConfigCreateBo) => apiClient.post<void>('/system/config', data),
  /** 修改 */
  update: (data: ConfigUpdateBo) => apiClient.put<void>('/system/config', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/config/${id}`),
  /** 按 key 更新参数值(预览开关等) */
  updateByKey: (configKey: string, configValue: string) =>
    apiClient.put<void>('/system/config/updateByKey', { configKey, configValue }),

  /**
   * 导出参数配置(xlsx)。后端 POST /system/config/export,Form<ConfigPageQuery> 提取
   * (config_handler.rs:165-170)。**该端点吃筛选条件**(list_all(query))。
   */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/system/config/export', 'config.xlsx', extra),

  /** 刷新参数缓存。后端 DELETE /system/config/refreshCache(config_route.rs:21)。 */
  refreshCache: () => apiClient.delete<void>('/system/config/refreshCache'),
}
