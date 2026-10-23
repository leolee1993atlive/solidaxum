import { describe, expect, it } from 'vitest'
import { newFlowNode, newSkipEdge } from './edit.js'
import { cloneGraph } from './snapshot.js'

describe('cloneGraph', () => {
  it('只留数据字段:selected / dragging / measured 不进快照', () => {
    const a = {
      ...newFlowNode('between', { x: 100, y: 100 }),
      selected: true,
      dragging: true,
      measured: { width: 100, height: 80 },
    }
    const e = {
      ...newSkipEdge(
        {
          source: a.id,
          target: 'x',
          sourceHandle: 'right',
          targetHandle: null,
        },
        []
      ),
      selected: true,
    }
    const g = cloneGraph({ nodes: [a], edges: [e] })
    expect(Object.keys(g.nodes[0]).sort()).toEqual([
      'data',
      'id',
      'position',
      'type',
    ])
    expect(Object.keys(g.edges[0]).sort()).toEqual([
      'data',
      'id',
      'source',
      'sourceHandle',
      'target',
      'targetHandle',
      'type',
    ])
    expect(g.edges[0].sourceHandle).toBe('right')
  })

  it('深拷贝:改原对象的 json 不影响快照', () => {
    const a = newFlowNode('between', { x: 0, y: 0 })
    const g = cloneGraph({ nodes: [a], edges: [] })
    a.data.json.nodeName = '改过了'
    a.position.x = 999
    expect(g.nodes[0].data.json.nodeName).toBe('审批人')
    expect(g.nodes[0].position.x).toBe(-50)
  })
})
