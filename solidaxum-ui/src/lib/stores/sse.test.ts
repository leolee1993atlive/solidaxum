import { describe, it, expect, vi, beforeEach } from 'vitest'

class MockEventSource {
  static instances: MockEventSource[] = []
  url: string
  onmessage: ((e: { data: string }) => void) | null = null
  onerror: (() => void) | null = null
  closed = false
  constructor(url: string) {
    this.url = url
    MockEventSource.instances.push(this)
  }
  close() {
    this.closed = true
  }
  emit(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) })
  }
}

describe('sse client', () => {
  beforeEach(() => {
    localStorage.clear()
    MockEventSource.instances = []
    vi.stubGlobal('EventSource', MockEventSource)
    localStorage.setItem('solidaxum_token', 'tok123')
    localStorage.setItem('solidaxum_client_id', 'cid456')
  })

  it('连接 URL 携带 clientid 与 Authorization query(后端 auth 中间件约定)', async () => {
    const { createSseClient } = await import('./sse.svelte')
    const c = createSseClient()
    c.connect()
    const url = MockEventSource.instances[0].url
    expect(url).toContain('/resource/message')
    expect(url).toContain('clientid=cid456')
    expect(url).toContain(`Authorization=${encodeURIComponent('Bearer tok123')}`)
    c.close()
  })

  it('收到消息追加到列表且未读+1, markAllRead 清零', async () => {
    const { createSseClient } = await import('./sse.svelte')
    const c = createSseClient()
    c.connect()
    MockEventSource.instances[0].emit({
      type: 'message', source: 'workflow', message: '您有新的待办任务:ae_ana',
      path: '/task/taskWaiting', messageId: 'm1', timestamp: 1,
    })
    expect(c.messages).toHaveLength(1)
    expect(c.unread).toBe(1)
    c.markAllRead()
    expect(c.unread).toBe(0)
  })

  it('无 token 时不建连', async () => {
    localStorage.clear()
    const { createSseClient } = await import('./sse.svelte')
    createSseClient().connect()
    expect(MockEventSource.instances).toHaveLength(0)
  })
})
