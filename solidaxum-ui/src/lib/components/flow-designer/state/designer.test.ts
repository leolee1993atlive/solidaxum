import { designerApi } from '$lib/api/workflow/designer.js'
import { parseNodeCoord } from '$lib/flow/coordinate.js'
import { newFlowNode } from '$lib/flow/edit.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { DesignerStore } from './designer.svelte.js'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: {
    queryDef: vi.fn(),
    saveJson: vi.fn(),
  },
}))

const leave6 = leave6Json as unknown as DefJson

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(designerApi.queryDef).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: leave6,
  })
  vi.mocked(designerApi.saveJson).mockResolvedValue({ code: 200, msg: 'ok' })
})

describe('DesignerStore.load', () => {
  it('按 id 拉定义,画布数组与定义级字段就位,加载态复位', async () => {
    const s = new DesignerStore()
    const p = s.load('7')
    expect(s.loading).toBe(true)
    await p
    expect(designerApi.queryDef).toHaveBeenCalledWith('7')
    expect(s.loading).toBe(false)
    expect(s.nodes).toHaveLength(14)
    expect(s.edges).toHaveLength(15)
    expect(s.base?.flowCode).toBe('leave6')
    expect(s.warnings).toEqual([])
  })

  it('后端没给 data → 抛错且加载态复位', async () => {
    vi.mocked(designerApi.queryDef).mockResolvedValue({ code: 200, msg: 'ok' })
    const s = new DesignerStore()
    await expect(s.load('7')).rejects.toThrow(/为空/)
    expect(s.loading).toBe(false)
    expect(s.base).toBeNull()
  })
})

describe('DesignerStore 脏检查与保存', () => {
  it('刚加载不脏;选中不算改动;移动节点算改动', async () => {
    const s = new DesignerStore()
    await s.load('7')
    expect(s.isDirty()).toBe(false)
    s.nodes = s.nodes.map((n, i) => (i === 0 ? { ...n, selected: true } : n))
    expect(s.isDirty()).toBe(false)
    s.nodes = s.nodes.map((n, i) =>
      i === 1
        ? { ...n, position: { x: n.position.x + 10, y: n.position.y } }
        : n
    )
    expect(s.isDirty()).toBe(true)
  })

  it('save 发送实时几何反算的 DefJson(onlyNodeSkip 默认),保存后不再脏', async () => {
    const s = new DesignerStore()
    await s.load('7')
    const applicant = s.nodes.find((n) => n.data.json.nodeName === '申请人')!
    s.nodes = s.nodes.map((n) =>
      n.id === applicant.id
        ? { ...n, position: { x: n.position.x + 10, y: n.position.y - 30 } }
        : n
    )
    await s.save()
    expect(designerApi.saveJson).toHaveBeenCalledTimes(1)
    const sent = vi.mocked(designerApi.saveJson).mock.calls[0][0]
    expect(sent.modelValue).toBe('CLASSICS')
    const moved = sent.nodeList.find((n) => n.nodeName === '申请人')!
    expect(parseNodeCoord(moved.coordinate)!.center).toEqual({ x: 410, y: 270 })
    expect(s.isDirty()).toBe(false)
    expect(s.saving).toBe(false)
  })

  it('保存失败:saving 复位,仍然是脏的', async () => {
    vi.mocked(designerApi.saveJson).mockRejectedValue(new Error('boom'))
    const s = new DesignerStore()
    await s.load('7')
    s.nodes = s.nodes.map((n, i) =>
      i === 1
        ? { ...n, position: { x: n.position.x + 10, y: n.position.y } }
        : n
    )
    await expect(s.save()).rejects.toThrow('boom')
    expect(s.saving).toBe(false)
    expect(s.isDirty()).toBe(true)
  })

  it('未加载就 toDefJson → 抛错', () => {
    expect(() => new DesignerStore().toDefJson()).toThrow(/尚未加载/)
  })
})

