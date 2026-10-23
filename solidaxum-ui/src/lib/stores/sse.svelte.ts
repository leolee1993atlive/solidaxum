// SSE 通知客户端: 对接后端 GET /resource/message(PushHub)。
// EventSource 无法设请求头, 凭据走 query(clientid + Authorization=Bearer),
// 与后端 auth 中间件 extract_token 的回退约定一致。
import { apiClient, getToken, getTokenClientId } from '$lib/api/client.js'
import { toast } from 'svelte-sonner'

export interface PushMessage {
  type: string
  source: string
  message: string
  path?: string | null
  messageId?: string
  timestamp?: number
}

const MAX_MESSAGES = 50

export function createSseClient() {
  let es: EventSource | null = null
  let messages = $state<PushMessage[]>([])
  let unread = $state(0)

  function connect() {
    const token = getToken()
    if (!token || es) return
    const url =
      `${apiClient.getBaseUrl()}/resource/message` +
      `?clientid=${encodeURIComponent(getTokenClientId())}` +
      `&Authorization=${encodeURIComponent(`Bearer ${token}`)}`
    es = new EventSource(url)
    es.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data) as PushMessage
        messages = [msg, ...messages].slice(0, MAX_MESSAGES)
        unread += 1
        if (msg.message) toast.info(msg.message)
      } catch {
        // 心跳/非 JSON 帧忽略
      }
    }
    es.onerror = () => {
      // EventSource 自动重连; token 被清(登出)时主动关闭
      if (!getToken()) close()
    }
  }

  function close() {
    es?.close()
    es = null
  }

  function markAllRead() {
    unread = 0
  }

  return {
    get messages() {
      return messages
    },
    get unread() {
      return unread
    },
    connect,
    close,
    markAllRead,
  }
}
