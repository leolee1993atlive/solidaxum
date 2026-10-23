import type { NodeKind } from '$lib/flow/types.js'

export interface NodeMenuItem {
  kind: NodeKind
  nodeType: number
  label: string
}

/** 可添加的节点种类,面板与右键菜单共用一份;顺序即显示顺序。 */
export const NODE_MENU: readonly NodeMenuItem[] = [
  { kind: 'between', nodeType: 1, label: '审批人' },
  { kind: 'gateway', nodeType: 3, label: '互斥网关' },
  { kind: 'gateway', nodeType: 4, label: '并行网关' },
  { kind: 'gateway', nodeType: 5, label: '包容网关' },
  { kind: 'start', nodeType: 0, label: '开始' },
  { kind: 'end', nodeType: 2, label: '结束' },
]

/** 面板拖拽的 dataTransfer 类型:载荷是 NodeMenuItem 的 JSON。 */
export const NODE_DRAG_MIME = 'application/x-fd-node'
/**
 * 拖的是审批 / 网关时额外带的空类型。dragover 阶段读不到 getData(浏览器保护模式),
 * 只能看 types,靠它决定要不要去找鼠标下的跳转做落线插入。
 */
export const NODE_DRAG_INSERTABLE = 'application/x-fd-insertable'

export function isInsertable(kind: NodeKind): boolean {
  return kind === 'between' || kind === 'gateway'
}
