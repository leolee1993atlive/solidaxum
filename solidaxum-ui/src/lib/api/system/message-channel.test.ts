import { describe, expect, it } from 'vitest'
import {
  MANAGED_CHANNELS,
  buildChannelSaveBody,
  isManagedChannel,
  type ChannelEditForm,
} from './message-channel.js'

function form(overrides: Partial<ChannelEditForm> = {}): ChannelEditForm {
  return {
    enabled: true,
    clientId: ' client ',
    clientSecret: '',
    agentId: ' agent ',
    agentSecret: '',
    baseUrl: ' https://example.com ',
    domain: ' corp.example.com ',
    remark: ' 备注 ',
    ...overrides,
  }
}

describe('MANAGED_CHANNELS / isManagedChannel', () => {
  it('只维护三 IM 渠道且顺序固定', () => {
    expect([...MANAGED_CHANNELS]).toEqual(['DINGTALK', 'WECHAT', 'FEISHU'])
  })

  it('短信/APP/未知渠道不属于配置页', () => {
    expect(isManagedChannel('DINGTALK')).toBe(true)
    expect(isManagedChannel('WECHAT')).toBe(true)
    expect(isManagedChannel('FEISHU')).toBe(true)
    expect(isManagedChannel('SMS')).toBe(false)
    expect(isManagedChannel('APP')).toBe(false)
    expect(isManagedChannel('UNKNOWN')).toBe(false)
  })
})

describe('buildChannelSaveBody', () => {
  it('空白密钥省略,保留后端旧值', () => {
    const body = buildChannelSaveBody(form())
    expect(body.clientSecret).toBeUndefined()
    expect(body.agentSecret).toBeUndefined()
  })

  it('掩码 ****** 视为未修改', () => {
    const body = buildChannelSaveBody(
      form({ clientSecret: '******', agentSecret: '******' })
    )
    expect(body.clientSecret).toBeUndefined()
    expect(body.agentSecret).toBeUndefined()
  })

  it('真实密钥随 body 提交', () => {
    const body = buildChannelSaveBody(
      form({ clientSecret: ' real-secret ', agentSecret: 'agent-secret' })
    )
    expect(body.clientSecret).toBe('real-secret')
    expect(body.agentSecret).toBe('agent-secret')
  })

  it('文本字段去首尾空白并保留 enabled', () => {
    const body = buildChannelSaveBody(form())
    expect(body.enabled).toBe(true)
    expect(body.clientId).toBe('client')
    expect(body.agentId).toBe('agent')
    expect(body.baseUrl).toBe('https://example.com')
    expect(body.domain).toBe('corp.example.com')
    expect(body.remark).toBe('备注')
  })
})
