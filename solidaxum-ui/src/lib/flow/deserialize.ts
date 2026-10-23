import { parseEdgeCoord, parseNodeCoord } from './coordinate.js'
import {
  centerToPosition,
  DEFAULT_LABEL_OFFSET,
  facingSide,
  nearestSide,
  nodeKindOf,
} from './geometry.js'
import { dagreLayout, type LayoutFn } from './layout.js'
import { midpoint } from './route.js'
import type {
  DefJson,
  DefJsonBase,
  FlowNode,
  NodeJson,
  NodeKind,
  Point,
  SkipEdge,
  SkipJson,
} from './types.js'

export interface FlowGraph {
  nodes: FlowNode[]
  edges: SkipEdge[]
  base: DefJsonBase
  /** 被丢弃的脏数据说明(如指向不存在节点的跳转),供界面提示;正常数据为空。 */
  warnings: string[]
}

export interface DeserializeOptions {
  /** 全部节点都无坐标时的兜底布局;测试可注入桩。 */
  layout?: LayoutFn
}

interface PlacedNode {
  json: NodeJson
  kind: NodeKind
  center: Point
  /** 中心是否来自数据(而非兜底布局);影响相连边是否信任存量折线 */
  stored: boolean
  text?: Point
}

/** 缺坐标的少数节点排到已有布局右侧的一列,不打乱用户摆好的其余节点。 */
function placeStragglers(placed: PlacedNode[], stragglers: PlacedNode[]) {
  const maxX = Math.max(...placed.map((n) => n.center.x))
  const minY = Math.min(...placed.map((n) => n.center.y))
  stragglers.forEach((n, i) => {
    n.center = { x: maxX + 150, y: minY + i * 120 }
  })
}

function placeNodes(def: DefJson, layoutFn: LayoutFn): PlacedNode[] {
  const nodes: PlacedNode[] = def.nodeList.map((json) => {
    const kind = nodeKindOf(Number(json.nodeType))
    const coord = parseNodeCoord(json.coordinate)
    return coord
      ? { json, kind, center: coord.center, stored: true, text: coord.text }
      : { json, kind, center: { x: 0, y: 0 }, stored: false }
  })
  const missing = nodes.filter((n) => !n.stored)
  if (missing.length === 0) return nodes
  if (missing.length === nodes.length) {
    const centers = layoutFn(
      nodes.map((n) => ({ id: n.json.nodeCode, kind: n.kind })),
      def.nodeList.flatMap((n) =>
        n.skipList.map((s) => ({
          source: s.nowNodeCode,
          target: s.nextNodeCode,
        }))
      )
    )
    for (const n of nodes) {
      n.center = centers.get(n.json.nodeCode) ?? n.center
    }
  } else {
    placeStragglers(
      nodes.filter((n) => n.stored),
      missing
    )
  }
  return nodes
}

function toFlowNode(n: PlacedNode): FlowNode {
  const { coordinate: _coordinate, skipList: _skipList, ...rest } = n.json
  const labelOffset = n.text
    ? { x: n.text.x - n.center.x, y: n.text.y - n.center.y }
    : { ...DEFAULT_LABEL_OFFSET[n.kind] }
  return {
    id: n.json.nodeCode,
    type: n.kind,
    position: centerToPosition(n.center, n.kind),
    data: {
      labelOffset,
      // 导入文件里 nodeType 可能是字符串("1"),画布内统一成数字
      json: { ...rest, nodeType: Number(rest.nodeType) },
    },
  }
}

function toSkipEdge(
  skip: SkipJson,
  src: PlacedNode,
  tgt: PlacedNode,
  serial: number
): SkipEdge {
  const {
    coordinate: _coordinate,
    nowNodeCode: _now,
    nextNodeCode: _next,
    ...rest
  } = skip
  // 任一端节点是兜底摆放的,存量折线就没有意义,一律改走自动路由
  const coord =
    src.stored && tgt.stored ? parseEdgeCoord(skip.coordinate) : null
  let sourceHandle: string
  let targetHandle: string
  let waypoints: Point[] | null
  let labelOffset: Point
  if (coord) {
    const pts = coord.points
    sourceHandle = nearestSide(src.center, src.kind, pts[0])
    targetHandle = nearestSide(tgt.center, tgt.kind, pts[pts.length - 1])
    waypoints = pts.slice(1, -1)
    const mid = midpoint(pts)
    labelOffset = coord.text
      ? { x: coord.text.x - mid.x, y: coord.text.y - mid.y }
      : { x: 0, y: 0 }
  } else {
    sourceHandle = facingSide(src.center, tgt.center)
    targetHandle = facingSide(tgt.center, src.center)
    waypoints = null
    labelOffset = { x: 0, y: 0 }
  }
  return {
    id: `${skip.nowNodeCode}->${skip.nextNodeCode}#${serial}`,
    source: skip.nowNodeCode,
    target: skip.nextNodeCode,
    sourceHandle,
    targetHandle,
    type: 'skip',
    data: { waypoints, labelOffset, json: rest },
  }
}

/**
 * DefJson → 画布图。几何(中心、句柄侧、折线、标签位)全部在这里换算,
 * 其余字段原样进 data.json / base,保存时零字段丢失。
 */
export function defJsonToFlow(
  def: DefJson,
  opts: DeserializeOptions = {}
): FlowGraph {
  const { nodeList: _nodeList, ...base } = def
  const placed = placeNodes(def, opts.layout ?? dagreLayout)
  const byCode = new Map(placed.map((n) => [n.json.nodeCode, n]))
  const warnings: string[] = []
  const edges: SkipEdge[] = []
  const serials = new Map<string, number>()
  for (const holder of placed) {
    for (const skip of holder.json.skipList) {
      // 两端以跳转自己的 nowNodeCode/nextNodeCode 为准,与 LogicFlow 的读法一致
      const src = byCode.get(skip.nowNodeCode)
      const tgt = byCode.get(skip.nextNodeCode)
      if (!src || !tgt) {
        warnings.push(
          `跳转 ${skip.nowNodeCode} → ${skip.nextNodeCode} 指向不存在的节点,已忽略`
        )
        continue
      }
      const key = `${skip.nowNodeCode}->${skip.nextNodeCode}`
      const serial = serials.get(key) ?? 0
      serials.set(key, serial + 1)
      edges.push(toSkipEdge(skip, src, tgt, serial))
    }
  }
  return { nodes: placed.map(toFlowNode), edges, base, warnings }
}
