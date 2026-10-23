import { describe, expect, it } from 'vitest'
import { defJsonToFlow } from './deserialize.js'
import { updateEdgeJson, updateNodeJson } from './edit.js'
import leave6Json from './fixtures/leave6.json'
import type { DefJson } from './types.js'
import { validateDefinition } from './validate.js'

const load = () => defJsonToFlow(leave6Json as unknown as DefJson)
const byName = (nodes: ReturnType<typeof load>['nodes'], name: string) =>
  nodes.find((n) => n.data.json.nodeName === name)!

describe('validateDefinition', () => {
  it('leave6 原样:两条条件 warning(default@@ 引擎不认、spel 用 #{ } 包裹),没有 error', () => {
    const { nodes, edges } = load()
    const issues = validateDefinition(nodes, edges)
    expect(issues.filter((i) => i.level === 'error')).toEqual([])
    expect(issues).toHaveLength(2)
    expect(issues.map((i) => i.message)).toEqual([
      expect.stringContaining('大于两天'),
      expect.stringContaining('包裹'),
    ])
    expect(issues.every((i) => i.target.kind === 'edge')).toBe(true)
  })

  it('节点编码重复 / 为空是 error', () => {
    const { nodes, edges } = load()
    const dup = nodes.map((n, i) => (i === 2 ? { ...n, id: nodes[1].id } : n))
    // 重复的两个节点各报一条,面板才能逐个定位
    expect(
      validateDefinition(dup, edges).filter((i) => i.level === 'error')
    ).toHaveLength(2)
    const empty = nodes.map((n, i) => (i === 2 ? { ...n, id: '' } : n))
    expect(
      validateDefinition(empty, edges).some(
        (i) => i.level === 'error' && /为空/.test(i.message)
      )
    ).toBe(true)
  })

  it('票签策略非法、监听器半填、悬空节点都是 warning,并指向对应节点', () => {
    const { nodes, edges } = load()
    const applicant = byName(nodes, '申请人')
    let next = updateNodeJson(nodes, applicant.id, { nodeRatio: 'passCount=0' })
    next = updateNodeJson(next, byName(next, '组长').id, {
      listenerType: 'start',
      listenerPath: null,
    })
    const orphanEdges = edges.filter(
      (e) => e.target !== byName(next, '董事').id
    )
    const issues = validateDefinition(next, orphanEdges)
    expect(issues.every((i) => i.level === 'warning')).toBe(true)
    expect(issues.find((i) => /票签策略/.test(i.message))?.target).toEqual({
      kind: 'node',
      id: applicant.id,
    })
    expect(issues.some((i) => /监听器/.test(i.message))).toBe(true)
    expect(issues.some((i) => /董事:没有入线/.test(i.message))).toBe(true)
  })

  it('缺开始/结束节点是 warning;spel 用 #{ } 包裹是 warning', () => {
    const { nodes, edges } = load()
    const noEnd = nodes.filter((n) => n.type !== 'end')
    expect(
      validateDefinition(noEnd, edges).some((i) => /缺少结束/.test(i.message))
    ).toBe(true)
    const e = edges.find((ed) =>
      ed.data.json.skipCondition?.startsWith('spel@@')
    )!
    const wrapped = updateEdgeJson(edges, e.id, {
      skipCondition: 'spel@@#{#a > 1}',
    })
    expect(
      validateDefinition(nodes, wrapped).some((i) => /包裹/.test(i.message))
    ).toBe(true)
  })
})
