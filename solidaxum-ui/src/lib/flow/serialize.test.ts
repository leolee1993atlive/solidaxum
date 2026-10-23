import { describe, expect, it } from 'vitest'
import { parseEdgeCoord, parseNodeCoord } from './coordinate.js'
import { defJsonToFlow } from './deserialize.js'
import { gatewayFanoutDef } from './fixtures/gateway-fanout.js'
import leave2Json from './fixtures/leave2.json'
import leave6Json from './fixtures/leave6.json'
import {
  anchorPoint,
  DEFAULT_LABEL_OFFSET,
  facingSide,
  nearestSide,
  nodeKindOf,
} from './geometry.js'
import { midpoint, orthogonalRoute } from './route.js'
import { flowToDefJson } from './serialize.js'
import type { DefJson, NodeJson, Point } from './types.js'

const leave2 = leave2Json as unknown as DefJson
const leave6 = leave6Json as unknown as DefJson

const add = (a: Point, b: Point): Point => ({ x: a.x + b.x, y: a.y + b.y })
const sub = (a: Point, b: Point): Point => ({ x: a.x - b.x, y: a.y - b.y })

/**
 * 把 DefJson 归一成"解析后的几何 + 其余字段",用于 round-trip 比较。
 * 规格 §8 允许的差异在这里被吸收:
 *  - 首/末点吸附到当前尺寸的最近锚点(半径 20→25 的旧数据)
 *  - 缺省标签位按默认规则补齐;有标签位的边随路径中点平移
 *  - 无 coordinate 的边以 orthogonalRoute 的输出为准
 *  - nodeType 归一为数字;modelValue 不参与比较
 */
function geometryView(def: DefJson) {
  const byCode = new Map(def.nodeList.map((n) => [n.nodeCode, n]))
  const geomOf = (n: NodeJson) => {
    const kind = nodeKindOf(Number(n.nodeType))
    return { kind, center: parseNodeCoord(n.coordinate)!.center }
  }
  const { nodeList, modelValue: _mv, ...base } = def
  return {
    base,
    nodes: nodeList.map((n) => {
      const { coordinate, skipList, ...rest } = n
      const { kind, center } = geomOf(n)
      const text =
        parseNodeCoord(coordinate)?.text ??
        add(center, DEFAULT_LABEL_OFFSET[kind])
      return {
        ...rest,
        nodeType: Number(n.nodeType),
        center,
        text,
        skips: skipList.map((s) => {
          const { coordinate: sc, ...srest } = s
          const tgt = geomOf(byCode.get(s.nextNodeCode)!)
          const ec = parseEdgeCoord(sc)
          let points: Point[]
          let stext: Point
          if (ec) {
            const first = ec.points[0]
            const last = ec.points[ec.points.length - 1]
            points = [
              anchorPoint(center, kind, nearestSide(center, kind, first)),
              ...ec.points.slice(1, -1),
              anchorPoint(
                tgt.center,
                tgt.kind,
                nearestSide(tgt.center, tgt.kind, last)
              ),
            ]
            const shift = sub(midpoint(points), midpoint(ec.points))
            stext = ec.text ? add(ec.text, shift) : midpoint(points)
          } else {
            const ss = facingSide(center, tgt.center)
            const ts = facingSide(tgt.center, center)
            points = orthogonalRoute(
              anchorPoint(center, kind, ss),
              ss,
              anchorPoint(tgt.center, tgt.kind, ts),
              ts
            )
            stext = midpoint(points)
          }
          return { ...srest, points, text: stext }
        }),
      }
    }),
  }
}

/** 序列化后的标签位是取整过的,比较前把期望值也取整。 */
function roundTexts<
  T extends { nodes: { text: Point; skips: { text: Point }[] }[] },
>(v: T): T {
  const r = (p: Point) => ({ x: Math.round(p.x), y: Math.round(p.y) })
  for (const n of v.nodes) {
    n.text = r(n.text)
    for (const s of n.skips) s.text = r(s.text)
  }
  return v
}

