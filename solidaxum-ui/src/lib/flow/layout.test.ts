import { describe, expect, it } from 'vitest'
import { dagreLayout } from './layout.js'

describe('dagreLayout', () => {
  const chain = [
    { id: 'start', kind: 'start' as const },
    { id: 'a', kind: 'between' as const },
    { id: 'gw', kind: 'gateway' as const },
    { id: 'end', kind: 'end' as const },
  ]
  const edges = [
    { source: 'start', target: 'a' },
    { source: 'a', target: 'gw' },
    { source: 'gw', target: 'end' },
  ]

  it('从左到右分层:沿链路 x 严格递增,且都是整数', () => {
    const c = dagreLayout(chain, edges)
    const xs = chain.map((n) => c.get(n.id)!.x)
    for (let i = 1; i < xs.length; i++) expect(xs[i]).toBeGreaterThan(xs[i - 1])
    for (const p of c.values()) {
      expect(Number.isInteger(p.x) && Number.isInteger(p.y)).toBe(true)
    }
  })

  it('每个节点都拿到中心点,且互不重叠(相邻中心距 ≥ 节点宽)', () => {
    const c = dagreLayout(chain, edges)
    expect(c.size).toBe(chain.length)
    expect(c.get('a')!.x - c.get('start')!.x).toBeGreaterThanOrEqual(75)
  })

  it('引用不存在节点的边与自环被忽略,不抛错', () => {
    expect(() =>
      dagreLayout(chain, [
        ...edges,
        { source: 'a', target: 'ghost' },
        { source: 'gw', target: 'gw' },
      ])
    ).not.toThrow()
  })

  it('空图返回空 Map', () => {
    expect(dagreLayout([], []).size).toBe(0)
  })
})