describe('DesignerStore · 面板方法', () => {
  it('selectedNode / selectedEdge 跟随 selected 标记;selectEdge 会清掉节点选中', async () => {
    const s = new DesignerStore()
    await s.load('7')
    expect(s.selectedNode).toBeUndefined()
    s.selectNode(s.nodes[1].id)
    expect(s.selectedNode?.id).toBe(s.nodes[1].id)
    s.selectEdge(s.edges[0].id)
    expect(s.selectedEdge?.id).toBe(s.edges[0].id)
    expect(s.selectedNode).toBeUndefined()
    s.selectNode(s.nodes[2].id)
    expect(s.selectedEdge).toBeUndefined()
  })

  it('updateNodeJson / updateEdgeJson 替换数组,算作改动', async () => {
    const s = new DesignerStore()
    await s.load('7')
    const before = s.nodes
    s.updateNodeJson(s.nodes[1].id, { nodeName: '改' })
    expect(s.nodes).not.toBe(before)
    expect(s.nodes[1].data.json.nodeName).toBe('改')
    expect(s.isDirty()).toBe(true)
    s.updateEdgeJson(s.edges[0].id, { skipName: 'x' })
    expect(s.edges[0].data.json.skipName).toBe('x')
  })

  it('renameNodeCode 成功返回 null 并连带改边;失败返回文案且不改', async () => {
    const s = new DesignerStore()
    await s.load('7')
    const target = s.nodes.find((n) => n.data.json.nodeName === '申请人')!
    expect(s.renameNodeCode(target.id, 'applicant')).toBeNull()
    expect(s.nodes.some((n) => n.id === 'applicant')).toBe(true)
    expect(
      s.edges.some((e) => e.source === 'applicant' || e.target === 'applicant')
    ).toBe(true)
    const snapshot = s.nodes
    expect(s.renameNodeCode('applicant', s.nodes[0].id)).toMatch(/已存在/)
    expect(s.nodes).toBe(snapshot)
  })

  it('validate 汇总 lib/flow/validate 的结果', async () => {
    const s = new DesignerStore()
    await s.load('7')
    expect(s.validate().map((i) => i.level)).toEqual(['warning', 'warning'])
  })
})

describe('DesignerStore 历史(撤销 / 重做)', () => {
  const byName = (s: DesignerStore, name: string) =>
    s.nodes.find((n) => n.data.json.nodeName === name)!

  it('刚加载不可撤销;commit 后可撤销;undo 整图回退并清掉选中,redo 再回来', async () => {
    const s = new DesignerStore()
    await s.load('7')
    expect(s.canUndo).toBe(false)
    const applicant = byName(s, '申请人')
    s.nodes = s.nodes.map((n) =>
      n.id === applicant.id
        ? { ...n, selected: true, position: { x: 999, y: 999 } }
        : n
    )
    s.commit()
    expect(s.canUndo).toBe(true)
    s.undo()
    expect(byName(s, '申请人').position).toEqual(applicant.position)
    expect(s.selectedNode).toBeUndefined()
    expect(s.canRedo).toBe(true)
    s.redo()
    expect(byName(s, '申请人').position).toEqual({ x: 999, y: 999 })
  })

  it('undo 保留当前节点的 measured(xyflow 不必重测)', async () => {
    const s = new DesignerStore()
    await s.load('7')
    s.nodes = s.nodes.map((n) => ({
      ...n,
      measured: { width: 100, height: 80 },
    }))
    s.nodes = s.nodes.map((n, i) =>
      i === 0 ? { ...n, position: { x: 1, y: 1 } } : n
    )
    s.commit()
    s.undo()
    expect(s.nodes[0].measured).toEqual({ width: 100, height: 80 })
  })

  it('面板写回:同一字段连续输入合并成一步;换字段是新的一步', async () => {
    const s = new DesignerStore()
    await s.load('7')
    const id = byName(s, '申请人').id
    s.updateNodeJson(id, { nodeName: '申' })
    s.updateNodeJson(id, { nodeName: '申请' })
    s.updateNodeJson(id, { nodeName: '申请部门' })
    s.updateNodeJson(id, { formPath: '/x' })
    s.undo()
    expect(s.nodes.find((n) => n.id === id)!.data.json).toMatchObject({
      nodeName: '申请部门',
      formPath: null,
    })
    s.undo()
    expect(s.nodes.find((n) => n.id === id)!.data.json.nodeName).toBe('申请人')
    expect(s.canUndo).toBe(false)
  })

  it('交互进行中忽略撤销重做', async () => {
    const s = new DesignerStore()
    await s.load('7')
    s.updateNodeJson(byName(s, '申请人').id, { nodeName: 'x' })
    s.interacting = true
    s.undo()
    expect(byName(s, 'x')).toBeDefined()
    s.interacting = false
    s.undo()
    expect(byName(s, '申请人')).toBeDefined()
  })

  it('保存不清历史;撤回到保存时的状态就不脏', async () => {
    const s = new DesignerStore()
    await s.load('7')
    s.updateNodeJson(byName(s, '申请人').id, { nodeName: '改名' })
    await s.save()
    expect(s.isDirty()).toBe(false)
    expect(s.canUndo).toBe(true)
    s.undo()
    expect(s.isDirty()).toBe(true)
    s.redo()
    expect(s.isDirty()).toBe(false)
  })
})