describe.each([
  ['leave2(真实 CLASSICS,含互斥网关与边标签)', () => leave2],
  ['leave6(真实 CLASSICS,14 节点含并行/互斥网关)', () => leave6],
  [
    'leave6 标为 mimic(C-1:加载按坐标渲染,保存改写 CLASSICS)',
    () => ({ ...leave6, modelValue: 'mimic' }),
  ],
  ['多线网关(8 入 12 出,混合存量/自动/NaN 标签)', gatewayFanoutDef],
])('round-trip · %s', (_name, make) => {
  const fixture = make()
  const once = flowToDefJson(defJsonToFlow(fixture))

  it('解析后几何与非几何字段逐一相等', () => {
    expect(roundTexts(geometryView(once))).toEqual(
      roundTexts(geometryView(fixture))
    )
  })

  it('modelValue 固定写 CLASSICS;nodeType 写数字', () => {
    expect(once.modelValue).toBe('CLASSICS')
    for (const n of once.nodeList) expect(typeof n.nodeType).toBe('number')
  })

  it('coordinate 总是两段式且无 NaN', () => {
    for (const n of once.nodeList) {
      expect(n.coordinate).toMatch(/^-?\d+,-?\d+\|-?\d+,-?\d+$/)
      for (const s of n.skipList) {
        expect(s.coordinate).toMatch(
          /^(-?\d+,-?\d+;)+-?\d+,-?\d+\|-?\d+,-?\d+$/
        )
      }
    }
  })

  it('第二次 round-trip 与第一次逐字节相同(输出已是规范形)', () => {
    const twice = flowToDefJson(defJsonToFlow(once))
    expect(twice).toEqual(once)
  })
})

describe('flowToDefJson · 保存细节', () => {
  it('边按 source 归组回源节点的 skipList,顺序与 edges 一致', () => {
    const g = defJsonToFlow(gatewayFanoutDef())
    const out = flowToDefJson(g)
    const hub = out.nodeList.find((n) => n.nodeCode === 'hub')!
    expect(hub.skipList.map((s) => s.nextNodeCode)).toEqual(
      g.edges.filter((e) => e.source === 'hub').map((e) => e.target)
    )
    expect(hub.skipList.every((s) => s.nowNodeCode === 'hub')).toBe(true)
  })

  it('拖动节点后:中心按实时 position 反算,线头跟随句柄侧', () => {
    const g = defJsonToFlow(leave6)
    const applicant = g.nodes.find((n) => n.data.json.nodeName === '申请人')!
    applicant.position = {
      x: applicant.position.x + 10,
      y: applicant.position.y - 30,
    }
    const out = flowToDefJson(g)
    const n = out.nodeList.find((n) => n.nodeName === '申请人')!
    expect(parseNodeCoord(n.coordinate)!.center).toEqual({ x: 410, y: 270 })
    // 申请人 → 副组长 的首点 = 新中心的右侧锚点
    expect(parseEdgeCoord(n.skipList[0].coordinate)!.points[0]).toEqual({
      x: 460,
      y: 270,
    })
  })

  it('waypoints=null 的边写出自动路由的全部角点', () => {
    const g = defJsonToFlow(gatewayFanoutDef())
    const e = g.edges.find((e) => e.source === 'r4' && e.target === 'hub')!
    expect(e.data.waypoints).toBeNull()
    const out = flowToDefJson(g)
    const r4 = out.nodeList.find((n) => n.nodeCode === 'r4')!
    const pts = parseEdgeCoord(r4.skipList[0].coordinate)!.points
    // r4 右锚 (750,560) → hub 左顶点 (975,500):右-上-右,中线 x = (770+955)/2
    expect(pts).toEqual([
      { x: 750, y: 560 },
      { x: 863, y: 560 },
      { x: 863, y: 500 },
      { x: 975, y: 500 },
    ])
  })

  it('句柄 id 不合法或缺失时回落到相向侧', () => {
    const g = defJsonToFlow(leave6)
    const e = g.edges[0]
    e.sourceHandle = 'nope'
    e.targetHandle = null
    expect(() => flowToDefJson(g)).not.toThrow()
  })

  it('边端点不在画布上 → 抛错(而不是静默丢边)', () => {
    const g = defJsonToFlow(leave6)
    g.edges[0].target = 'ghost'
    expect(() => flowToDefJson(g)).toThrow(/ghost/)
  })
})
