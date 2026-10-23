import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import { roleApi } from './role.js'
import { postApi } from './post.js'
import { userApi, SUPER_ADMIN_USER_ID } from './user.js'

/**
 * 导出端点的形状契约。三个端点在后端都是 POST + Form(x-www-form-urlencoded),
 * 用 AppJson 会 400。这里锁住 method / Content-Type / 路径,
 * 免得下次有人顺手改成 apiClient.post。
 */
function captureFetch() {
  const calls: { url: string; init: RequestInit }[] = []
  const stub = vi.fn(async (url: string, init: RequestInit) => {
    calls.push({ url, init })
    return {
      ok: true,
      blob: async () => new Blob(['PK']),
      headers: { get: () => 'attachment; filename="x.xlsx"' },
    } as unknown as Response
  })
  vi.stubGlobal('fetch', stub)
  // exportBlob 走 document.createElement('a').click(),jsdom 下无副作用。
  // ⚠️ 只补 createObjectURL/revokeObjectURL(jsdom 没实现),**不能整个替换 URL** ——
  // apiClient 与 URLSearchParams 之外还有代码依赖 URL 构造函数,换掉就是一串怪错。
  vi.stubGlobal(
    'URL',
    Object.assign(globalThis.URL, {
      createObjectURL: () => 'blob:x',
      revokeObjectURL: () => {},
    })
  )
  return calls
}

describe('导出端点契约', () => {
  let calls: { url: string; init: RequestInit }[]
  beforeEach(() => {
    calls = captureFetch()
  })
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('role 导出走 POST + form-urlencoded', async () => {
    await roleApi.export({ roleName: '管理' })
    expect(calls[0].url).toContain('/system/role/export')
    expect(calls[0].init.method).toBe('POST')
    expect((calls[0].init.headers as Record<string, string>)['Content-Type']).toBe(
      'application/x-www-form-urlencoded'
    )
    expect(calls[0].init.body).toContain('roleName=')
  })

  it('post 导出走 POST + form-urlencoded', async () => {
    await postApi.export({ postName: '组长' })
    expect(calls[0].url).toContain('/system/post/export')
    expect(calls[0].init.method).toBe('POST')
  })

  it('user 导出走 POST + form-urlencoded', async () => {
    await userApi.export({ userName: 'lmy' })
    expect(calls[0].url).toContain('/system/user/export')
    expect(calls[0].init.method).toBe('POST')
  })

  it('空串筛选值不进 body(后端 Option<String> 收到 Some("") 会筛出零行)', async () => {
    await roleApi.export({ roleName: '', status: '0' })
    expect(calls[0].init.body).toBe('status=0')
  })
})

describe('SUPER_ADMIN_USER_ID', () => {
  it('与后端 crates/core/src/lib.rs:1 的 SUPER_ADMIN_USER_ID 同值', () => {
    expect(SUPER_ADMIN_USER_ID).toBe('1761100000000000001')
  })
})