describe('DesignerStore 画布编辑入口', () => {
  const byName = (s: DesignerStore, name: string) =>
    s.nodes.find((n) => n.data.json.nodeName === name)!

  it('addNode 追加并唯一选中,记一步', async () => {
    const s = new DesignerStore()
    await s.load('7')
    s.selectNode(byName(s, '申请人').id)
    const node = newFlowNode('between', { x: 800, y: 300 })
    s.addNode(node)
    expect(s.nodes.at(-1)!.id).toBe(node.id)
    expect(s.selectedNode?.id).toBe(node.id)
    expect(s.canUndo).toBe(true)
    s.undo()
    expect(s.nodes.some((n) => n.id === node.id)).toBe(false)
  })

  it('insertNodeOnEdge 保留原边条件并选中新节点;失败返回文案', async () => {
    const s = new DesignerStore()
    await s.load('7')
    const withCond = s.edges.find((e) => e.data.json.skipCondition)!
    const node = newFlowNode('between', { x: 700, y: 100 })
    expect(s.insertNodeOnEdge(withCond.id, node)).toBeNull()
    const inbound = s.edges.find((e) => e.target === node.id)!
    expect(inbound.source).toBe(withCond.source)
    expect(inbound.data.json.skipCondition).toBe(
      withCond.data.json.skipCondition
    )
    expect(
      s.edges.some((e) => e.source === node.id && e.target === withCond.target)
    ).toBe(true)
    expect(s.selectedNode?.id).toBe(node.id)
    expect(
      s.insertNodeOnEdge('nope', newFlowNode('between', { x: 0, y: 0 }))
    ).toMatch(/不存在/)
  })

  it('copy / paste:选集进剪贴板,粘贴到给定点并记一步;没有可复制的返回 false', async () => {
    const s = new DesignerStore()
    await s.load('7')
    expect(s.copy()).toBe(false)
    expect(s.clipboard).toBeNull()
    s.selectNode(byName(s, '申请人').id)
    expect(s.copy()).toBe(true)
    expect(s.clipboard?.nodes).toHaveLength(1)
    const before = s.nodes.length
    s.paste({ x: 1000, y: 1000 })
    expect(s.nodes).toHaveLength(before + 1)
    expect(s.selectedNode?.position).toEqual({ x: 1000, y: 1000 })
    expect(s.selectedNode?.data.json.nodeName).toBe('申请人')
    s.undo()
    expect(s.nodes).toHaveLength(before)
  })
})
