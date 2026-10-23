import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import { exportBlob } from '$lib/api/download.js'

export interface DictTypeVo {
  dictId: string
  dictName: string
  dictType: string
  remark?: string
}
export interface DictDataVo {
  dictCode: string
  dictSort: number
  dictLabel: string
  dictValue: string
  dictType: string
  cssClass?: string
  listClass?: string
  isDefault: string
  remark?: string
}

/** 字典类型下拉候选(GE9:字段信息的「字典类型」列用) */
export interface DictTypeOption {
  dictId: string
  dictName: string
  dictType: string
}

export const dictApi = {
  /** 按类型取字典数据 (R<Vec>),供 useDict 用 */
  getByType: (dictType: string) =>
    apiClient.get<DictDataVo[]>(`/system/dict/data/type/${dictType}`),
  /** 全量字典类型,供代码生成的字段字典配置选择 */
  optionselect: () =>
    apiClient.get<DictTypeOption[]>('/system/dict/type/optionselect'),
  // ---- dict_type 管理 ----
  typeList: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<DictTypeVo>('/system/dict/type/list', toPageQuery(q, extra)),
  typeGet: (id: string) => apiClient.get<DictTypeVo>(`/system/dict/type/${id}`),
  typeCreate: (data: Partial<DictTypeVo>) => apiClient.post<void>('/system/dict/type', data),
  typeUpdate: (data: Partial<DictTypeVo>) => apiClient.put<void>('/system/dict/type', data),
  typeDelete: (id: string) => apiClient.delete<void>(`/system/dict/type/${id}`),
  /**
   * 导出字典类型(xlsx)。后端 POST /system/dict/type/export。
   * ⚠️ 该端点**不吃筛选条件** —— handler 参数名是 `_query`,body 收下就丢,
   *    走的是无参的 list()(dict_type_handler.rs:144-150)。导出的恒为全量。
   */
  typeExport: () => exportBlob('/system/dict/type/export', 'dictType.xlsx'),
  /** 刷新字典缓存。后端 DELETE /system/dict/type/refreshCache。 */
  typeRefreshCache: () => apiClient.delete<void>('/system/dict/type/refreshCache'),
  // ---- dict_data 管理 ----
  dataList: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<DictDataVo>('/system/dict/data/list', toPageQuery(q, extra)),
  dataGet: (id: string) => apiClient.get<DictDataVo>(`/system/dict/data/${id}`),
  dataCreate: (data: Partial<DictDataVo>) => apiClient.post<void>('/system/dict/data', data),
  dataUpdate: (data: Partial<DictDataVo>) => apiClient.put<void>('/system/dict/data', data),
  dataDelete: (id: string) => apiClient.delete<void>(`/system/dict/data/${id}`),
  /**
   * 导出字典数据(xlsx)。后端 POST /system/dict/data/export。
   * ⚠️ 与 typeExport 同样**不吃筛选条件**(dict_data_handler.rs:139-145,`_query` + 无参 list_all())。
   */
  dataExport: () => exportBlob('/system/dict/data/export', 'dictData.xlsx'),
}
