import leave6Json from '$lib/flow/fixtures/leave6.json'
import { NODE_SIZE } from '$lib/flow/geometry.js'
import type { DefJson, FlowNode, NodeKind } from '$lib/flow/types.js'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/svelte'
import { Position } from '@xyflow/svelte'
import type { NodeHandle } from '@xyflow/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import Fixture from './Canvas.fixture.svelte'
import { NODE_DRAG_INSERTABLE, NODE_DRAG_MIME } from './node-menu.js'
import { DesignerStore } from './state/designer.svelte.js'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: { queryDef: vi.fn(), saveJson: vi.fn() },
}))

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

/** 同 Canvas.test:happy-dom 没有布局,走 xyflow 的 SSR 路径让边画出来 */
function staticLayout(nodes: FlowNode[]): FlowNode[] {
  return nodes.map((n) => {
    const { w, h } = NODE_SIZE[n.type as NodeKind]
    const at = (
      id: string,
      position: Position,
      x: number,
      y: number
    ): NodeHandle => ({
      id,
      type: 'source',
      position,
      x: x - 5,
      y: y - 5,
      width: 10,
      height: 10,
    })
    return {
      ...n,
      initialWidth: w,
      initialHeight: h,
      handles: [
        at('top', Position.Top, w / 2, 0),
        at('right', Position.Right, w, h / 2),
        at('bottom', Position.Bottom, w / 2, h),
        at('left', Position.Left, 0, h / 2),
      ],
    }
  })
}

async function setup(readonly = false) {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  store.nodes = staticLayout(store.nodes)
  store.history.reset({ nodes: store.nodes, edges: store.edges })
  const view = render(Fixture, { props: { store, readonly } })
  await waitFor(() =>
    expect(view.container.querySelectorAll('.svelte-flow__edge').length).toBe(
      15
    )
  )
  const host = view.container.querySelector('.fd-canvas') as HTMLElement
  const nodeEl = (id: string) =>
    view.container.querySelector(
      `.svelte-flow__node[data-id="${id}"]`
    ) as HTMLElement
  const edgeEl = (id: string) =>
    view.container.querySelector(
      `.svelte-flow__edge[data-id="${id}"]`
    ) as HTMLElement
  return { store, host, nodeEl, edgeEl, ...view }
}

function dragEvent(type: string, dt: DataTransfer, x = 300, y = 200) {
  const e = new DragEvent(type, { bubbles: true, cancelable: true })
  Object.defineProperty(e, 'dataTransfer', { value: dt })
  Object.defineProperty(e, 'clientX', { value: x })
  Object.defineProperty(e, 'clientY', { value: y })
  return e
}

function payload(kind: NodeKind, nodeType: number, label: string) {
  const dt = new DataTransfer()
  dt.setData(NODE_DRAG_MIME, JSON.stringify({ kind, nodeType, label }))
  if (kind === 'between' || kind === 'gateway')
    dt.setData(NODE_DRAG_INSERTABLE, '')
  return dt
}

