import { parseSkipCondition, validateSkipCondition } from './condition.js'
import { parseListeners } from './node-fields.js'
import { parseNodeRatio, validateNodeRatio } from './ratio.js'
import type { FlowNode, SkipEdge } from './types.js'

export interface ValidationIssue {
  /** error 会阻止保存(数据会坏);warning 只提醒,保存半成品是允许的 */
  level: 'error' | 'warning'
  /** 出问题的节点 id 或边 id,面板据此定位;定义级问题 id 为空串 */
  target: { kind: 'node' | 'edge'; id: string }
  message: string
}

/**
 * 保存前的定义级校验。老设计器允许保存任何半成品,这里也一样:
 * 只有会把数据存坏的(编码空/重复)才算 error,其余(缺开始结束、悬空节点、
 * 引擎不认的表达式、策略格式)都是 warning,由用户决定是否照存。
 */
export function validateDefinition(
  nodes: FlowNode[],
  edges: SkipEdge[]
): ValidationIssue[] {
  const issues: ValidationIssue[] = []
  const push = (
    level: ValidationIssue['level'],
    kind: 'node' | 'edge',
    id: string,
    message: string
  ) => issues.push({ level, target: { kind, id }, message })

  const seen = new Map<string, number>()
  for (const n of nodes) seen.set(n.id, (seen.get(n.id) ?? 0) + 1)

  if (!nodes.some((n) => n.type === 'start'))
    push('warning', 'node', '', '流程缺少开始节点')
  if (!nodes.some((n) => n.type === 'end'))
    push('warning', 'node', '', '流程缺少结束节点')

  for (const n of nodes) {
    const name = n.data.json.nodeName || n.id
    if (n.id.trim() === '') push('error', 'node', n.id, '节点编码不能为空')
    else if ((seen.get(n.id) ?? 0) > 1)
      push('error', 'node', n.id, `节点编码 ${n.id} 重复`)

    if (n.type === 'between') {
      const ratioError = validateNodeRatio(
        parseNodeRatio(n.data.json.nodeRatio)
      )
      if (ratioError)
        push('warning', 'node', n.id, `${name}:票签策略 ${ratioError}`)
    }
    const rows = parseListeners(
      n.data.json.listenerType,
      n.data.json.listenerPath
    )
    if (rows.some((r) => !r.listenerType.trim() || !r.listenerPath.trim())) {
      push('warning', 'node', n.id, `${name}:监听器的类型与路径都要填`)
    }
    if (n.type !== 'end' && !edges.some((e) => e.source === n.id)) {
      push('warning', 'node', n.id, `${name}:没有出线`)
    }
    if (n.type !== 'start' && !edges.some((e) => e.target === n.id)) {
      push('warning', 'node', n.id, `${name}:没有入线`)
    }
  }
  for (const e of edges) {
    const error = validateSkipCondition(
      parseSkipCondition(e.data.json.skipCondition)
    )
    if (error) {
      const label = e.data.json.skipName || `${e.source} → ${e.target}`
      push('warning', 'edge', e.id, `跳转 ${label}:${error}`)
    }
  }
  return issues
}
