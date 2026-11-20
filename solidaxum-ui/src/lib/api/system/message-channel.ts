/**
 * 消息渠道配置 API(钉钉/企微/飞书)。
 *
 * 与 `sys_message_channel` 一一对应;密钥回读固定为 `******`,保存时空串
 * (或掩码原样)表示保留旧值。
 */
import { apiClient } from '$lib/api/client.js'

export const MANAGED_CHANNELS = ['DINGTALK', 'WECHAT', 'FEISHU'] as const
export type ManagedChannel = (typeof MANAGED_CHANNELS)[number]

export function isManagedChannel(value: string): value is ManagedChannel {
  return (MANAGED_CHANNELS as readonly string[]).includes(value)
}

export interface ChannelRow {
  channel: ManagedChannel
  channelName: string
  enabled: boolean
  clientId: string
  /** 已配置时固定为 ****** */
  clientSecret: string
  agentId: string
  agentSecret: string
  baseUrl: string
  domain: string
  remark?: string | null
  /** 凭据是否齐全(前端据此显示"未配置") */
  configured: boolean
  /** 生效来源:db / env / none */
  source: 'db' | 'env' | 'none'
}

export interface ChannelSaveBody {
  enabled?: boolean
  clientId?: string
  clientSecret?: string
  agentId?: string
  agentSecret?: string
  baseUrl?: string
  domain?: string
  remark?: string
}

export interface ChannelEditForm {
  enabled: boolean
  clientId: string
  clientSecret: string
  agentId: string
  agentSecret: string
  baseUrl: string
  domain: string
  remark: string
}

export interface ChannelTestResult {
  ok: boolean
  channel: string
  source: string
  tokenPrefix: string
  message: string
}

/**
 * 组装保存 body:空白/掩码密钥一律省略,交由后端保留旧值。
 * 这是「空白 secret 保留」契约在前端的唯一实现点,页面直接复用。
 */
export function buildChannelSaveBody(form: ChannelEditForm): ChannelSaveBody {
  const secret = form.clientSecret.trim()
  const agentSecret = form.agentSecret.trim()
  const body: ChannelSaveBody = {
    enabled: form.enabled,
    clientId: form.clientId.trim(),
    agentId: form.agentId.trim(),
    baseUrl: form.baseUrl.trim(),
    domain: form.domain.trim(),
    remark: form.remark.trim(),
  }
  if (secret && secret !== '******') body.clientSecret = secret
  if (agentSecret && agentSecret !== '******') body.agentSecret = agentSecret
  return body
}

export const messageChannelApi = {
  list: () => apiClient.get<ChannelRow[]>('/system/messageChannel/list'),
  save: (channel: ManagedChannel, body: ChannelSaveBody) =>
    apiClient.put<void>(`/system/messageChannel/${channel}`, body),
  /** 连通性自检:只取 token,不发送消息 */
  test: (channel: ManagedChannel) =>
    apiClient.post<ChannelTestResult>(
      `/system/messageChannel/${channel}/test`,
      {}
    ),
}