describe('右键菜单', () => {
  it('右键节点 → 唯一选中 + 复制/删除;右键空白 → 粘贴(置灰)/添加节点/适应画布', async () => {
    const { store, host, nodeEl } = await setup()
    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    await fireEvent.contextMenu(nodeEl(applicant.id), {
      clientX: 50,
      clientY: 60,
    })
    expect(store.selectedNode?.id).toBe(applicant.id)
    let items = screen
      .getAllByRole('menuitem')
      .map((el) => el.textContent?.replace(/\s+/g, ' ').trim())
    expect(items).toEqual(['复制 Ctrl+C', '删除 Delete'])

    await fireEvent.pointerDown(document.body)
    await fireEvent.contextMenu(host.querySelector('.svelte-flow__pane')!, {
      clientX: 400,
      clientY: 300,
    })
    items = screen
      .getAllByRole('menuitem')
      .map((el) => el.textContent?.replace(/\s+/g, ' ').trim())
    expect(items).toEqual([
      '粘贴 Ctrl+V',
      '添加审批人',
      '添加互斥网关',
      '添加并行网关',
      '添加包容网关',
      '添加开始',
      '添加结束',
      '适应画布',
    ])
    expect(screen.getByRole('menuitem', { name: /粘贴/ })).toBeDisabled()
  })

  it('开始节点的"复制"置灰;右键边 → 选中该边 + 删除', async () => {
    const { store, nodeEl, edgeEl } = await setup()
    const start = store.nodes.find((n) => n.type === 'start')!
    await fireEvent.contextMenu(nodeEl(start.id))
    expect(screen.getByRole('menuitem', { name: /复制/ })).toBeDisabled()
    await fireEvent.pointerDown(document.body)

    const edge = store.edges[0]
    await fireEvent.contextMenu(edgeEl(edge.id))
    expect(store.selectedEdge?.id).toBe(edge.id)
    expect(
      screen.getAllByRole('menuitem').map((el) => el.textContent?.trim())
    ).toEqual(['删除 Delete'])
  })

  it('菜单"添加审批人"落在右键位置并记历史;"复制"后空白菜单的"粘贴"可用', async () => {
    const { store, host, nodeEl } = await setup()
    const before = store.nodes.length
    await fireEvent.contextMenu(host.querySelector('.svelte-flow__pane')!)
    await fireEvent.click(screen.getByRole('menuitem', { name: '添加审批人' }))
    expect(store.nodes).toHaveLength(before + 1)
    expect(store.selectedNode?.data.json.nodeType).toBe(1)
    expect(store.canUndo).toBe(true)

    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    await fireEvent.contextMenu(nodeEl(applicant.id))
    await fireEvent.click(screen.getByRole('menuitem', { name: /复制/ }))
    expect(store.clipboard?.nodes.map((n) => n.id)).toEqual([applicant.id])
    await fireEvent.contextMenu(host.querySelector('.svelte-flow__pane')!)
    expect(screen.getByRole('menuitem', { name: /粘贴/ })).toBeEnabled()
    await fireEvent.click(screen.getByRole('menuitem', { name: /粘贴/ }))
    expect(store.nodes).toHaveLength(before + 2)
    expect(store.selectedNode?.data.json.nodeName).toBe('申请人')
    expect(store.selectedNode?.id).not.toBe(applicant.id)
  })

  it('只读画布没有编辑层:右键不弹菜单', async () => {
    const { store, nodeEl } = await setup(true)
    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    await fireEvent.contextMenu(nodeEl(applicant.id))
    expect(screen.queryByRole('menu')).toBeNull()
  })
})

