import { describe, expect, it } from 'vitest'
import {
  clearWaypointsAround,
  insertNodeOnEdge,
  newFlowNode,
  newNodeCode,
  newSkipEdge,
  reconnectEdge,
  renameNodeCode,
  updateEdgeJson,
  updateNodeJson,
} from './edit.js'
import type { SkipEdge } from './types.js'

const edge = (
  source: string,
  target: string,
  waypoints: SkipEdge['data']['waypoints']
): SkipEdge => ({
  id: `${source}->${target}#0`,
  source,
  target,
  type: 'skip',
  data: {
    waypoints,
    labelOffset: { x: 0, y: 0 },
    json: { skipName: '', skipType: 'PASS', skipCondition: null },
  },
})

describe('newSkipEdge', () => {
  it('按连线的两端与句柄构造 PASS 边,自动路由、标签偏移 0', () => {
    const e = newSkipEdge(
      { source: 'a', target: 'b', sourceHandle: 'right', targetHandle: 'left' },
      []
    )
    expect(e).toMatchObject({
      id: 'a->b#0',
      source: 'a',
      target: 'b',
      sourceHandle: 'right',
      targetHandle: 'left',
      type: 'skip',
    })
    expect(e.data.waypoints).toBeNull()
    expect(e.data.labelOffset).toEqual({ x: 0, y: 0 })
    expect(e.data.json).toEqual({
      skipName: '',
      skipType: 'PASS',
      skipCondition: null,
    })
  })

  it('同向第二条边序号递增,与 deserialize 的编号一致', () => {
    const first = newSkipEdge(
      { source: 'a', target: 'b', sourceHandle: 'right', targetHandle: 'left' },
      []
    )
    const second = newSkipEdge(
      { source: 'a', target: 'b', sourceHandle: 'bottom', targetHandle: 'top' },
      [first]
    )
    expect(second.id).toBe('a->b#1')
    // 反向是另一对,从 0 起
    expect(
      newSkipEdge(
        { source: 'b', target: 'a', sourceHandle: null, targetHandle: null },
        [first, second]
      ).id
    ).toBe('b->a#0')
  })

  it('删掉中间序号后再连,取首个空闲号而不是计数(撞 id 会破坏 xyflow 的 keyed 渲染)', () => {
    const c = {
      source: 'a',
      target: 'b',
      sourceHandle: null,
      targetHandle: null,
    }
    const first = newSkipEdge(c, [])
    const second = newSkipEdge(c, [first])
    // 只剩 #1 时计数法会再发一个 #1;首个空闲号补 #0
    expect(newSkipEdge(c, [second]).id).toBe('a->b#0')
  })

  it('句柄为 null 时不写 sourceHandle/targetHandle(保存时回落相向侧)', () => {
    const e = newSkipEdge(
      { source: 'a', target: 'b', sourceHandle: null, targetHandle: null },
      []
    )
    expect(e.sourceHandle).toBeUndefined()
    expect(e.targetHandle).toBeUndefined()
  })
})

describe('clearWaypointsAround', () => {
  it('只清与被移动节点相连且有存量折线的边', () => {
    const edges = [
      edge('a', 'b', [{ x: 1, y: 1 }]),
      edge('b', 'c', [{ x: 2, y: 2 }]),
      edge('c', 'd', []),
      edge('d', 'e', null),
    ]
    const out = clearWaypointsAround(edges, ['b'])
    expect(out.map((e) => e.data.waypoints)).toEqual([null, null, [], null])
    // 未受影响的边保持同一引用,Svelte Flow 不会无谓重渲染
    expect(out[2]).toBe(edges[2])
    expect(out[3]).toBe(edges[3])
  })

  it('没有任何边需要改时返回原数组引用', () => {
    const edges = [edge('a', 'b', null), edge('b', 'c', [])]
    expect(clearWaypointsAround(edges, ['a'])).toBe(edges)
    expect(clearWaypointsAround(edges, ['zzz'])).toBe(edges)
  })
})

describe('newFlowNode / newNodeCode', () => {
  it('中心点换算成左上角,默认标签偏移按种类', () => {
    const n = newFlowNode('gateway', { x: 700, y: 300 })
    expect(n.type).toBe('gateway')
    expect(n.position).toEqual({ x: 675, y: 275 })
    expect(n.data.labelOffset).toEqual({ x: 0, y: 40 })
    expect(n.data.json.nodeType).toBe(3)
    expect(n.data.json.nodeName).toBe('互斥网关')
    expect(n.id).toBe(n.data.json.nodeCode)
  })

  it('网关可指定 nodeType(并行 4 / 包容 5),其余字段为空', () => {
    const n = newFlowNode('gateway', { x: 0, y: 0 }, 4)
    expect(n.data.json.nodeType).toBe(4)
    expect(n.data.json.permissionFlag).toBeNull()
    expect(n.data.json.ext).toBeNull()
  })

  it('between 100×80:中心 (400,300) → 左上角 (350,260)', () => {
    expect(newFlowNode('between', { x: 400, y: 300 }).position).toEqual({
      x: 350,
      y: 260,
    })
  })

  it('节点编码定义内唯一', () => {
    const codes = new Set(Array.from({ length: 50 }, () => newNodeCode()))
    expect(codes.size).toBe(50)
  })
})

