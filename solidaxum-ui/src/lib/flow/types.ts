import type { Edge, Node } from '@xyflow/svelte'

/**
 * 后端 warm-flow 定义 JSON(solidaxum/crates/flow/src/model/def_json.rs,
 * camelCase)。字段名与可空性逐一对齐,不得增删——这是与后端唯一的数据契约。
 * 雪花 id 由响应层字符串化、请求层还原,前端一律按 string 处理。
 * 带 ? 的键:query-def 空模板会整个缺省(warm_flow_ui_handler.rs:77-89),
 * 反序列化时按 null 归一。
 */
export interface DefJson {
  id?: string | null
  flowCode: string
  flowName: string
  /** 保存时固定写 CLASSICS;读到 mimic 也按经典模式渲染 */
  modelValue: string
  category?: string | null
  version: string
  isPublish: number | null
  formCustom: string | null
  formPath?: string | null
  listenerType?: string | null
  listenerPath?: string | null
  ext?: string | null
  createBy?: string | null
  updateBy?: string | null
  nodeList: NodeJson[]
  /** 仅 query-flow-chart 带:[done, todo, notDone] 三个 "r,g,b" 串 */
  chartStatusColor?: string[] | null
  topText?: string | null
  topTextShow?: boolean
}

export interface NodeJson {
  /** 0 start / 1 between / 2 end / 3 serial / 4 parallel / 5 inclusive */
  nodeType: number
  /** 唯一,作为画布 node id */
  nodeCode: string
  nodeName: string | null
  version?: string | null
  /** 办理人 storageId 列表,"@@" 分隔 */
  permissionFlag: string | null
  nodeRatio: string | null
  /** "cx,cy" 或 "cx,cy|tx,ty",中心点坐标 + 可选标签绝对位 */
  coordinate: string | null
  anyNodeSkip: string | null
  listenerType: string | null
  listenerPath: string | null
  formCustom: string | null
  formPath: string | null
  /** "[{code,value}]" JSON 串 */
  ext: string | null
  createBy?: string | null
  updateBy?: string | null
  /** 仅 query-flow-chart 带:0 未到 / 1 进行中 / 2 已完成 */
  status?: number | null
  /** 从本节点出发的全部跳转 */
  skipList: SkipJson[]
}

export type SkipType = 'PASS' | 'REJECT' | 'NONE'

export interface SkipJson {
  nowNodeCode: string
  nextNodeCode: string
  skipName: string | null
  skipType: SkipType | null
  skipCondition: string | null
  /** "x1,y1;x2,y2;...;xn,yn" 或 "...|tx,ty",首末点是两端锚点 */
  coordinate: string | null
  createBy?: string | null
  updateBy?: string | null
  status?: number | null
}

/** 定义级字段(不含 nodeList)。画布不编辑,加载后原样回写。 */
export type DefJsonBase = Omit<DefJson, 'nodeList'>

// ── 画布侧类型 ────────────────────────────────────────────────────────────

export interface Point {
  x: number
  y: number
}

export interface Size {
  w: number
  h: number
}

/** 节点四侧锚点,同时是 Svelte Flow 的句柄 id */
export type Side = 'top' | 'right' | 'bottom' | 'left'

/** 画布节点组件种类;三种网关共用 gateway */
export type NodeKind = 'start' | 'between' | 'end' | 'gateway'

/** 节点 JSON 里除几何(coordinate)与出边(skipList)之外的全部字段,原样透传 */
export type NodeJsonFields = Omit<NodeJson, 'coordinate' | 'skipList'>

/** 跳转 JSON 里除几何与两端(已在 edge.source/target)之外的全部字段,原样透传 */
export type SkipJsonFields = Omit<
  SkipJson,
  'coordinate' | 'nowNodeCode' | 'nextNodeCode'
>

// data 用 type 而非 interface:Svelte Flow 要求 data 可赋给 Record<string, unknown>,
// interface 没有隐式索引签名,不满足该约束。
export type FlowNodeData = {
  /** 标签相对节点中心的偏移;网关默认 (0, 40) 落在菱形下方 */
  labelOffset: Point
  json: NodeJsonFields
}

export type SkipEdgeData = {
  /** null = 自动正交路由;数组 = 存量折线去掉首末点后的中间点(可为空数组) */
  waypoints: Point[] | null
  /** 标签相对路径弧长中点的偏移 */
  labelOffset: Point
  json: SkipJsonFields
}

export type FlowNode = Node<FlowNodeData, NodeKind>
// data 必有:边只由 deserialize / 连线回调构造,画布上不存在没有 data 的 skip 边。
export type SkipEdge = Edge<SkipEdgeData, 'skip'> & { data: SkipEdgeData }
