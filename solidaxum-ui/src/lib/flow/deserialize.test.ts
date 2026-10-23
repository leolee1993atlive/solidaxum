import { describe, expect, it } from 'vitest'
import { defJsonToFlow } from './deserialize.js'
import { gatewayFanoutDef } from './fixtures/gateway-fanout.js'
import leave6Json from './fixtures/leave6.json'
import type { DefJson } from './types.js'

// 导入文件里 nodeType 是字符串、没有 id、网关缺 nodeName 键,与 DefJson 声明的形状不完全一致,故强转
const leave6 = leave6Json as unknown as DefJson

const code = (prefix: string) =>
  leave6.nodeList.find((n) => n.nodeCode.startsWith(prefix))!.nodeCode

describe('defJsonToFlow · leave6(真实 LogicFlow 导出)', () => {
  const g = defJsonToFlow(leave6)

  it('节点数/边数与数据一致,无告警', () => {
    expect(g.nodes).toHaveLength(14)
    expect(g.edges).toHaveLength(15)
    expect(g.warnings).toEqual([])
  })

  it('中心点换算成左上角 position;nodeType 归一为数字', () => {
    const applicant = g.nodes.find((n) => n.id === code('c25a0e86'))!
    // "400,300" between 100×80 → (350,260)
    expect(applicant.position).toEqual({ x: 350, y: 260 })
    expect(applicant.type).toBe('between')
    expect(applicant.data.json.nodeType).toBe(1)
    const gw = g.nodes.find((n) => n.id === code('48117e2c'))!
    expect(gw.position).toEqual({ x: 675, y: 275 })
    expect(gw.type).toBe('gateway')
  })

  it('句柄侧由首末点反推:网关上顶点出、上方节点左侧入', () => {
    // 700,275;700,200;810,200 → 网关 top 出,副经理(860,200) left 入
    const e = g.edges.find(
      (e) => e.source === code('48117e2c') && e.target === code('2bfa3919')
    )!
    expect(e.sourceHandle).toBe('top')
    expect(e.targetHandle).toBe('left')
    expect(e.data.waypoints).toEqual([{ x: 700, y: 200 }])
  })

  it('半径 20 时代的线头 (260,300) 归到 start 右侧', () => {
    const e = g.edges.find((e) => e.source === code('122b89a5'))!
    expect(e.sourceHandle).toBe('right')
    expect(e.targetHandle).toBe('left')
    expect(e.data.waypoints).toEqual([]) // 两点直线 = 存量折线,不是自动路由
  })

  it('边标签位换算成相对弧长中点的偏移', () => {
    const e = g.edges.find(
      (e) => e.source === code('48117e2c') && e.target === code('2bfa3919')
    )!
    // 中点 (717.5,200),标签 (700,237)
    expect(e.data.labelOffset).toEqual({ x: -17.5, y: 37 })
    expect(e.data.json.skipName).toBe('大于两天')
  })

  it('节点标签:有段的取相对中心偏移,网关无段取默认 (0,40)', () => {
    const applicant = g.nodes.find((n) => n.id === code('c25a0e86'))!
    expect(applicant.data.labelOffset).toEqual({ x: 0, y: 0 })
    const gw = g.nodes.find((n) => n.id === code('48117e2c'))!
    expect(gw.data.labelOffset).toEqual({ x: 0, y: 40 })
  })

  it('几何字段不进 data.json;其余字段原样透传', () => {
    const applicant = g.nodes.find((n) => n.id === code('c25a0e86'))!
    expect('coordinate' in applicant.data.json).toBe(false)
    expect('skipList' in applicant.data.json).toBe(false)
    expect(applicant.data.json.permissionFlag).toBe('${initiator}')
    expect(applicant.data.json.ext).toContain('ButtonPermissionEnum')
    expect(g.base.flowCode).toBe('leave6')
    expect('nodeList' in g.base).toBe(false)
  })

  it('边 id 形如 source->target#序号', () => {
    for (const e of g.edges) {
      expect(e.id).toBe(`${e.source}->${e.target}#0`)
      expect(e.type).toBe('skip')
    }
  })
})