describe('面板拖入', () => {
  it('拖过边时候选边高亮(只对可插入的种类);松手插入并保留条件', async () => {
    const { store, host, edgeEl } = await setup()
    const cond = store.edges.find((e) => e.data.json.skipCondition)!
    vi.spyOn(document, 'elementFromPoint').mockReturnValue(
      edgeEl(cond.id).querySelector('path')
    )

    await fireEvent(
      host,
      dragEvent('dragover', payload('between', 1, '审批人'))
    )
    expect(store.dropTargetEdgeId).toBe(cond.id)
    await waitFor(() =>
      expect(edgeEl(cond.id).querySelector('.fd-drop-target')).not.toBeNull()
    )

    // 开始节点拖过同一条边:不找候选边
    await fireEvent(host, dragEvent('dragover', payload('start', 0, '开始')))
    expect(store.dropTargetEdgeId).toBeNull()

    await fireEvent(
      host,
      dragEvent('dragover', payload('between', 1, '审批人'))
    )
    const before = { nodes: store.nodes.length, edges: store.edges.length }
    await fireEvent(host, dragEvent('drop', payload('between', 1, '审批人')))
    expect(store.dropTargetEdgeId).toBeNull()
    expect(store.nodes).toHaveLength(before.nodes + 1)
    expect(store.edges).toHaveLength(before.edges + 1)
    const added = store.selectedNode!
    const inbound = store.edges.find((e) => e.target === added.id)!
    expect(inbound.source).toBe(cond.source)
    expect(inbound.data.json.skipCondition).toBe(cond.data.json.skipCondition)
    expect(
      store.edges.some((e) => e.source === added.id && e.target === cond.target)
    ).toBe(true)
  })

  it('落在空白处 → 普通追加;不是节点载荷的拖放不处理', async () => {
    const { store, host } = await setup()
    vi.spyOn(document, 'elementFromPoint').mockReturnValue(null)
    const before = store.nodes.length
    await fireEvent(
      host,
      dragEvent('dragover', payload('gateway', 5, '包容网关'))
    )
    await fireEvent(host, dragEvent('drop', payload('gateway', 5, '包容网关')))
    expect(store.nodes).toHaveLength(before + 1)
    expect(store.selectedNode?.data.json.nodeType).toBe(5)

    const text = new DataTransfer()
    text.setData('text/plain', 'x')
    await fireEvent(host, dragEvent('drop', text))
    expect(store.nodes).toHaveLength(before + 1)
  })
})

describe('快捷键与重连锚', () => {
  it('Ctrl+Z / Ctrl+Y 撤销重做;焦点在输入框里不拦截', async () => {
    const { store } = await setup()
    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    store.updateNodeJson(applicant.id, { nodeName: '改名' })
    await fireEvent.keyDown(document.body, { key: 'z', ctrlKey: true })
    expect(
      store.nodes.find((n) => n.id === applicant.id)!.data.json.nodeName
    ).toBe('申请人')
    await fireEvent.keyDown(document.body, { key: 'y', ctrlKey: true })
    expect(
      store.nodes.find((n) => n.id === applicant.id)!.data.json.nodeName
    ).toBe('改名')

    const input = document.createElement('input')
    document.body.appendChild(input)
    await fireEvent.keyDown(input, { key: 'z', ctrlKey: true })
    expect(
      store.nodes.find((n) => n.id === applicant.id)!.data.json.nodeName
    ).toBe('改名')
    input.remove()
  })

  it('Ctrl+C / Ctrl+V:复制选中节点并粘贴到指针最后位置', async () => {
    const { store, host } = await setup()
    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    store.selectNode(applicant.id)
    await fireEvent.keyDown(document.body, { key: 'c', ctrlKey: true })
    expect(store.clipboard?.nodes).toHaveLength(1)
    await fireEvent.pointerMove(host, { clientX: 500, clientY: 400 })
    const before = store.nodes.length
    await fireEvent.keyDown(document.body, { key: 'v', metaKey: true })
    expect(store.nodes).toHaveLength(before + 1)
    expect(store.selectedNode?.id).not.toBe(applicant.id)
  })

  it('选中的边两端出现重连锚;只读画布没有', async () => {
    const { store, container } = await setup()
    expect(container.querySelectorAll('.fd-reconnect-anchor')).toHaveLength(0)
    store.selectEdge(store.edges[0].id)
    await waitFor(() =>
      expect(container.querySelectorAll('.fd-reconnect-anchor')).toHaveLength(2)
    )
    expect(
      container.querySelector('.svelte-flow__edgeupdater-source')
    ).not.toBeNull()
    expect(
      container.querySelector('.svelte-flow__edgeupdater-target')
    ).not.toBeNull()

    cleanup()
    const ro = await setup(true)
    ro.store.selectEdge(ro.store.edges[0].id)
    await waitFor(() =>
      expect(
        ro.container.querySelectorAll('.svelte-flow__edge.selected').length
      ).toBe(1)
    )
    expect(ro.container.querySelectorAll('.fd-reconnect-anchor')).toHaveLength(
      0
    )
  })
})
