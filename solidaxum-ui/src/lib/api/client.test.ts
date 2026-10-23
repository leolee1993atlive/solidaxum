import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
  apiClient,
  TOKEN_KEY,
  CLIENT_ID_KEY,
  getToken,
  clearToken,
  getTokenClientId,
} from './client'

function mockFetch(body: unknown, init?: { status?: number; ok?: boolean }) {
  const status = init?.status ?? 200
  return vi.spyOn(globalThis, 'fetch').mockResolvedValue(
    new Response(JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    })
  )
}

beforeEach(() => {
  localStorage.clear()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('client auth helpers', () => {
  beforeEach(() => localStorage.clear())

  it('getToken 读 localStorage,无值返回 null', () => {
    expect(getToken()).toBeNull()
    localStorage.setItem(TOKEN_KEY, 'abc')
    expect(getToken()).toBe('abc')
  })

  it('clearToken 同时清 token 与 clientid', () => {
    localStorage.setItem(TOKEN_KEY, 'abc')
    localStorage.setItem(CLIENT_ID_KEY, 'cid')
    clearToken()
    expect(localStorage.getItem(TOKEN_KEY)).toBeNull()
    expect(localStorage.getItem(CLIENT_ID_KEY)).toBeNull()
  })

  it('getTokenClientId 无值时回落默认', () => {
    expect(getTokenClientId()).toBe('')
    localStorage.setItem(CLIENT_ID_KEY, 'cid')
    expect(getTokenClientId()).toBe('cid')
  })
})

describe('ApiClient auth headers', () => {
  it('attaches Authorization Bearer + clientid when token present', async () => {
    localStorage.setItem(TOKEN_KEY, 'tok123')
    localStorage.setItem(CLIENT_ID_KEY, 'cid456')
    const f = mockFetch({ code: 200, msg: 'ok', data: { a: 1 } })
    await apiClient.get('/anything')
    const init = f.mock.calls[0][1] as RequestInit
    const h = init.headers as Record<string, string>
    expect(h['Authorization']).toBe('Bearer tok123')
    expect(h['clientid']).toBe('cid456')
  })

  it('omits Authorization when no token (still sends clientid)', async () => {
    localStorage.setItem(CLIENT_ID_KEY, 'cid')
    const f = mockFetch({ code: 200, msg: 'ok', data: null })
    await apiClient.get('/x')
    const h = (f.mock.calls[0][1] as RequestInit).headers as Record<string, string>
    expect(h['Authorization']).toBeUndefined()
    expect(h['clientid']).toBe('cid')
  })
})

describe('ApiClient single-value parsing', () => {
  it('returns data on code===200', async () => {
    mockFetch({ code: 200, msg: 'ok', data: { id: 7 } })
    const r = await apiClient.get('/x')
    expect(r.code).toBe(200)
    expect(r.data).toEqual({ id: 7 })
  })

  it('throws on business error code (!=200)', async () => {
    mockFetch({ code: 500, msg: '服务异常', data: null })
    await expect(apiClient.get('/x')).rejects.toThrow('服务异常')
  })
})

describe('ApiClient.getList nested paging', () => {
  it('unwraps data.{rows,total} into ListResult', async () => {
    mockFetch({
      code: 200,
      msg: 'ok',
      data: { code: 200, msg: '查询成功', rows: [{ id: 1 }, { id: 2 }], total: 2 },
    })
    const r = await apiClient.getList('/list', { pageNum: '1', pageSize: '10' })
    expect(r.rows).toHaveLength(2)
    expect(r.total).toBe(2)
    // 后端未触顶时不带这两个字段 —— 视图按「未截断」处理
    expect(r.totalCapped).toBeUndefined()
    expect(r.totalApprox).toBeUndefined()
  })

  it('carries totalCapped/totalApprox through', async () => {
    // 后端上限计数触顶:total 是**下界**,必须把这个事实带到视图,
    // 否则一个被截断的 10000 看着和精确总数一模一样。
    mockFetch({
      code: 200,
      msg: 'ok',
      data: {
        code: 200,
        msg: '查询成功',
        rows: [{ id: 1 }],
        total: 10000,
        totalCapped: true,
        totalApprox: 200002,
      },
    })
    const r = await apiClient.getList('/list')
    expect(r.total).toBe(10000)
    expect(r.totalCapped).toBe(true)
    expect(r.totalApprox).toBe(200002)
  })
})

describe('ApiClient 401 handling', () => {
  it('clears token and triggers redirect on 401', async () => {
    localStorage.setItem(TOKEN_KEY, 'tok')
    mockFetch({ code: 401, msg: 'unauth' }, { status: 401 })
    // 防止真的跳转
    const assignSpy = vi.spyOn(window, 'location', 'get')
    await expect(apiClient.get('/x')).rejects.toBeDefined()
    expect(localStorage.getItem(TOKEN_KEY)).toBeNull()
    assignSpy.mockRestore()
  })
})

describe('ApiClient custom headers', () => {
  it('post 透传 opts.headers,且不覆盖默认 Content-Type / clientid', async () => {
    localStorage.setItem(CLIENT_ID_KEY, 'cid')
    const f = mockFetch({ code: 200, msg: 'ok', data: null })
    await apiClient.post('/x', { a: 1 }, { headers: { onlyNodeSkip: 'true' } })
    const h = (f.mock.calls[0][1] as RequestInit).headers as Record<
      string,
      string
    >
    expect(h['onlyNodeSkip']).toBe('true')
    expect(h['Content-Type']).toBe('application/json')
    expect(h['clientid']).toBe('cid')
  })

  it('post 不传 headers 时行为不变', async () => {
    const f = mockFetch({ code: 200, msg: 'ok', data: null })
    await apiClient.post('/x', { a: 1 })
    const h = (f.mock.calls[0][1] as RequestInit).headers as Record<
      string,
      string
    >
    expect(h['Content-Type']).toBe('application/json')
    expect(Object.keys(h)).not.toContain('undefined')
  })
})
