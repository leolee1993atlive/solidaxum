import { newNodeCode, nextSerial } from './edit.js'
import { nodeKindOf } from './geometry.js'
import { cloneGraph, type Graph } from './snapshot.js'
import type { FlowNode, Point, SkipEdge } from './types.js'

/** 会话内存剪贴板的内容:一组节点与它们之间的边(位置原样)。 */
export type Clipboard = Graph

/**
 * 复制当前选集(设计 §6.3):开始 / 结束不进剪贴板(一个流程只有一个),
 * 边只带两端都在选集里的。选集为空返回 null。
 */
export function copySelection(
  nodes: FlowNode[],
  edges: SkipEdge[]
): Clipboard | null {
  const picked = nodes.filter((n) => {
    const kind = nodeKindOf(n.data.json.nodeType)
    return n.selected && kind !== 'start' && kind !== 'end'
  })
  if (picked.length === 0) return null
  const ids = new Set(picked.map((n) => n.id))
  return cloneGraph({
    nodes: picked,
    edges: edges.filter((e) => ids.has(e.source) && ids.has(e.target)),
  })
}

const snap10 = (v: number) => Math.round(v / 10) * 10

/**
 * 粘贴:整组外接框左上角对齐到 at(吸附 10px 网格,组内相对位置不变),每个节点
 * 分配新编码、名称不改(编码已保证唯一,用户本来就要改名);边按新旧 id 映射并回到
 * 自动路由;anyNodeSkip 指向组内节点的一起映射,指向组外的原样保留(仍是有效引用)。
 * 返回的数组里只有粘贴出来的元素是选中态。
 */
export function pasteClipboard(
  clip: Clipboard,
  nodes: FlowNode[],
  edges: SkipEdge[],
  at: Point
): { nodes: FlowNode[]; edges: SkipEdge[]; pastedIds: string[] } {
  const dx = snap10(at.x) - Math.min(...clip.nodes.map((n) => n.position.x))
  const dy = snap10(at.y) - Math.min(...clip.nodes.map((n) => n.position.y))
  const idMap = new Map<string, string>()
  for (const n of clip.nodes) idMap.set(n.id, newNodeCode())

  const pasted = clip.nodes.map((n) => {
    const id = idMap.get(n.id)!
    const any = n.data.json.anyNodeSkip
    return {
      ...n,
      id,
      selected: true,
      position: { x: n.position.x + dx, y: n.position.y + dy },
      data: {
        ...n.data,
        json: {
          ...n.data.json,
          nodeCode: id,
          anyNodeSkip: any && idMap.has(any) ? idMap.get(any)! : any,
        },
      },
    }
  })

  const outEdges = edges.map((e) =>
    e.selected ? { ...e, selected: false } : e
  )
  for (const e of clip.edges) {
    const source = idMap.get(e.source)!
    const target = idMap.get(e.target)!
    outEdges.push({
      ...e,
      id: `${source}->${target}#${nextSerial(outEdges, source, target)}`,
      source,
      target,
      selected: true,
      data: { ...e.data, waypoints: null },
    })
  }
  return {
    nodes: [
      ...nodes.map((n) => (n.selected ? { ...n, selected: false } : n)),
      ...pasted,
    ],
    edges: outEdges,
    pastedIds: pasted.map((n) => n.id),
  }
}
