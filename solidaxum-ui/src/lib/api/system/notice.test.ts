import { apiClient } from '$lib/api/client.js'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { noticeApi } from './notice.js'

vi.mock('$lib/api/client.js', () => ({
  apiClient: {
    get: vi.fn(),
    getList: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(apiClient.getList).mockResolvedValue({ rows: [], total: 0 })
  vi.mocked(apiClient.get).mockResolvedValue({ code: 200, msg: 'ok' })
  vi.mocked(apiClient.post).mockResolvedValue({ code: 200, msg: 'ok' })
  vi.mocked(apiClient.put).mockResolvedValue({ code: 200, msg: 'ok' })
  vi.mocked(apiClient.delete).mockResolvedValue({ code: 200, msg: 'ok' })
})

describe('定向公告 / 已读回执 API', () => {
  it('发布走 POST /system/notice/{id}/publish(与后端路由一致)', async () => {
    await noticeApi.publish('1761800000000000001')
    expect(apiClient.post).toHaveBeenCalledWith(
      '/system/notice/1761800000000000001/publish',
      {}
    )
  })

  it('我的公告分页走 GET /system/notice/my', async () => {
    await noticeApi.my({ pageNum: 1, pageSize: 100 })
    expect(apiClient.getList).toHaveBeenCalledWith('/system/notice/my', {
      pageNum: '1',
      pageSize: '100',
    })
  })

  it('单条已读走 PUT /system/notice/{id}/read,幂等可重试', async () => {
    await noticeApi.markRead('1761800000000000001')
    expect(apiClient.put).toHaveBeenCalledWith(
      '/system/notice/1761800000000000001/read',
      {}
    )
  })

  it('全部已读走 PUT /system/notice/readAll', async () => {
    await noticeApi.readAll()
    expect(apiClient.put).toHaveBeenCalledWith('/system/notice/readAll', {})
  })

  it('创建/更新透传定向目标字段(targetType=USER 时带 userIds)', async () => {
    await noticeApi.create({
      noticeTitle: '定向公告',
      noticeType: '1',
      targetType: 'USER',
      userIds: '301,302',
    })
    expect(apiClient.post).toHaveBeenCalledWith(
      '/system/notice',
      expect.objectContaining({ targetType: 'USER', userIds: '301,302' })
    )

    await noticeApi.update({
      noticeId: '1761800000000000001',
      noticeTitle: '定向公告',
      targetType: 'ALL',
      userIds: undefined,
    })
    expect(apiClient.put).toHaveBeenCalledWith(
      '/system/notice',
      expect.objectContaining({ noticeId: '1761800000000000001' })
    )
  })
})
