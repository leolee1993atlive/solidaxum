import { describe, expect, it } from 'vitest'
import { copySelection, pasteClipboard } from './clipboard.js'
import { newFlowNode, newSkipEdge, updateNodeJson } from './edit.js'
import type { FlowNode, SkipEdge } from './types.js'

const sel = <T extends FlowNode | SkipEdge>(x: T): T => ({
  ...x,
  selected: true,
})

/** start → a → b → end,a/b 之间还有一条驳回边;c 是 a 的驳回目标(anyNodeSkip)。 */
function graph() {
  const start = newFlowNode('start', { x: 0, y: 100 })
  const a = newFlowNode('between', { x: 200, y: 100 })
  const b = newFlowNode('between', { x: 400, y: 100 })
  const c = newFlowNode('between', { x: 400, y: 300 })
  const end = newFlowNode('end', { x: 600, y: 100 })
  const nodes = updateNodeJson(
    [start, a, b, c, end].map((n) => ({
      ...n,
      data: { ...n.data, json: { ...n.data.json, permissionFlag: 'u1@@u2' } },
    })),
    a.id,
    { anyNodeSkip: c.id }
  )
  const link = (s: FlowNode, t: FlowNode, edges: SkipEdge[]) =>
    newSkipEdge(
      {
        source: s.id,
        target: t.id,
        sourceHandle: 'right',
        targetHandle: 'left',
      },
      edges
    )
  const edges: SkipEdge[] = []
  edges.push(link(start, a, edges))
  edges.push(link(a, b, edges))
  edges.push({
    ...link(b, a, edges),
    data: {
      waypoints: [{ x: 300, y: 250 }],
      labelOffset: { x: 4, y: 4 },
      json: { skipName: '驳回', skipType: 'REJECT', skipCondition: null },
    },
  })
  edges.push(link(b, end, edges))
  return { start, a, b, c, end, nodes, edges }
}

describe('copySelection', () => {
  it('开始 / 结束不进剪贴板;边只带两端都在选集里的;快照是深拷贝', () => {
    const { start, a, b, end, nodes, edges } = graph()
    const picked = new Set([start.id, a.id, b.id, end.id])
    const clip = copySelection(
      nodes.map((n) => (picked.has(n.id) ? sel(n) : n)),
      edges
    )!
    expect(clip.nodes.map((n) => n.id)).toEqual([a.id, b.id])
    expect(clip.edges.map((e) => e.id)).toEqual([
      `${a.id}->${b.id}#0`,
      `${b.id}->${a.id}#0`,
    ])
    expect(clip.nodes[0].selected).toBeUndefined()
    expect(clip.nodes[0].data.json).not.toBe(a.data.json)
  })

  it('没有选中、或只选了开始 / 结束 → null', () => {
    const { start, nodes, edges } = graph()
    expect(copySelection(nodes, edges)).toBeNull()
    expect(
      copySelection(
        nodes.map((n) => (n.id === start.id ? sel(n) : n)),
        edges
      )
    ).toBeNull()
  })
})

describe('pasteClipboard', () => {
  it('新编码与 nodeCode 同步,名称与业务字段原样;外接框左上角对齐到吸附后的落点', () => {
    const { a, b, nodes, edges } = graph()
    const clip = copySelection(
      nodes.map((n) => (n.id === a.id || n.id === b.id ? sel(n) : n)),
      edges
    )!
    const r = pasteClipboard(clip, nodes, edges, { x: 1004, y: 507 })
    expect(r.nodes).toHaveLength(7)
    expect(r.pastedIds).toHaveLength(2)
    const [na, nb] = r.pastedIds.map((id) => r.nodes.find((n) => n.id === id)!)
    expect(na.id).not.toBe(a.id)
    expect(na.data.json.nodeCode).toBe(na.id)
    expect(na.data.json.nodeName).toBe('审批人')
    expect(na.data.json.permissionFlag).toBe('u1@@u2')
    // a 原位置 (150,60),b (350,60):左上角落到 (1000,510),b 保持相对偏移 200
    expect(na.position).toEqual({ x: 1000, y: 510 })
    expect(nb.position).toEqual({ x: 1200, y: 510 })
  })

  it('边按新旧 id 映射并回到自动路由;只有粘贴出来的元素是选中态', () => {
    const { a, b, nodes, edges } = graph()
    const selected = nodes.map((n) =>
      n.id === a.id || n.id === b.id ? sel(n) : n
    )
    const clip = copySelection(selected, edges)!
    const r = pasteClipboard(clip, selected, edges.map(sel), { x: 0, y: 0 })
    const [na, nb] = r.pastedIds
    const pastedEdges = r.edges.slice(edges.length)
    expect(pastedEdges.map((e) => e.id)).toEqual([
      `${na}->${nb}#0`,
      `${nb}->${na}#0`,
    ])
    expect(pastedEdges[1].data.waypoints).toBeNull()
    expect(pastedEdges[1].data.labelOffset).toEqual({ x: 4, y: 4 })
    expect(pastedEdges[1].data.json.skipType).toBe('REJECT')
    expect(r.nodes.filter((n) => n.selected).map((n) => n.id)).toEqual([na, nb])
    expect(r.edges.filter((e) => e.selected)).toEqual(pastedEdges)
  })

  it('anyNodeSkip:指向组内节点的映射到副本,指向组外的原样保留', () => {
    const { a, b, c, nodes, edges } = graph()
    // 只复制 a:a.anyNodeSkip = c 在组外 → 保留
    const only = pasteClipboard(
      copySelection(
        nodes.map((n) => (n.id === a.id ? sel(n) : n)),
        edges
      )!,
      nodes,
      edges,
      { x: 0, y: 0 }
    )
    expect(
      only.nodes.find((n) => n.id === only.pastedIds[0])!.data.json.anyNodeSkip
    ).toBe(c.id)
    // 复制 a + c:映射到 c 的副本
    const both = pasteClipboard(
      copySelection(
        nodes.map((n) => (n.id === a.id || n.id === c.id ? sel(n) : n)),
        edges
      )!,
      nodes,
      edges,
      { x: 0, y: 0 }
    )
    const [na, nc] = both.pastedIds
    expect(both.nodes.find((n) => n.id === na)!.data.json.anyNodeSkip).toBe(nc)
    expect(b.id).toBeTruthy()
  })

  it('连续粘贴两次:编码互不相同,第二次不与第一次撞 id', () => {
    const { a, b, nodes, edges } = graph()
    const clip = copySelection(
      nodes.map((n) => (n.id === a.id || n.id === b.id ? sel(n) : n)),
      edges
    )!
    const first = pasteClipboard(clip, nodes, edges, { x: 0, y: 0 })
    const second = pasteClipboard(clip, first.nodes, first.edges, {
      x: 0,
      y: 200,
    })
    const ids = second.nodes.map((n) => n.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(new Set(second.edges.map((e) => e.id)).size).toBe(
      second.edges.length
    )
    expect(second.nodes.filter((n) => n.selected)).toHaveLength(2)
  })
})
