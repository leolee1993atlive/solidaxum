import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/** 列表行 / 详情 NoticeListVo —— 对齐后端 (camelCase)。noticeType: 1 通知 / 2 公告。 */
export interface NoticeListVo {
  noticeId: string
  noticeTitle: string
  noticeType: string
  noticeContent?: string
  status: string
  remark?: string
  // NTC5/NTC6:list 端点批量把 create_by 翻译成 create_by_name(notice_handler.rs:30-44)。
  // ⚠️ 单查端点 get_notice_by_id 不做这步翻译,单查拿到的 create_by_name 仍是 None。
  createByName?: string
  createTime?: string
}

/** POST /system/notice body */
export interface NoticeCreateBo {
  noticeTitle: string
  noticeType?: string
  noticeContent?: string
  status?: string
  remark?: string
}

/** PUT /system/notice body */
export interface NoticeUpdateBo extends NoticeCreateBo {
  noticeId: string
}

export const noticeApi = {
  /** 分页列表 —— 后端 R<TableDataInfo<NoticeListVo>>,用 getList 解嵌套。 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<NoticeListVo>('/system/notice/list', toPageQuery(q, extra)),
  /** 详情 */
  getById: (id: string) => apiClient.get<NoticeListVo>(`/system/notice/${id}`),
  /** 详情(NTC2 深链接入口用)。⚠️ 单查端点不做 create_by_name 翻译,创建者可能为空。 */
  get: (noticeId: string) => apiClient.get<NoticeListVo>(`/system/notice/${noticeId}`),
  /** 新建 */
  create: (data: NoticeCreateBo) => apiClient.post<void>('/system/notice', data),
  /** 修改 */
  update: (data: NoticeUpdateBo) => apiClient.put<void>('/system/notice', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/notice/${id}`),
}
