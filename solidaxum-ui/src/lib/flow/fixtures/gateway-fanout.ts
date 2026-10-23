import type { DefJson, NodeJson, SkipJson } from '../types.js'

/**
 * 手工构造的多线网关流程(不良事件上报的形状):
 *   start → 角色分流网关 → 8 个角色上报节点 → 事件分类网关 → 12 个分类处理节点 → end
 * 分类网关一身挂 20 条线(8 入 12 出),入/出线分布在四侧;
 * 边混合:存量折线 / 两点直线 / 无 coordinate / 有标签位 / "|NaN,NaN" 标签段。
 * 另有一个 x=0 且无标签段的节点,对应老设计器"坐标含 0 丢标签段"的怪癖。
 */
export function gatewayFanoutDef(): DefJson {
  const node = (
    nodeType: number,
    nodeCode: string,
    nodeName: string | null,
    coordinate: string | null,
    skipList: SkipJson[] = []
  ): NodeJson => ({
    nodeType,
    nodeCode,
    nodeName,
    permissionFlag: nodeType === 1 ? 'role:1' : null,
    nodeRatio: null,
    coordinate,
    anyNodeSkip: null,
    listenerType: null,
    listenerPath: null,
    formCustom: null,
    formPath: null,
    ext: null,
    skipList,
  })
  const skip = (
    now: string,
    next: string,
    coordinate: string | null,
    extra: Partial<SkipJson> = {}
  ): SkipJson => ({
    nowNodeCode: now,
    nextNodeCode: next,
    skipName: null,
    skipType: 'PASS',
    skipCondition: null,
    coordinate,
    ...extra,
  })

  // 角色节点:分类网关 hub 在 (1000,500);8 个角色围在左侧/上方/下方
  const hub = { x: 1000, y: 500 }
  const roles = [
    { code: 'r1', name: '医生', c: { x: 700, y: 200 } },
    { code: 'r2', name: '护士', c: { x: 700, y: 320 } },
    { code: 'r3', name: '药师', c: { x: 700, y: 440 } },
    { code: 'r4', name: '技师', c: { x: 700, y: 560 } },
    { code: 'r5', name: '行政', c: { x: 700, y: 680 } },
    { code: 'r6', name: '后勤', c: { x: 700, y: 800 } },
    { code: 'r7', name: '患者', c: { x: 1000, y: 200 } }, // 从上方进
    { code: 'r8', name: '家属', c: { x: 1000, y: 800 } }, // 从下方进
  ]
  // 分类节点:12 个,分布在右侧一列与右上/右下
  const cats = Array.from({ length: 12 }, (_, i) => ({
    code: `c${i + 1}`,
    name: `分类${i + 1}`,
    c: { x: 1400, y: 80 + i * 90 },
  }))

  const roleSkips = (r: (typeof roles)[number], i: number): SkipJson => {
    // 左侧 6 个走折线进 hub 左顶点;上/下两个直线进 hub 上/下顶点
    if (r.c.x < hub.x) {
      const start = { x: r.c.x + 50, y: r.c.y }
      const coord = `${start.x},${start.y};${880},${start.y};${880},${hub.y};${hub.x - 25},${hub.y}`
      // 第 2 条带显式标签位,第 3 条带 NaN 标签段,第 4 条不存 coordinate
      if (i === 1) return skip(r.code, 'hub', `${coord}|880,${start.y - 12}`)
      if (i === 2) return skip(r.code, 'hub', `${coord}|NaN,NaN`)
      if (i === 3) return skip(r.code, 'hub', null)
      return skip(r.code, 'hub', coord)
    }
    return r.c.y < hub.y
      ? skip(r.code, 'hub', `${r.c.x},${r.c.y + 40};${hub.x},${hub.y - 25}`)
      : skip(r.code, 'hub', `${r.c.x},${r.c.y - 40};${hub.x},${hub.y + 25}`)
  }

  const hubSkips: SkipJson[] = cats.map((cat, i) => {
    const end = { x: cat.c.x - 50, y: cat.c.y }
    const coord = `${hub.x + 25},${hub.y};1200,${hub.y};1200,${end.y};${end.x},${end.y}`
    const name = `分类${i + 1}`
    const cond = `serial@@category|${i + 1}`
    // 偶数条带标签位,第 5 条不存 coordinate(自动路由),其余存折线
    if (i === 4)
      return skip('hub', cat.code, null, {
        skipName: name,
        skipCondition: cond,
      })
    if (i % 2 === 0) {
      return skip('hub', cat.code, `${coord}|1200,${end.y - 10}`, {
        skipName: name,
        skipCondition: cond,
      })
    }
    return skip('hub', cat.code, coord, { skipName: name, skipCondition: cond })
  })

  const nodeList: NodeJson[] = [
    // start 在 x=0:老设计器会因坐标含 0 丢掉标签段
    node(0, 'start', '开始', '0,500', [
      skip('start', 'split', '25,500;175,500'),
    ]),
    // split 用包容网关(5):三种网关画布行为相同,顺带补齐 6 种 nodeType 的 fixture 覆盖
    node(5, 'split', '角色分流', '200,500|200,540', [
      ...roles.map((r) =>
        skip('split', r.code, null, {
          skipName: r.name,
          skipCondition: `serial@@role|${r.code}`,
        })
      ),
    ]),
    ...roles.map((r, i) =>
      node(1, r.code, r.name, `${r.c.x},${r.c.y}|${r.c.x},${r.c.y}`, [
        roleSkips(r, i),
      ])
    ),
    node(3, 'hub', '事件分类', `${hub.x},${hub.y}`, hubSkips),
    ...cats.map((cat) =>
      node(
        1,
        cat.code,
        cat.name,
        `${cat.c.x},${cat.c.y}|${cat.c.x},${cat.c.y}`,
        [
          skip(
            cat.code,
            'end',
            `${cat.c.x + 50},${cat.c.y};1600,${cat.c.y};1600,575;1655,575`
          ),
        ]
      )
    ),
    node(2, 'end', '结束', '1680,575|1680,575'),
  ]

  return {
    id: '1761300000000000099',
    flowCode: 'adverse_event',
    flowName: '不良事件上报',
    modelValue: 'CLASSICS',
    category: null,
    version: '1',
    isPublish: 0,
    formCustom: 'N',
    formPath: null,
    listenerType: null,
    listenerPath: null,
    ext: null,
    nodeList,
  }
}