describe('updateNodeJson / updateEdgeJson', () => {
  it('只替换目标对象,其余引用不变;补丁合并进 json', () => {
    const a = newFlowNode('between', { x: 0, y: 0 })
    const b = newFlowNode('between', { x: 100, y: 0 })
    const out = updateNodeJson([a, b], b.id, {
      nodeName: '改名',
      nodeRatio: '50',
    })
    expect(out[0]).toBe(a)
    expect(out[1]).not.toBe(b)
    expect(out[1].data.json).toMatchObject({
      nodeName: '改名',
      nodeRatio: '50',
      nodeCode: b.id,
    })
    expect(b.data.json.nodeName).toBe('审批人')
    const e = edge('a', 'b', null)
    expect(
      updateEdgeJson([e], 'a->b#0', { skipType: 'REJECT' })[0].data.json
        .skipType
    ).toBe('REJECT')
  })
})

describe('renameNodeCode', () => {
  const setup = () => {
    const a = newFlowNode('between', { x: 0, y: 0 })
    const b = newFlowNode('between', { x: 100, y: 0 })
    const c = newFlowNode('between', { x: 200, y: 0 })
    const nodes = updateNodeJson([a, b, c], c.id, { anyNodeSkip: b.id })
    const edges = [
      edge(a.id, b.id, null),
      edge(b.id, c.id, [{ x: 1, y: 1 }]),
      edge(a.id, c.id, null),
    ]
    return { a, b, c, nodes, edges }
  }

  it('改 id / nodeCode / 相连边两端与 id / 其它节点的 anyNodeSkip 引用', () => {
    const { a, b, c, nodes, edges } = setup()
    const r = renameNodeCode(nodes, edges, b.id, 'approve_1')
    if ('error' in r) throw new Error(r.error)
    const nb = r.nodes.find((n) => n.data.json.nodeCode === 'approve_1')!
    expect(nb.id).toBe('approve_1')
    expect(r.nodes.find((n) => n.id === c.id)!.data.json.anyNodeSkip).toBe(
      'approve_1'
    )
    expect(r.edges.map((e) => e.id)).toEqual([
      `${a.id}->approve_1#0`,
      `approve_1->${c.id}#0`,
      `${a.id}->${c.id}#0`,
    ])
    expect(r.edges[1].data.waypoints).toEqual([{ x: 1, y: 1 }]) // 几何不动
    expect(r.edges[2]).toBe(edges[2]) // 无关边引用不变
  })

  it('空、重复、不存在 → 返回错误,不改数据', () => {
    const { a, b, nodes, edges } = setup()
    expect(renameNodeCode(nodes, edges, b.id, '  ')).toEqual({
      error: expect.stringMatching(/为空/),
    })
    expect(renameNodeCode(nodes, edges, b.id, a.id)).toEqual({
      error: expect.stringMatching(/已存在/),
    })
    expect(renameNodeCode(nodes, edges, 'ghost', 'x')).toEqual({
      error: expect.stringMatching(/不存在/),
    })
    expect(renameNodeCode(nodes, edges, b.id, b.id)).toEqual({ nodes, edges })
  })
})

