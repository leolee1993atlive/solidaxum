import { serializeEdgeCoord, serializeNodeCoord } from './coordinate.js'
import {
  anchorPoint,
  facingSide,
  nodeKindOf,
  positionToCenter,
} from './geometry.js'
import { midpoint, orthogonalRoute } from './route.js'
import type {
  DefJson,
  DefJsonBase,
  FlowNode,
  NodeJson,
  NodeKind,
  Point,
  Side,
  SkipEdge,
  SkipEdgeData,
  SkipJson,
} from './types.js'

export interface FlowGraphInput {
  nodes: FlowNode[]
  edges: SkipEdge[]
  base: DefJsonBase
}

interface NodeGeom {
  kind: NodeKind
  center: Point
}

const SIDE_SET: ReadonlySet<string> = new Set([
  'top',
  'right',
  'bottom',
  'left',
])

function asSide(handle: string | null | undefined, fallback: Side): Side {
  return handle && SIDE_SET.has(handle) ? (handle as Side) : fallback
}

function edgePoints(
  edge: SkipEdge,
  data: SkipEdgeData,
  src: NodeGeom,
  tgt: NodeGeom
): { points: Point[]; text: Point } {
  const srcSide = asSide(edge.sourceHandle, facingSide(src.center, tgt.center))
  const tgtSide = asSide(edge.targetHandle, facingSide(tgt.center, src.center))
  const p0 = anchorPoint(src.center, src.kind, srcSide)
  const pn = anchorPoint(tgt.center, tgt.kind, tgtSide)
  const points =
    data.waypoints === null
      ? orthogonalRoute(p0, srcSide, pn, tgtSide)
      : [p0, ...data.waypoints, pn]
  const mid = midpoint(points)
  const off = data.labelOffset
  return { points, text: { x: mid.x + off.x, y: mid.y + off.y } }
}

/**
 * 画布图 → DefJson。输入必须是 useSvelteFlow().getNodes()/getEdges() 的实时数据
 * (外部 $state 里没有拖拽后的坐标)。modelValue 固定 CLASSICS。
 */
export function flowToDefJson(graph: FlowGraphInput): DefJson {
  const geom = new Map<string, NodeGeom>()
  const nodeJson = new Map<string, NodeJson>()
  const nodeList: NodeJson[] = []
  for (const node of graph.nodes) {
    const kind = nodeKindOf(Number(node.data.json.nodeType))
    const center = positionToCenter(node.position, kind)
    const { labelOffset } = node.data
    const text = { x: center.x + labelOffset.x, y: center.y + labelOffset.y }
    const json: NodeJson = {
      ...node.data.json,
      nodeType: Number(node.data.json.nodeType),
      coordinate: serializeNodeCoord(center, text),
      skipList: [],
    }
    geom.set(node.id, { kind, center })
    nodeJson.set(node.id, json)
    nodeList.push(json)
  }
  for (const edge of graph.edges) {
    const src = geom.get(edge.source)
    const tgt = geom.get(edge.target)
    if (!src || !tgt) {
      throw new Error(`跳转 ${edge.source} → ${edge.target} 的端点不在画布上`)
    }
    const { points, text } = edgePoints(edge, edge.data, src, tgt)
    const skip: SkipJson = {
      nowNodeCode: edge.source,
      nextNodeCode: edge.target,
      ...edge.data.json,
      coordinate: serializeEdgeCoord(points, text),
    }
    nodeJson.get(edge.source)!.skipList.push(skip)
  }
  return { ...graph.base, modelValue: 'CLASSICS', nodeList }
}
