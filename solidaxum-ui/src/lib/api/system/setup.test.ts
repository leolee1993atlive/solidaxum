import { apiClient } from '$lib/api/client.js'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { markSetupCompleted, setupApi, shouldRedirectToSetup } from './setup.js'

vi.mock('$lib/api/client.js', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  // 每个用例都从"未记忆"状态开始(模块级 knownIncomplete 串用例)
  markSetupCompleted()
  vi.mocked(apiClient.get).mockResolvedValue({ code: 200, msg: 'ok' })
  vi.mocked(apiClient.post).mockResolvedValue({ code: 200, msg: 'ok' })
})

describe('首装初始化向导 API', () => {
  it('状态探测走 GET /system/setup/status', async () => {
    await setupApi.status()
    expect(apiClient.get).toHaveBeenCalledWith('/system/setup/status')
  })

  it('聚合完成走 POST /system/setup/complete,systemName 泛化命名', async () => {
    await setupApi.complete({
      systemName: 'SolidAxum 演示系统',
      adminOldPassword: 'Admin@123456',
      adminNewPassword: 'New@123456',
      userInitPassword: 'Init@456789',
    })
    expect(apiClient.post).toHaveBeenCalledWith('/system/setup/complete', {
      systemName: 'SolidAxum 演示系统',
      adminOldPassword: 'Admin@123456',
      adminNewPassword: 'New@123456',
      userInitPassword: 'Init@456789',
    })
  })

  it('未完成 → 应强导向导,并记住本会话状态(不再重复探测)', async () => {
    vi.mocked(apiClient.get).mockResolvedValue({
      code: 200,
      msg: 'ok',
      data: { completed: false },
    })

    expect(await shouldRedirectToSetup()).toBe(true)
    expect(await shouldRedirectToSetup()).toBe(true)
    // 第二次命中记忆,不重复打接口
    expect(apiClient.get).toHaveBeenCalledTimes(1)
  })

  it('已完成 → 不导向导', async () => {
    vi.mocked(apiClient.get).mockResolvedValue({
      code: 200,
      msg: 'ok',
      data: { completed: true },
    })

    expect(await shouldRedirectToSetup()).toBe(false)
  })

  it('探测失败按已完成处理,不拦截用户', async () => {
    vi.mocked(apiClient.get).mockRejectedValue(new Error('network'))

    expect(await shouldRedirectToSetup()).toBe(false)
  })

  it('markSetupCompleted 清掉记忆,恢复探测', async () => {
    vi.mocked(apiClient.get).mockResolvedValue({
      code: 200,
      msg: 'ok',
      data: { completed: false },
    })

    expect(await shouldRedirectToSetup()).toBe(true)
    markSetupCompleted()
    await shouldRedirectToSetup()
    expect(apiClient.get).toHaveBeenCalledTimes(2)
  })
})