describe('insertNodeOnEdge', () => {
  // 网关 (200,200) --条件 A--> 审批 X (500,200);把审批 Y 落在两者之间 (350,200) 的线上
  const setup = () => {
    const gw = newFlowNode('gateway', { x: 200, y: 200 }, 3)
    const x = newFlowNode('between', { x: 500, y: 200 })
    const e: SkipEdge = {
      ...edge(gw.id, x.id, [{ x: 300, y: 300 }]),
      sourceHandle: 'right',
      targetHandle: 'left',
      data: {
        waypoints: [{ x: 300, y: 300 }],
        labelOffset: { x: 5, y: -8 },
        json: {
          skipName: '同意',
          skipType: 'PASS',
          skipCondition: 'eq@@flag@@1',
        },
      },
    }
    const y = newFlowNode('between', { x: 350, y: 200 })
    return { gw, x, y, e }
  }

  it('原边改指向新节点并保留条件;新节点到原目标是一条新的 PASS 边', () => {
    const { gw, x, y, e } = setup()
    const r = insertNodeOnEdge([gw, x], [e], e.id, y)
    if ('error' in r) throw new Error(r.error)
    expect(r.nodes.map((n) => n.id)).toEqual([gw.id, x.id, y.id])
    expect(r.edges).toHaveLength(2)
    const [inbound, outbound] = r.edges
    expect(inbound).toMatchObject({
      id: `${gw.id}->${y.id}#0`,
      source: gw.id,
      target: y.id,
      sourceHandle: 'right', // 源侧句柄不动
      targetHandle: 'left', // 新节点朝向网关的一侧
    })
    expect(inbound.data.json).toEqual(e.data.json) // 条件 / 名称 / 类型原样
    expect(inbound.data.waypoints).toBeNull()
    expect(inbound.data.labelOffset).toEqual({ x: 0, y: 0 })
    expect(outbound).toMatchObject({
      id: `${y.id}->${x.id}#0`,
      source: y.id,
      target: x.id,
      sourceHandle: 'right', // 新节点朝向原目标的一侧
      targetHandle: 'left', // 沿用原边的目标句柄
    })
    expect(outbound.data.json).toEqual({
      skipName: '',
      skipType: 'PASS',
      skipCondition: null,
    })
  })

  it('原边留在原来的位置,新边追加在末尾(skipList 顺序稳定)', () => {
    const { gw, x, y, e } = setup()
    const other = edge(x.id, gw.id, null)
    const r = insertNodeOnEdge([gw, x], [e, other], e.id, y)
    if ('error' in r) throw new Error(r.error)
    expect(r.edges.map((it) => it.id)).toEqual([
      `${gw.id}->${y.id}#0`,
      other.id,
      `${y.id}->${x.id}#0`,
    ])
    expect(r.edges[1]).toBe(other)
  })

  it('原边没有句柄时新边也不写目标句柄(保存时回落相向侧)', () => {
    const { gw, x, y } = setup()
    const bare = edge(gw.id, x.id, null)
    const r = insertNodeOnEdge([gw, x], [bare], bare.id, y)
    if ('error' in r) throw new Error(r.error)
    expect(r.edges[1].targetHandle).toBeUndefined()
  })

  it('开始 / 结束、不存在的边、重复编码 → 返回错误,不改数据', () => {
    const { gw, x, y, e } = setup()
    expect(
      insertNodeOnEdge([gw, x], [e], e.id, newFlowNode('start', { x: 0, y: 0 }))
    ).toEqual({ error: '开始 / 结束节点不能插在跳转中间' })
    expect(
      insertNodeOnEdge([gw, x], [e], e.id, newFlowNode('end', { x: 0, y: 0 }))
    ).toHaveProperty('error')
    expect(insertNodeOnEdge([gw, x], [e], 'nope', y)).toEqual({
      error: '跳转 nope 不存在',
    })
    expect(insertNodeOnEdge([gw, x], [e], e.id, { ...y, id: x.id })).toEqual({
      error: `节点编码 ${x.id} 已存在`,
    })
  })
})

describe('reconnectEdge', () => {
  const setup = () => {
    const a = newFlowNode('between', { x: 0, y: 0 })
    const b = newFlowNode('between', { x: 300, y: 0 })
    const c = newFlowNode('between', { x: 300, y: 300 })
    const e: SkipEdge = {
      ...edge(a.id, b.id, [{ x: 150, y: 0 }]),
      sourceHandle: 'right',
      targetHandle: 'left',
      data: {
        waypoints: [{ x: 150, y: 0 }],
        labelOffset: { x: 3, y: 3 },
        json: { skipName: '驳回', skipType: 'REJECT', skipCondition: 'x' },
      },
    }
    return { a, b, c, e }
  }

  it('两端与句柄都没变 → false', () => {
    const { a, b, e } = setup()
    expect(
      reconnectEdge([e], e, {
        source: a.id,
        target: b.id,
        sourceHandle: 'right',
        targetHandle: 'left',
      })
    ).toBe(false)
  })

  it('目标端换到别的节点:id 按新两端重编,json 保留,路径与偏移归零', () => {
    const { a, b, c, e } = setup()
    const r = reconnectEdge([e], e, {
      source: a.id,
      target: c.id,
      sourceHandle: 'right',
      targetHandle: 'top',
    })
    expect(r).toMatchObject({
      id: `${a.id}->${c.id}#0`,
      source: a.id,
      target: c.id,
      sourceHandle: 'right',
      targetHandle: 'top',
    })
    if (!r) throw new Error('unreachable')
    expect(r.data.json).toEqual(e.data.json)
    expect(r.data.waypoints).toBeNull()
    expect(r.data.labelOffset).toEqual({ x: 0, y: 0 })
    expect(b.id).not.toBe(c.id)
  })

  it('只换句柄也算变化;空闲号排除自己,不与自己撞号', () => {
    const { a, b, e } = setup()
    const r = reconnectEdge([e], e, {
      source: a.id,
      target: b.id,
      sourceHandle: 'bottom',
      targetHandle: 'left',
    })
    if (!r) throw new Error('unreachable')
    expect(r.id).toBe(`${a.id}->${b.id}#0`)
    expect(r.sourceHandle).toBe('bottom')
  })

  it('换到已有同向边的一对节点上 → 取下一个空闲号', () => {
    const { a, b, c, e } = setup()
    const existing = edge(a.id, c.id, null) // a->c#0 已占
    const r = reconnectEdge([e, existing], e, {
      source: a.id,
      target: c.id,
      sourceHandle: null,
      targetHandle: null,
    })
    if (!r) throw new Error('unreachable')
    expect(r.id).toBe(`${a.id}->${c.id}#1`)
    expect(r.sourceHandle).toBeUndefined()
  })
})
