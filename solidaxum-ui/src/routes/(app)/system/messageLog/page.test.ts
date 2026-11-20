import { messageLogApi, type MessageLogVO } from '$lib/api/system/messageLog.js'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import Page from './+page.svelte'

vi.mock('$app/state', () => ({
  page: { url: new URL('http://localhost/system/messageLog') },
}))

vi.mock('$lib/context/auth.svelte.js', () => ({
  useAuth: () => ({ hasPermission: () => true }),
  hasPermission: () => true,
  hasRole: () => true,
}))

vi.mock('svelte-sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn(), warning: vi.fn() },
}))

vi.mock('$lib/api/system/messageLog.js', () => ({
  messageLogApi: { list: vi.fn() },
}))

const row: MessageLogVO = {
  logId: '1001',
  channel: 'DINGTALK',
  bizType: 'task_reminder',
  bizId: '2002',
  status: 'FAILED',
  recipientUserIds: '["7"]',
  content: '任务提醒正文',
  errorMessage: '网络错误',
  createTime: '2026-10-31 10:00:00',
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(messageLogApi.list).mockResolvedValue({ rows: [row], total: 1 })
})

afterEach(cleanup)

describe('消息发送历史页', () => {
  it('展示渠道/状态/业务类型/失败原因', async () => {
    render(Page)
    await waitFor(() =>
      expect(messageLogApi.list).toHaveBeenCalledWith(
        expect.objectContaining({ pageNum: 1, pageSize: 20 })
      )
    )
    expect(await screen.findByText('钉钉')).toBeInTheDocument()
    expect(screen.getByText('失败')).toBeInTheDocument()
    expect(screen.getByText('task_reminder')).toBeInTheDocument()
    expect(screen.getByText('网络错误')).toBeInTheDocument()
  })

  it('按状态筛选后重新查询', async () => {
    render(Page)
    await screen.findByText('钉钉')

    await fireEvent.pointerDown(screen.getByLabelText('状态'))
    await fireEvent.pointerUp(
      await screen.findByRole('option', { name: '失败' })
    )
    await fireEvent.click(screen.getByRole('button', { name: '搜索' }))

    await waitFor(() =>
      expect(messageLogApi.list).toHaveBeenLastCalledWith(
        expect.objectContaining({ pageNum: 1, pageSize: 20, status: 'FAILED' })
      )
    )
  })
})
