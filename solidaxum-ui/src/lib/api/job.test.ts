import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  jobApi,
  jobStatusLabel,
  jobConcurrentLabel,
  DEFAULT_JOB_STATUS,
} from './job.js'

// 走查修复钉(2026-10-10): axum 0.8 的 nest("/job") + route("/") 只匹配
// /job,前端 POST/PUT /job/ 会 404,定时任务新增/编辑整个不可用。
// 这里钉住 create/update 的请求 URL 绝不允许尾斜杠回归。
describe('jobApi create/update URL 无尾斜杠', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  function mockFetch() {
    return vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ code: 200, msg: 'ok', data: null }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    )
  }

  it('create 打到 /job,不是 /job/', async () => {
    const f = mockFetch()
    await jobApi.create({ job_name: 'x' } as never)
    const url = String(f.mock.calls[0][0])
    expect(url.endsWith('/job')).toBe(true)
    expect(url.endsWith('/job/')).toBe(false)
  })

  it('update 打到 /job,不是 /job/', async () => {
    const f = mockFetch()
    await jobApi.update({ job_id: '1', job_name: 'x' } as never)
    const url = String(f.mock.calls[0][0])
    expect(url.endsWith('/job')).toBe(true)
    expect(url.endsWith('/job/')).toBe(false)
  })
})

describe('jobStatusLabel', () => {
  // 后端真值: job/handler.rs:126-128,164-166,192-206
  //   status=="1" → schedule_job(会被调度) / status=="0" → remove_job(暂停)
  it('"1" 是正常(会被调度)', () => {
    expect(jobStatusLabel('1')).toBe('正常')
  })
  it('"0" 是暂停', () => {
    expect(jobStatusLabel('0')).toBe('暂停')
  })
  it('未知值原样返回', () => {
    expect(jobStatusLabel('9')).toBe('9')
  })
})

describe('DEFAULT_JOB_STATUS', () => {
  it('新建任务默认为会被调度的状态,而不是暂停', () => {
    expect(DEFAULT_JOB_STATUS).toBe('1')
    expect(jobStatusLabel(DEFAULT_JOB_STATUS)).toBe('正常')
  })
})

describe('jobConcurrentLabel', () => {
  // 后端行为: scheduler.rs:103,116-117 → concurrent==0 取分布式锁(禁止并发), ==1 跳锁(允许)
  it('0 是禁止(取分布式锁)', () => {
    expect(jobConcurrentLabel(0)).toBe('禁止')
  })
  it('1 是允许(跳过锁)', () => {
    expect(jobConcurrentLabel(1)).toBe('允许')
  })
  it('未知值原样返回', () => {
    expect(jobConcurrentLabel(9)).toBe('9')
  })
  it('undefined 返回占位符', () => {
    expect(jobConcurrentLabel(undefined)).toBe('-')
  })
})
