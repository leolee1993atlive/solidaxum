import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'

/**
 * 消息发送历史 API —— 对接 `/system/messageLog/list`(B5)。
 *
 * 全局审计流水,只读;筛选条件与后端 `SendLogPageQuery` 一一对应。
 * 时间范围键是 `begin` / `end`(不是 params[beginTime])。
 */

export interface MessageLogVO {
  logId: string
  tenantId?: string
  batchId?: string
  bizType?: string
  bizId?: string
  /** SSE/APP/SMS/WECHAT/DINGTALK/FEISHU */
  channel: string
  recipientUserIds?: string
  content?: string
  /** SKIPPED/SUCCESS/FAILED */
  status: string
  skipReason?: string
  errorMessage?: string
  providerResponse?: string
  createTime?: string
}

export interface MessageLogQuery extends PageQuery {
  channel?: string
  status?: string
  bizType?: string
  begin?: string
  end?: string
}

export const messageLogApi = {
  list: (q: MessageLogQuery) =>
    apiClient.getList<MessageLogVO>(
      '/system/messageLog/list',
      toPageQuery(q, {
        channel: q.channel,
        status: q.status,
        bizType: q.bizType,
        begin: q.begin,
        end: q.end,
      })
    ),
}
