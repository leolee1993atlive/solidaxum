import { designerApi } from '$lib/api/workflow/designer.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DesignerStore } from '../state/designer.svelte.js'
import HandlerList from './HandlerList.svelte'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: {
    queryDef: vi.fn(),
    saveJson: vi.fn(),
    handlerFeedback: vi.fn(),
    handlerType: vi.fn(),
    handlerResult: vi.fn(),
  },
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(designerApi.handlerFeedback).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: [{ storageId: 'user:1', handlerName: '张三' }],
  })
  vi.mocked(designerApi.handlerType).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: ['用户', '角色'],
  })
  vi.mocked(designerApi.handlerResult).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: { handlerAuths: { rows: [], total: 0 }, treeSelections: [] },
  })
})
afterEach(cleanup)

function setup(permissionFlag: string | null) {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  const id = store.nodes.find((n) => n.data.json.nodeName === '组长')!.id
  store.updateNodeJson(id, { permissionFlag })
  const current = () => store.nodes.find((n) => n.id === id)!
  return { store, current }
}

describe('HandlerList', () => {
  it('storageId 项去 handler-feedback 拿名字回显;表达式项原样显示、不查询', async () => {
    const { store, current } = setup('user:1@@${initiator}')
    render(HandlerList, { props: { store, node: current() } })
    await waitFor(() => expect(screen.getByText('张三')).toBeInTheDocument())
    expect(designerApi.handlerFeedback).toHaveBeenCalledWith(['user:1'])
    expect(screen.getByText('${initiator}')).toBeInTheDocument()
  })

  it('回显失败只显示原始串,不阻塞', async () => {
    vi.mocked(designerApi.handlerFeedback).mockRejectedValue(new Error('boom'))
    const { store, current } = setup('role:2')
    render(HandlerList, { props: { store, node: current() } })
    await waitFor(() => expect(designerApi.handlerFeedback).toHaveBeenCalled())
    expect(screen.getByText('role:2')).toBeInTheDocument()
  })

  it('手动添加与移除写回 permissionFlag(@@ 拼接,空时为 null)', async () => {
    const { store, current } = setup('user:1')
    const view = render(HandlerList, { props: { store, node: current() } })
    await fireEvent.input(screen.getByLabelText('办理人手动输入'), {
      target: { value: 'dept:3' },
    })
    await fireEvent.click(screen.getByText('添加'))
    expect(current().data.json.permissionFlag).toBe('user:1@@dept:3')
    await view.rerender({ store, node: current() })
    await fireEvent.click(screen.getByLabelText('移除 user:1'))
    expect(current().data.json.permissionFlag).toBe('dept:3')
    await view.rerender({ store, node: current() })
    await fireEvent.click(screen.getByLabelText('移除 dept:3'))
    expect(current().data.json.permissionFlag).toBeNull()
  })

  it('弹窗里什么都不改直接确定,已回显的名字不丢', async () => {
    const { store, current } = setup('user:1')
    render(HandlerList, { props: { store, node: current() } })
    await waitFor(() => expect(screen.getByText('张三')).toBeInTheDocument())
    await fireEvent.click(screen.getByText('选择'))
    await waitFor(() => expect(designerApi.handlerResult).toHaveBeenCalled())
    await fireEvent.click(screen.getByText('确定'))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    )
    expect(current().data.json.permissionFlag).toBe('user:1')
    expect(screen.getByText('张三')).toBeInTheDocument()
  })

  it('只读时没有添加/选择/移除', () => {
    const { store, current } = setup('user:1')
    render(HandlerList, { props: { store, node: current(), readonly: true } })
    expect(screen.queryByText('添加')).toBeNull()
    expect(screen.queryByText('选择')).toBeNull()
    expect(screen.queryByLabelText('移除 user:1')).toBeNull()
  })
})