describe('defJsonToFlow · 多线网关', () => {
  const g = defJsonToFlow(gatewayFanoutDef())
  const hubIn = g.edges.filter((e) => e.target === 'hub')
  const hubOut = g.edges.filter((e) => e.source === 'hub')

  it('hub 一身 8 入 12 出', () => {
    expect(hubIn).toHaveLength(8)
    expect(hubOut).toHaveLength(12)
  })

  it('入线分布在左/上/下三侧', () => {
    const sides = new Set(hubIn.map((e) => e.targetHandle))
    expect(sides).toEqual(new Set(['left', 'top', 'bottom']))
  })

  it('无 coordinate 的边:waypoints=null,句柄取相向侧', () => {
    const e = hubIn.find((e) => e.source === 'r4')!
    expect(e.data.waypoints).toBeNull()
    expect(e.sourceHandle).toBe('right') // r4(700,560) → hub(1000,500):横向更远
    expect(e.targetHandle).toBe('left')
  })

  it('"|NaN,NaN" 标签段按无标签位处理(偏移 0)', () => {
    const e = hubIn.find((e) => e.source === 'r3')!
    expect(e.data.waypoints).toHaveLength(2)
    expect(e.data.labelOffset).toEqual({ x: 0, y: 0 })
  })

  it('同向多边序号递增', () => {
    const def = gatewayFanoutDef()
    const hub = def.nodeList.find((n) => n.nodeCode === 'hub')!
    hub.skipList.push({ ...hub.skipList[0], skipType: 'REJECT' })
    const ids = defJsonToFlow(def)
      .edges.filter((e) => e.source === 'hub' && e.target === 'c1')
      .map((e) => e.id)
    expect(ids).toEqual(['hub->c1#0', 'hub->c1#1'])
  })
})

describe('defJsonToFlow · 缺坐标与脏数据', () => {
  const stub = () => {
    const calls: number[] = []
    const layout = (nodes: { id: string }[]) => {
      calls.push(nodes.length)
      return new Map(nodes.map((n, i) => [n.id, { x: 100 + i * 200, y: 100 }]))
    }
    return { layout, calls }
  }

  it('全部节点无坐标 → 调兜底布局,边全部自动路由', () => {
    const def = gatewayFanoutDef()
    for (const n of def.nodeList) n.coordinate = null
    const { layout, calls } = stub()
    const g = defJsonToFlow(def, { layout })
    expect(calls).toEqual([def.nodeList.length])
    expect(g.nodes[0].position).toEqual({ x: 100 - 25, y: 100 - 25 }) // start 50×50
    expect(g.edges.every((e) => e.data.waypoints === null)).toBe(true)
  })

  it('只有个别节点无坐标 → 不调布局,排到右侧一列;相连边改自动路由', () => {
    const def = gatewayFanoutDef()
    const c12 = def.nodeList.find((n) => n.nodeCode === 'c12')!
    c12.coordinate = null
    const { layout, calls } = stub()
    const g = defJsonToFlow(def, { layout })
    expect(calls).toEqual([])
    const placed = g.nodes.find((n) => n.id === 'c12')!
    // 最右中心 1680(end) + 150 = 1830 → between 左上角 x = 1780
    expect(placed.position.x).toBe(1830 - 50)
    const inEdge = g.edges.find((e) => e.target === 'c12')!
    expect(inEdge.data.waypoints).toBeNull()
  })

  it('指向不存在节点的跳转被丢弃并告警', () => {
    const def = gatewayFanoutDef()
    def.nodeList[0].skipList.push({
      nowNodeCode: 'start',
      nextNodeCode: 'ghost',
      skipName: null,
      skipType: 'PASS',
      skipCondition: null,
      coordinate: null,
    })
    const g = defJsonToFlow(def)
    expect(g.edges.some((e) => e.target === 'ghost')).toBe(false)
    expect(g.warnings).toHaveLength(1)
    expect(g.warnings[0]).toContain('ghost')
  })

  it('空定义(query-def 空模板)→ 空图', () => {
    const g = defJsonToFlow({ ...gatewayFanoutDef(), nodeList: [] })
    expect(g.nodes).toEqual([])
    expect(g.edges).toEqual([])
  })
})
