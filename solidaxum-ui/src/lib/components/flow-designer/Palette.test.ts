import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import Fixture from './Palette.fixture.svelte'
import { NODE_DRAG_INSERTABLE, NODE_DRAG_MIME } from './node-menu.js'
import { DesignerStore } from './state/designer.svelte.js'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: { queryDef: vi.fn(), saveJson: vi.fn() },
}))

afterEach(cleanup)

function setup() {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  render(Fixture, { props: { store } })
  return store
}

/** happy-dom 的 DragEvent 构造器不认 dataTransfer 选项,手工挂一个 */
function dragEvent(type: string, dt: DataTransfer) {
  const e = new DragEvent(type, { bubbles: true, cancelable: true })
  Object.defineProperty(e, 'dataTransfer', { value: dt })
  return e
}

describe('Palette', () => {
  it('六种节点按固定顺序,都是可拖的 role=button', () => {
    setup()
    const items = screen.getAllByRole('button')
    // 图标 svg 是 aria-hidden,可访问名只剩文字;textContent 会把网关符号也带上
    expect(items.map((el) => el.lastElementChild?.textContent)).toEqual([
      '审批人',
      '互斥网关',
      '并行网关',
      '包容网关',
      '开始',
      '结束',
    ])
    for (const el of items) expect(el).toHaveAttribute('draggable', 'true')
  })

  it('起拖写入节点载荷;审批 / 网关额外带可插入标记,开始 / 结束不带', () => {
    setup()
    const dt = new DataTransfer()
    fireEvent(
      screen.getByRole('button', { name: '互斥网关' }),
      dragEvent('dragstart', dt)
    )
    expect(JSON.parse(dt.getData(NODE_DRAG_MIME))).toEqual({
      kind: 'gateway',
      nodeType: 3,
      label: '互斥网关',
    })
    expect(dt.types).toContain(NODE_DRAG_INSERTABLE)

    const dt2 = new DataTransfer()
    fireEvent(
      screen.getByRole('button', { name: '开始' }),
      dragEvent('dragstart', dt2)
    )
    expect(JSON.parse(dt2.getData(NODE_DRAG_MIME)).kind).toBe('start')
    expect(dt2.types).not.toContain(NODE_DRAG_INSERTABLE)
  })

  it('点击 = 加到视口中心并唯一选中,记一步历史;拖拽结束清掉候选边', async () => {
    const store = setup()
    const before = store.nodes.length
    await fireEvent.click(screen.getByRole('button', { name: '并行网关' }))
    expect(store.nodes).toHaveLength(before + 1)
    const added = store.nodes.at(-1)!
    expect(added.data.json.nodeType).toBe(4)
    expect(store.selectedNode?.id).toBe(added.id)
    expect(store.canUndo).toBe(true)

    store.dropTargetEdgeId = 'x'
    await fireEvent(
      screen.getByRole('button', { name: '并行网关' }),
      new DragEvent('dragend', { bubbles: true })
    )
    expect(store.dropTargetEdgeId).toBeNull()
  })
})
