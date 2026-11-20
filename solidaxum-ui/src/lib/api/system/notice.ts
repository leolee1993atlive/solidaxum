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
  /** 目标类型:ALL 全部 / USER 指定用户 */
  targetType?: string
  /** 指定用户 ID 串(逗号分隔,targetType=USER 时生效) */
  userIds?: string
  /** 发送状态:0 草稿 / 1 已发布 / 2 已撤回 */
  sendStatus?: string
  sendTime?: string
  remark?: string
  // NTC5/NTC6:list 端点批量把 create_by 翻译成 create_by_name(notice_handler.rs:30-44)。
  // ⚠️ 单查端点 get_notice_by_id 不做这步翻译,单查拿到的 create_by_name 仍是 None。
  createByName?: string
  createTime?: string
}

/** 当前用户可见公告(含已读回执) */
export interface NoticeMyVo {
  noticeId: string
  noticeTitle: string
  noticeType: string
  noticeContent?: string
  remark?: string
  targetType?: string
  sendTime?: string
  readFlag?: string
  readTime?: string
  starFlag?: string
  createTime?: string
}

/** POST /system/notice body */
export interface NoticeCreateBo {
  noticeTitle: string
  noticeType?: string
  noticeContent?: string
  status?: string
  targetType?: string
  userIds?: string
  remark?: string
}

/** PUT /system/notice body */
export interface NoticeUpdateBo extends NoticeCreateBo {
  noticeId: string
}

export const noticeApi = {
  /** 分页列表 —— 后端 R<TableDataInfo<NoticeListVo>>,用 getList 解嵌套。 */
  list: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<NoticeListVo>(
      '/system/notice/list',
      toPageQuery(q, extra)
    ),
  /** 详情 */
  getById: (id: string) => apiClient.get<NoticeListVo>(`/system/notice/${id}`),
  /** 详情(NTC2 深链接入口用)。⚠️ 单查端点不做 create_by_name 翻译,创建者可能为空。 */
  get: (noticeId: string) =>
    apiClient.get<NoticeListVo>(`/system/notice/${noticeId}`),
  /** 新建 */
  create: (data: NoticeCreateBo) =>
    apiClient.post<void>('/system/notice', data),
  /** 修改 */
  update: (data: NoticeUpdateBo) => apiClient.put<void>('/system/notice', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/notice/${id}`),
  /** 发布(按目标发送 SSE);已发布重复发布后端返回 400 */
  publish: (id: string) =>
    apiClient.post<void>(`/system/notice/${id}/publish`, {}),
  /** 当前用户可见公告(登录即可) */
  my: (q: PageQuery) =>
    apiClient.getList<NoticeMyVo>('/system/notice/my', toPageQuery(q)),
  /** 标记单条已读(幂等,并发唯一键冲突后端容忍) */
  markRead: (id: string) =>
    apiClient.put<void>(`/system/notice/${id}/read`, {}),
  /** 全部已读 */
  readAll: () => apiClient.put<void>('/system/notice/readAll', {}),
}
