import {
  messageChannelApi,
  type ChannelRow,
} from '$lib/api/system/message-channel.js'
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
  page: { url: new URL('http://localhost/system/messageChannel') },
}))

vi.mock('$lib/context/auth.svelte.js', () => ({
  useAuth: () => ({ hasPermission: () => true }),
  hasPermission: () => true,
  hasRole: () => true,
}))

vi.mock('svelte-sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn(), warning: vi.fn() },
}))

vi.mock('$lib/api/system/message-channel.js', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('$lib/api/system/message-channel.js')>()
  return {
    ...actual,
    messageChannelApi: { list: vi.fn(), save: vi.fn(), test: vi.fn() },
  }
})

function channel(
  channel: ChannelRow['channel'],
  channelName: string,
  configured: boolean,
  source: ChannelRow['source']
): ChannelRow {
  return {
    channel,
    channelName,
    enabled: true,
    clientId: `${channel}-client`,
    clientSecret: '******',
    agentId: '',
    agentSecret: '',
    baseUrl: '',
    domain: '',
    remark: '',
    configured,
    source,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(messageChannelApi.list).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: [
      channel('DINGTALK', '钉钉', true, 'db'),
      channel('WECHAT', '企业微信', false, 'none'),
      channel('FEISHU', '飞书', false, 'none'),
    ],
  })
  vi.mocked(messageChannelApi.save).mockResolvedValue({
    code: 200,
    msg: 'ok',
  })
  vi.mocked(messageChannelApi.test).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: {
      ok: true,
      channel: 'DINGTALK',
      source: 'db',
      tokenPrefix: 'abc',
      message: '连接成功',
    },
  })
})

afterEach(cleanup)

describe('消息渠道配置页', () => {
  it('展示三渠道及配置状态', async () => {
    render(Page)
    expect(await screen.findByText('钉钉')).toBeInTheDocument()
    expect(screen.getByText('企业微信')).toBeInTheDocument()
    expect(screen.getByText('飞书')).toBeInTheDocument()
    expect(screen.getByText('已配置')).toBeInTheDocument()
    expect(screen.getAllByText('未配置').length).toBeGreaterThanOrEqual(2)
  })

  it('保存时空白密钥省略,保留后端旧值', async () => {
    render(Page)
    await screen.findByText('钉钉')

    await fireEvent.click(screen.getAllByRole('button', { name: '保存' })[0])

    await waitFor(() =>
      expect(messageChannelApi.save).toHaveBeenCalledWith(
        'DINGTALK',
        expect.objectContaining({ enabled: true, clientId: 'DINGTALK-client' })
      )
    )
    const body = vi.mocked(messageChannelApi.save).mock.calls[0][1]
    expect(body.clientSecret).toBeUndefined()
    expect(body.agentSecret).toBeUndefined()
  })

  it('连接测试调用 test 接口', async () => {
    render(Page)
    await screen.findByText('钉钉')

    await fireEvent.click(
      screen.getAllByRole('button', { name: '连接测试' })[0]
    )
    await waitFor(() =>
      expect(messageChannelApi.test).toHaveBeenCalledWith('DINGTALK')
    )
  })
})
