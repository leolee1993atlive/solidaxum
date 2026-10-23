import { Graph, layout } from '@dagrejs/dagre'
import { NODE_SIZE } from './geometry.js'
import type { NodeKind, Point } from './types.js'

export interface LayoutNode {
  id: string
  kind: NodeKind
}

export interface LayoutEdge {
  source: string
  target: string
}

/** 给一组无坐标节点算出中心点。只在数据缺坐标时作首次落点,之后由用户拖拽维护。 */
export type LayoutFn = (
  nodes: LayoutNode[],
  edges: LayoutEdge[]
) => Map<string, Point>

type Box = { width: number; height: number; x?: number; y?: number }

/**
 * dagre 分层布局,从左到右(与 LogicFlow 经典模板及历史数据的走向一致)。
 * 引用了不存在节点的边直接忽略,不让脏数据把整个布局搞挂。
 */
export const dagreLayout: LayoutFn = (nodes, edges) => {
  const g = new Graph<Record<string, unknown>, Box, Record<string, never>>()
  g.setGraph({
    rankdir: 'LR',
    nodesep: 60,
    ranksep: 80,
    marginx: 40,
    marginy: 40,
  })
  g.setDefaultEdgeLabel(() => ({}))
  const ids = new Set(nodes.map((n) => n.id))
  for (const n of nodes) {
    const { w, h } = NODE_SIZE[n.kind]
    g.setNode(n.id, { width: w, height: h })
  }
  for (const e of edges) {
    if (ids.has(e.source) && ids.has(e.target) && e.source !== e.target) {
      g.setEdge(e.source, e.target)
    }
  }
  layout(g)
  const centers = new Map<string, Point>()
  for (const n of nodes) {
    const box = g.node(n.id)
    centers.set(n.id, { x: Math.round(box.x ?? 0), y: Math.round(box.y ?? 0) })
  }
  return centers
}
