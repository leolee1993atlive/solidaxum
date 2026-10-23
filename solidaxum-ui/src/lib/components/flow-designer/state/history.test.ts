import { newFlowNode } from '$lib/flow/edit.js'
import type { Graph } from '$lib/flow/snapshot.js'
import { describe, expect, it } from 'vitest'
import { COALESCE_WINDOW_MS, HISTORY_LIMIT, History } from './history.svelte.js'

/** 第 i 步:一个节点在 x = i 处。 */
const step = (i: number): Graph => ({
  nodes: [
    {
      ...newFlowNode('between', { x: 0, y: 0 }),
      id: 'n',
      position: { x: i, y: 0 },
    },
  ],
  edges: [],
})
const xOf = (g: Graph | null) => g?.nodes[0].position.x

describe('History', () => {
  it('reset 后没有可撤销;push 后可撤销;undo / redo 按顺序来回', () => {
    const h = new History()
    h.reset(step(0))
    expect(h.canUndo).toBe(false)
    expect(h.undo()).toBeNull()

    h.push(step(1))
    h.push(step(2))
    expect(h.canUndo).toBe(true)
    expect(h.canRedo).toBe(false)
    expect(xOf(h.undo())).toBe(1)
    expect(xOf(h.undo())).toBe(0)
    expect(h.canUndo).toBe(false)
    expect(h.undo()).toBeNull()
    expect(xOf(h.redo())).toBe(1)
    expect(xOf(h.redo())).toBe(2)
    expect(h.canRedo).toBe(false)
    expect(h.redo()).toBeNull()
  })

  it('撤销后再提交 → redo 栈清空', () => {
    const h = new History()
    h.reset(step(0))
    h.push(step(1))
    h.undo()
    expect(h.canRedo).toBe(true)
    h.push(step(5))
    expect(h.canRedo).toBe(false)
    expect(xOf(h.undo())).toBe(0)
  })

  it('同一合并键在窗口内连续提交合并成一步;换键或超时就是新的一步', () => {
    const h = new History()
    h.reset(step(0))
    h.push(step(1), 'node:n:nodeName', 1000)
    h.push(step(2), 'node:n:nodeName', 1500)
    h.push(step(3), 'node:n:nodeName', 1900)
    expect(xOf(h.undo())).toBe(0) // 三次输入是一步
    expect(xOf(h.redo())).toBe(3)

    h.push(step(4), 'node:n:nodeName', 1900 + COALESCE_WINDOW_MS) // 刚好超时
    expect(xOf(h.undo())).toBe(3)
    h.redo()
    h.push(step(5), 'node:n:formPath', 1901 + COALESCE_WINDOW_MS) // 换键
    expect(xOf(h.undo())).toBe(4)
  })

  it('不带键的提交从不合并,也切断后面的合并链', () => {
    const h = new History()
    h.reset(step(0))
    h.push(step(1), undefined, 1000)
    h.push(step(2), undefined, 1001)
    expect(xOf(h.undo())).toBe(1)
    h.redo()
    h.push(step(3), 'k', 1002)
    h.push(step(4), undefined, 1003)
    h.push(step(5), 'k', 1004) // 上一步无键,不能并回第 3 步
    expect(xOf(h.undo())).toBe(4)
  })

  it('撤销之后同键提交不合并(撤销切断合并链)', () => {
    const h = new History()
    h.reset(step(0))
    h.push(step(1), 'k', 1000)
    h.undo()
    h.push(step(2), 'k', 1001)
    expect(xOf(h.undo())).toBe(0)
  })

  it(`超过 ${HISTORY_LIMIT} 步丢最旧`, () => {
    const h = new History()
    h.reset(step(0))
    for (let i = 1; i <= HISTORY_LIMIT + 5; i += 1) h.push(step(i))
    let last: Graph | null = null
    let n = 0
    for (let g = h.undo(); g; g = h.undo()) {
      last = g
      n += 1
    }
    expect(n).toBe(HISTORY_LIMIT)
    expect(xOf(last)).toBe(5)
  })

  it('快照是深拷贝:提交后改原对象、或改撤销返回的对象,都不影响栈里的内容', () => {
    const h = new History()
    const g0 = step(0)
    h.reset(g0)
    const g1 = step(1)
    h.push(g1)
    g1.nodes[0].position.x = 999
    g0.nodes[0].data.json.nodeName = '改过了'
    const back = h.undo()!
    expect(back.nodes[0].data.json.nodeName).toBe('审批人')
    back.nodes[0].position.x = 777
    expect(xOf(h.redo())).toBe(1)
    expect(xOf(h.undo())).toBe(0)
  })
})
