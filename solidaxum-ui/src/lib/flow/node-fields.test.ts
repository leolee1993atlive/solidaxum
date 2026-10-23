import { describe, expect, it } from 'vitest'
import { defJsonToFlow } from './deserialize.js'
import leave6Json from './fixtures/leave6.json'
import {
  formatListeners,
  joinPermissionFlag,
  parseListeners,
  previousNodes,
  splitPermissionFlag,
} from './node-fields.js'
import type { DefJson } from './types.js'

describe('监听器行编解码(类型逗号分隔、路径 @@ 分隔,按下标配对)', () => {
  it('parse:两边长度不齐时补空串', () => {
    expect(parseListeners('start,finish', 'a.B@@c.D')).toEqual([
      { listenerType: 'start', listenerPath: 'a.B' },
      { listenerType: 'finish', listenerPath: 'c.D' },
    ])
    expect(parseListeners('start', null)).toEqual([
      { listenerType: 'start', listenerPath: '' },
    ])
    expect(parseListeners(null, null)).toEqual([])
  })

  it('format:全空行丢弃,没有行时两个字段都是 null', () => {
    expect(
      formatListeners([
        { listenerType: 'start', listenerPath: ' a.B ' },
        { listenerType: '', listenerPath: '' },
        { listenerType: 'finish', listenerPath: 'c.D' },
      ])
    ).toEqual({ listenerType: 'start,finish', listenerPath: 'a.B@@c.D' })
    expect(formatListeners([])).toEqual({
      listenerType: null,
      listenerPath: null,
    })
    expect(formatListeners([{ listenerType: '', listenerPath: '' }])).toEqual({
      listenerType: null,
      listenerPath: null,
    })
  })

  it('半填的行保留(校验层会提醒),不会被静默丢掉', () => {
    expect(
      formatListeners([{ listenerType: 'start', listenerPath: '' }])
    ).toEqual({
      listenerType: 'start',
      listenerPath: '',
    })
  })
})

describe('permissionFlag @@ 拼串', () => {
  it('split 去空段;join 去空白后拼,空列表 → null', () => {
    expect(splitPermissionFlag('user:1@@role:2@@')).toEqual([
      'user:1',
      'role:2',
    ])
    expect(splitPermissionFlag(null)).toEqual([])
    expect(joinPermissionFlag([' user:1 ', '', 'role:2'])).toBe(
      'user:1@@role:2'
    )
    expect(joinPermissionFlag([])).toBeNull()
  })

  it('表达式项(如 ${initiator})原样保留', () => {
    expect(splitPermissionFlag('${initiator}')).toEqual(['${initiator}'])
  })
})

describe('previousNodes(驳回到指定节点的候选)', () => {
  const g = defJsonToFlow(leave6Json as unknown as DefJson)
  const byName = (name: string) =>
    g.nodes.find((n) => n.data.json.nodeName === name)!

  it('沿 PASS 边回溯祖先,去掉开始与网关,近的在前', () => {
    const prev = previousNodes(g.nodes, g.edges, byName('经理会签').id)
    expect(prev.map((n) => n.data.json.nodeName)).toEqual([
      '副经理',
      '副组长',
      '申请人',
      '组长',
    ])
  })

  it('第一个审批节点没有可驳回的前置节点', () => {
    expect(previousNodes(g.nodes, g.edges, byName('申请人').id)).toEqual([])
  })

  it('REJECT 边不参与回溯;环不会死循环', () => {
    const edges = [
      ...g.edges,
      {
        ...g.edges[0],
        id: 'loop',
        source: byName('经理会签').id,
        target: byName('申请人').id,
        data: {
          ...g.edges[0].data,
          json: { ...g.edges[0].data.json, skipType: 'REJECT' as const },
        },
      },
      {
        ...g.edges[0],
        id: 'cycle',
        source: byName('经理会签').id,
        target: byName('副组长').id,
      },
    ]
    const prev = previousNodes(g.nodes, edges, byName('经理会签').id)
    expect(new Set(prev.map((n) => n.id)).size).toBe(prev.length)
    expect(prev.map((n) => n.data.json.nodeName)).toContain('副组长')
    // 经 PASS 环能回到自身的节点,也不该成为自己的驳回候选
    expect(prev.some((n) => n.id === byName('经理会签').id)).toBe(false)
  })
})
