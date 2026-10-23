import type { Connection } from '@xyflow/svelte'
import {
  centerToPosition,
  DEFAULT_LABEL_OFFSET,
  facingSide,
  nodeKindOf,
  positionToCenter,
} from './geometry.js'
import type {
  FlowNode,
  NodeJsonFields,
  NodeKind,
  Point,
  SkipEdge,
  SkipJsonFields,
} from './types.js'

/** 新边 id 的序号取首个空闲号:与 deserialize 的连续编号一致,且删掉中间序号的边后再连也不会撞 id。 */
export function nextSerial(
  edges: SkipEdge[],
  source: string,
  target: string
): number {
  let n = 0
  while (edges.some((e) => e.id === `${source}->${target}#${n}`)) n += 1
  return n
}

/**
 * 用户拖线成功后构造的新边。不走 Svelte Flow 自带的 addEdge:它会拒绝
 * 同一对句柄之间的第二条边,而流程里同向多边(不同条件)是合法的。
 */
export function newSkipEdge(c: Connection, edges: SkipEdge[]): SkipEdge {
  return {
    id: `${c.source}->${c.target}#${nextSerial(edges, c.source, c.target)}`,
    source: c.source,
    target: c.target,
    sourceHandle: c.sourceHandle ?? undefined,
    targetHandle: c.targetHandle ?? undefined,
    type: 'skip',
    data: {
      waypoints: null,
      labelOffset: { x: 0, y: 0 },
      json: {
        skipName: '',
        skipType: 'PASS',
        skipCondition: null,
      },
    },
  }
}

/** 节点被拖动后,与之相连的存量折线不再成立,清空让它们回到自动路由。 */
export function clearWaypointsAround(
  edges: SkipEdge[],
  movedNodeIds: Iterable<string>
): SkipEdge[] {
  const ids = new Set(movedNodeIds)
  let changed = false
  const out = edges.map((e) => {
    if (
      e.data.waypoints === null ||
      (!ids.has(e.source) && !ids.has(e.target))
    ) {
      return e
    }
    changed = true
    return { ...e, data: { ...e.data, waypoints: null } }
  })
  return changed ? out : edges
}

const KIND_TO_TYPE: Readonly<Record<NodeKind, number>> = {
  start: 0,
  between: 1,
  end: 2,
  gateway: 3,
}

const KIND_NAME: Readonly<Record<NodeKind, string>> = {
  start: '开始',
  between: '审批人',
  end: '结束',
  gateway: '互斥网关',
}

let seq = 0

/** 画布内唯一的节点编码;与 warm-flow 一样只要求定义内唯一,用时间戳 + 序号即可。 */
export function newNodeCode(): string {
  seq += 1
  return `n${Date.now().toString(36)}${seq}`
}

/**
 * 工具栏新增节点。nodeType 缺省取该种类的第一个值;三种网关由工具栏各自传 3/4/5,
 * 面板不提供改网关类型的入口(规格 §9.3,网关无节点级字段);
 * 其余 JSON 字段按后端可空字段给 null,与 LogicFlow 新建节点的产出一致。
 */
export function newFlowNode(
  kind: NodeKind,
  center: Point,
  nodeType = KIND_TO_TYPE[kind]
): FlowNode {
  const json: NodeJsonFields = {
    nodeType,
    nodeCode: newNodeCode(),
    nodeName: KIND_NAME[kind],
    permissionFlag: null,
    nodeRatio: null,
    anyNodeSkip: null,
    listenerType: null,
    listenerPath: null,
    formCustom: null,
    formPath: null,
    ext: null,
  }
  return {
    id: json.nodeCode,
    type: kind,
    position: centerToPosition(center, kind),
    data: { labelOffset: { ...DEFAULT_LABEL_OFFSET[kind] }, json },
  }
}

/** 面板改节点属性:整体替换该节点对象,Svelte Flow 才能感知(数组是 $state.raw)。 */
export function updateNodeJson(
  nodes: FlowNode[],
  id: string,
  patch: Partial<NodeJsonFields>
): FlowNode[] {
  return nodes.map((n) =>
    n.id === id
      ? { ...n, data: { ...n.data, json: { ...n.data.json, ...patch } } }
      : n
  )
}

export function updateEdgeJson(
  edges: SkipEdge[],
  id: string,
  patch: Partial<SkipJsonFields>
): SkipEdge[] {
  return edges.map((e) =>
    e.id === id
      ? { ...e, data: { ...e.data, json: { ...e.data.json, ...patch } } }
      : e
  )
}

/**
 * 改节点编码:节点 id、json.nodeCode、相连边的两端与 id、其它节点的 anyNodeSkip 引用一起改。
 * 编码在定义内必须唯一且非空,不满足时返回错误文案、不做任何改动。
 */
