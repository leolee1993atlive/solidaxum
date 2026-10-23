import type { FlowNode, SkipEdge } from './types.js'

/**
 * 节点上几个"多值拼串"字段的编解码,以及面板需要的图查询。
 * 拼串格式来自 LogicFlow 版 propertySetting.vue:监听器类型逗号分隔、路径 "@@" 分隔、
 * 办理人 "@@" 分隔。
 */

export const LISTENER_TYPES = [
  { value: 'start', label: '开始' },
  { value: 'assignment', label: '分派' },
  { value: 'finish', label: '完成' },
  { value: 'create', label: '创建' },
] as const

export interface ListenerRow {
  listenerType: string
  listenerPath: string
}

/** 类型与路径按下标配对;长度不齐时缺的一侧补空串。 */
export function parseListeners(
  listenerType: string | null | undefined,
  listenerPath: string | null | undefined
): ListenerRow[] {
  const types = listenerType ? listenerType.split(',') : []
  const paths = listenerPath ? listenerPath.split('@@') : []
  const n = Math.max(types.length, paths.length)
  return Array.from({ length: n }, (_, i) => ({
    listenerType: types[i] ?? '',
    listenerPath: paths[i] ?? '',
  }))
}

/** 全空行丢弃;没有任何行时两个字段都写 null。 */
export function formatListeners(rows: ListenerRow[]): {
  listenerType: string | null
  listenerPath: string | null
} {
  const kept = rows.filter(
    (r) => r.listenerType.trim() || r.listenerPath.trim()
  )
  if (kept.length === 0) return { listenerType: null, listenerPath: null }
  return {
    listenerType: kept.map((r) => r.listenerType.trim()).join(','),
    listenerPath: kept.map((r) => r.listenerPath.trim()).join('@@'),
  }
}

export function splitPermissionFlag(s: string | null | undefined): string[] {
  return s ? s.split('@@').filter((x) => x !== '') : []
}

export function joinPermissionFlag(list: string[]): string | null {
  const kept = list.map((x) => x.trim()).filter((x) => x !== '')
  return kept.length ? kept.join('@@') : null
}

/**
 * "驳回到指定节点"的候选:沿 PASS 边逆向能到达的祖先节点,去掉开始与网关,
 * 按回溯到达的先后排序(近的在前),并排除节点自身(PASS 环可达时)。
 * 与 LogicFlow 版 tool.js getPreviousNodes 不同:它不去类型、按节点表倒序;
 * 这里有意收紧——驳回的目标是审批节点,开始/网关没有可办内容。
 */
export function previousNodes(
  nodes: FlowNode[],
  edges: SkipEdge[],
  nodeCode: string
): FlowNode[] {
  const pass = edges.filter((e) => (e.data.json.skipType ?? 'PASS') === 'PASS')
  const order: string[] = []
  const visited = new Set<string>()
  const walk = (code: string) => {
    if (visited.has(code)) return
    visited.add(code)
    for (const e of pass) {
      if (e.target === code) {
        if (!order.includes(e.source)) order.push(e.source)
        walk(e.source)
      }
    }
  }
  walk(nodeCode)
  const byId = new Map(nodes.map((n) => [n.id, n]))
  return order
    .map((id) => byId.get(id))
    .filter(
      (n): n is FlowNode =>
        !!n && n.type !== 'start' && n.type !== 'gateway' && n.id !== nodeCode
    )
}
