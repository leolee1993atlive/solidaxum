import { designerApi } from '$lib/api/workflow/designer.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DesignerStore } from '../state/designer.svelte.js'
import PropertyPanel from './PropertyPanel.svelte'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: {
    queryDef: vi.fn(),
    saveJson: vi.fn(),
    listenerList: vi.fn(),
    handlerFeedback: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(designerApi.listenerList).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: [],
  })
  vi.mocked(designerApi.handlerFeedback).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: [],
  })
})
afterEach(cleanup)

function setup() {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  const byName = (name: string) =>
    store.nodes.find((n) => n.data.json.nodeName === name)!
  return { store, byName }
}

describe('PropertyPanel 路由', () => {
  it('没有选中 → 只读展示定义摘要', () => {
    const { store } = setup()
    render(PropertyPanel, { props: { store } })
    expect(screen.getByText('请假申请-排他并行会签')).toBeInTheDocument()
    expect(screen.getByText(/leave6/)).toBeInTheDocument()
    expect(screen.queryByLabelText('节点编码')).toBeNull()
  })

  it('选中审批节点 → BetweenForm;选中网关 → GatewayForm 列出出线', async () => {
    const { store, byName } = setup()
    store.selectNode(byName('申请人').id)
    const view = render(PropertyPanel, { props: { store } })
    expect(screen.getByText('审批节点')).toBeInTheDocument()
    expect(screen.getByText('协作方式')).toBeInTheDocument()

    const gw = store.nodes.find((n) => n.type === 'gateway')!
    store.selectNode(gw.id)
    await view.rerender({ store })
    expect(screen.getByText('网关')).toBeInTheDocument()
    expect(screen.getByText('大于两天')).toBeInTheDocument()
  })

  it('网关表单里点出线 → 该边被选中,面板切到跳转表单', async () => {
    const { store } = setup()
    const gw = store.nodes.find((n) => n.type === 'gateway')!
    store.selectNode(gw.id)
    const view = render(PropertyPanel, { props: { store } })
    await fireEvent.click(screen.getByText('大于两天'))
    const selected = store.edges.find((e) => e.selected)!
    expect(selected.data.json.skipName).toBe('大于两天')
    expect(store.nodes.some((n) => n.selected)).toBe(false)
    await view.rerender({ store })
    expect(screen.getByText('跳转')).toBeInTheDocument()
    expect((screen.getByLabelText('跳转名称') as HTMLInputElement).value).toBe(
      '大于两天'
    )
  })

  it('开始 / 结束节点用各自的表单', async () => {
    const { store } = setup()
    store.selectNode(store.nodes.find((n) => n.type === 'start')!.id)
    const view = render(PropertyPanel, { props: { store } })
    expect(screen.getByText('开始节点')).toBeInTheDocument()
    expect(screen.getByText('+ 监听器')).toBeInTheDocument()
    store.selectNode(store.nodes.find((n) => n.type === 'end')!.id)
    await view.rerender({ store })
    expect(screen.getByText('结束节点')).toBeInTheDocument()
    expect(screen.queryByText('+ 监听器')).toBeNull()
  })
})
