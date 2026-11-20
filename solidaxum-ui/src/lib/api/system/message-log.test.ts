import { apiClient } from '$lib/api/client.js'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { messageLogApi } from './messageLog.js'

vi.mock('$lib/api/client.js', () => ({
  apiClient: {
    getList: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(apiClient.getList).mockResolvedValue({ rows: [], total: 0 })
})

describe('消息发送历史 API', () => {
  it('列表携带渠道/状态/业务类型/时间范围与分页', async () => {
    await messageLogApi.list({
      pageNum: 2,
      pageSize: 20,
      channel: 'DINGTALK',
      status: 'FAILED',
      bizType: 'task_reminder',
      begin: '2026-10-01 00:00:00',
      end: '2026-10-31 23:59:59',
    })
    expect(apiClient.getList).toHaveBeenCalledWith('/system/messageLog/list', {
      pageNum: '2',
      pageSize: '20',
      channel: 'DINGTALK',
      status: 'FAILED',
      bizType: 'task_reminder',
      begin: '2026-10-01 00:00:00',
      end: '2026-10-31 23:59:59',
    })
  })

  it('空筛选不发送空串', async () => {
    await messageLogApi.list({
      pageNum: 1,
      pageSize: 10,
      channel: '',
      status: '',
      bizType: '',
      begin: '',
      end: '',
    })
    expect(apiClient.getList).toHaveBeenCalledWith('/system/messageLog/list', {
      pageNum: '1',
      pageSize: '10',
    })
  })
})