export function renameNodeCode(
  nodes: FlowNode[],
  edges: SkipEdge[],
  from: string,
  to: string
): { nodes: FlowNode[]; edges: SkipEdge[] } | { error: string } {
  const next = to.trim()
  if (next === '') return { error: '节点编码不能为空' }
  if (next === from) return { nodes, edges }
  if (nodes.some((n) => n.id === next))
    return { error: `节点编码 ${next} 已存在` }
  if (!nodes.some((n) => n.id === from)) return { error: `节点 ${from} 不存在` }
  const swap = (id: string) => (id === from ? next : id)
  return {
    nodes: nodes.map((n) => {
      if (n.id === from) {
        return {
          ...n,
          id: next,
          data: { ...n.data, json: { ...n.data.json, nodeCode: next } },
        }
      }
      return n.data.json.anyNodeSkip === from
        ? {
            ...n,
            data: { ...n.data, json: { ...n.data.json, anyNodeSkip: next } },
          }
        : n
    }),
    edges: edges.map((e) => {
      if (e.source !== from && e.target !== from) return e
      const source = swap(e.source)
      const target = swap(e.target)
      const serial = e.id.slice(e.id.lastIndexOf('#'))
      return { ...e, id: `${source}->${target}${serial}`, source, target }
    }),
  }
}

/**
 * 面板拖入落到一条跳转上(设计 §4.3):原边改为指向新节点,条件 / 名称 / 类型原样保留,
 * 再从新节点接一条 PASS 边到原目标。两条边都回到自动路由,标签偏移归零——
 * 偏移是相对旧路径弧长中点的,路径换掉后旧偏移没有意义。
 * 新节点必须是审批 / 网关:开始没有入边、结束没有出边,插在中间不成立。
 */
export function insertNodeOnEdge(
  nodes: FlowNode[],
  edges: SkipEdge[],
  edgeId: string,
  node: FlowNode
): { nodes: FlowNode[]; edges: SkipEdge[] } | { error: string } {
  const edge = edges.find((e) => e.id === edgeId)
  if (!edge) return { error: `跳转 ${edgeId} 不存在` }
  const kind = nodeKindOf(node.data.json.nodeType)
  if (kind === 'start' || kind === 'end')
    return { error: '开始 / 结束节点不能插在跳转中间' }
  if (nodes.some((n) => n.id === node.id))
    return { error: `节点编码 ${node.id} 已存在` }
  const src = nodes.find((n) => n.id === edge.source)
  const tgt = nodes.find((n) => n.id === edge.target)
  if (!src || !tgt) return { error: `跳转 ${edgeId} 的端点不存在` }

  const center = (n: FlowNode) =>
    positionToCenter(n.position, nodeKindOf(n.data.json.nodeType))
  const cN = center(node)
  const others = edges.filter((e) => e.id !== edgeId)
  const inbound: SkipEdge = {
    ...edge,
    id: `${edge.source}->${node.id}#${nextSerial(others, edge.source, node.id)}`,
    target: node.id,
    targetHandle: facingSide(cN, center(src)),
    data: { ...edge.data, waypoints: null, labelOffset: { x: 0, y: 0 } },
  }
  const outbound = newSkipEdge(
    {
      source: node.id,
      target: edge.target,
      sourceHandle: facingSide(cN, center(tgt)),
      targetHandle: edge.targetHandle ?? null,
    },
    [...others, inbound]
  )
  return {
    nodes: [...nodes, node],
    edges: [...edges.map((e) => (e.id === edgeId ? inbound : e)), outbound],
  }
}

/**
 * 拖边的一端落到别的节点 / 句柄上(设计 §5.2)。两端与句柄都没变返回 false,
 * xyflow 收到 false 不改数据。变了就换端点、按新的两端重编 id(空闲号在排除自己
 * 之后算,免得和自己撞号),json 原样保留,路径与标签偏移同 insertNodeOnEdge 一样归零。
 */
export function reconnectEdge(
  edges: SkipEdge[],
  oldEdge: SkipEdge,
  c: Connection
): SkipEdge | false {
  const same =
    c.source === oldEdge.source &&
    c.target === oldEdge.target &&
    (c.sourceHandle ?? null) === (oldEdge.sourceHandle ?? null) &&
    (c.targetHandle ?? null) === (oldEdge.targetHandle ?? null)
  if (same) return false
  const others = edges.filter((e) => e.id !== oldEdge.id)
  return {
    ...oldEdge,
    id: `${c.source}->${c.target}#${nextSerial(others, c.source, c.target)}`,
    source: c.source,
    target: c.target,
    sourceHandle: c.sourceHandle ?? undefined,
    targetHandle: c.targetHandle ?? undefined,
    data: { ...oldEdge.data, waypoints: null, labelOffset: { x: 0, y: 0 } },
  }
}
