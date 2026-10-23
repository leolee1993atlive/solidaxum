import { designerApi } from '$lib/api/workflow/designer.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DesignerStore } from '../state/designer.svelte.js'
import BetweenForm from './BetweenForm.svelte'
import { resetOptionsCache } from './options.js'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: {
    queryDef: vi.fn(),
    saveJson: vi.fn(),
    listenerList: vi.fn(),
    handlerFeedback: vi.fn(),
    handlerType: vi.fn(),
    handlerResult: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  resetOptionsCache()
  vi.mocked(designerApi.listenerList).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: ['leaveListener'],
  })
  vi.mocked(designerApi.handlerFeedback).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: [],
  })
})
afterEach(cleanup)

// 下拉是 bits-ui Select:触发器按 pointerdown 打开,选项在 portal 里按 pointerup 选中
// (同 QueryForm.test)。触发器文案即当前值。
async function pick(label: string, option: string) {
  await fireEvent.pointerDown(screen.getByLabelText(label))
  await fireEvent.pointerUp(await screen.findByRole('option', { name: option }))
}

function setup(name = '经理会签') {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  const node = store.nodes.find((n) => n.data.json.nodeName === name)!
  store.selectNode(node.id)
  const current = () => store.nodes.find((n) => n.id === node.id)!
  return { store, node, current }
}

describe('BetweenForm · 基础', () => {
  it('回填编码/名称,改名逐字符写回', async () => {
    const { store, node, current } = setup()
    render(BetweenForm, { props: { store, node } })
    expect((screen.getByLabelText('节点编码') as HTMLInputElement).value).toBe(
      node.id
    )
    const name = screen.getByLabelText('节点名称') as HTMLInputElement
    expect(name.value).toBe('经理会签')
    await fireEvent.input(name, { target: { value: '经理' } })
    expect(current().data.json.nodeName).toBe('经理')
  })

  it('改节点编码:失焦提交,连带改相连边;重复编码报错并回退', async () => {
    const { store, node } = setup()
    const view = render(BetweenForm, { props: { store, node } })
    const code = screen.getByLabelText('节点编码') as HTMLInputElement
    await fireEvent.input(code, { target: { value: 'manager_sign' } })
    await fireEvent.blur(code)
    expect(store.nodes.some((n) => n.id === 'manager_sign')).toBe(true)
    expect(store.edges.some((e) => e.target === 'manager_sign')).toBe(true)
    expect(store.edges.some((e) => e.target === node.id)).toBe(false)
    // 真实页面里 {#key node.id} 会重建表单;这里把改名后的节点喂回去
    const renamed = store.nodes.find((n) => n.id === 'manager_sign')!
    await view.rerender({ store, node: renamed })
    const other = store.nodes.find((n) => n.data.json.nodeName === '申请人')!.id
    await fireEvent.input(code, { target: { value: other } })
    await fireEvent.blur(code)
    expect(screen.getByRole('alert')).toHaveTextContent(/已存在/)
    expect(code.value).toBe('manager_sign')
    expect(store.nodes.filter((n) => n.id === other)).toHaveLength(1)
  })

  it('nodeRatio "0.000" 显示为或签;切到票签写 50 并出现策略输入', async () => {
    const { store, node, current } = setup('申请人')
    expect(node.data.json.nodeRatio).toBe('0.000')
    const view = render(BetweenForm, { props: { store, node } })
    expect(screen.getByLabelText('或签')).toHaveAttribute(
      'data-state',
      'checked'
    )
    expect(screen.queryByLabelText('票签策略')).toBeNull()
    await fireEvent.click(screen.getByLabelText('票签'))
    expect(current().data.json.nodeRatio).toBe('50')
    await view.rerender({ store, node: current() })
    expect((screen.getByLabelText('票签策略') as HTMLInputElement).value).toBe(
      '50'
    )
  })

  it('票签策略校验:通过率越界显示错误;固定人数写 passCount=N', async () => {
    const { store, node, current } = setup('申请人')
    store.updateNodeJson(node.id, { nodeRatio: '50' })
    const view = render(BetweenForm, { props: { store, node: current() } })
    const value = screen.getByLabelText('票签策略') as HTMLInputElement
    await fireEvent.input(value, { target: { value: '150' } })
    expect(current().data.json.nodeRatio).toBe('150')
    await view.rerender({ store, node: current() })
    expect(screen.getByRole('alert')).toHaveTextContent(/0.001/)
    await pick('策略类型', '固定通过人数')
    await view.rerender({ store, node: current() })
    await fireEvent.input(screen.getByLabelText('票签策略'), {
      target: { value: '2' },
    })
    expect(current().data.json.nodeRatio).toBe('passCount=2')
  })

  it('驳回到指定节点:候选是沿 PASS 边回溯的前置审批节点', async () => {
    const { store, node, current } = setup('经理会签')
    render(BetweenForm, { props: { store, node } })
    const sel = screen.getByLabelText('驳回到指定节点')
    expect(sel.tagName).toBe('BUTTON')
    expect(sel).toHaveTextContent('不指定')
    await fireEvent.pointerDown(sel)
    const labels = (await screen.findAllByRole('option')).map((o) =>
      o.textContent?.trim()
    )
    expect(labels).toEqual(['不指定', '副经理', '副组长', '申请人', '组长'])
    await fireEvent.pointerUp(screen.getByRole('option', { name: '申请人' }))
    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    expect(current().data.json.anyNodeSkip).toBe(applicant.id)
  })

  it('表单:切自定义表单写 formCustom=Y;路径写 formPath', async () => {
    const { store, node, current } = setup()
    const view = render(BetweenForm, { props: { store, node } })
    await fireEvent.click(screen.getByLabelText('自定义表单'))
    expect(current().data.json.formCustom).toBe('Y')
    await view.rerender({ store, node: current() })
    await fireEvent.input(screen.getByLabelText('表单唯一标识'), {
      target: { value: 'leave_form' },
    })
    expect(current().data.json.formPath).toBe('leave_form')
  })
})

describe('BetweenForm · 监听器', () => {
  it('加一行、填类型与路径 → listenerType/listenerPath 拼串;删行清空为 null', async () => {
    const { store, node, current } = setup()
    const view = render(BetweenForm, { props: { store, node } })
    await fireEvent.click(screen.getByText('监听器'))
    await fireEvent.click(screen.getByText('+ 监听器'))
    await view.rerender({ store, node: current() })
    await pick('监听器类型', '完成')
    await view.rerender({ store, node: current() })
    await fireEvent.input(screen.getByLabelText('监听器路径'), {
      target: { value: 'leaveListener' },
    })
    expect(current().data.json).toMatchObject({
      listenerType: 'finish',
      listenerPath: 'leaveListener',
    })
    await view.rerender({ store, node: current() })
    await fireEvent.click(screen.getByLabelText('删除监听器'))
    expect(current().data.json).toMatchObject({
      listenerType: null,
      listenerPath: null,
    })
  })
})
