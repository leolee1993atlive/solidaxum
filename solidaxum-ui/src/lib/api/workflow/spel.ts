import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/** 流程表达式定义行 —— 字段以后端 /workflow/spel/list 实际返回为准。 */
export interface SpelVo {
  id: string
  componentName?: string
  methodName?: string
  methodParams?: string
  viewSpel?: string
  remark?: string
  status?: string
  createTime?: string
}

/** POST /workflow/spel body —— 对齐后端 SpelAddBo(dto.rs:819-827)。 */
export interface SpelCreateBo {
  componentName?: string
  methodName?: string
  methodParams?: string
  /** 后端 String + validate(min=1),不可为空。 */
  viewSpel: string
  remark?: string
  status?: string
}

/** PUT /workflow/spel body —— 比 create 多一个 id。 */
export interface SpelUpdateBo extends SpelCreateBo {
  id: string
}

export const spelApi = {
  /** 分页列表(嵌套 R<TableDataInfo<SpelVo>>)。 */
  list: (q: PageQuery, extra: Record<string, string | number | undefined | null> = {}) =>
    apiClient.getList<SpelVo>('/workflow/spel/list', toPageQuery(q, extra)),
  getById: (id: string) => apiClient.get<SpelVo>(`/workflow/spel/${id}`),
  create: (data: SpelCreateBo) => apiClient.post<void>('/workflow/spel', data),
  update: (data: SpelUpdateBo) => apiClient.put<void>('/workflow/spel', data),
  delete: (ids: string[]) => apiClient.delete<void>(`/workflow/spel/${ids.join(',')}`),
}
