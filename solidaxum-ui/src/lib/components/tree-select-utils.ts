import type { TreeSelectNode } from './tree-select-types.js'

/**
 * 移除 excludeIds 中的节点**及其全部子孙**。
 * 用于「编辑部门时不能把自己挂到自己的子孙下」——
 * 只移除自身而保留子孙,用户仍可选中子孙造成循环挂载。
 */
export function pruneSubtrees(
  nodes: TreeSelectNode[],
  excludeIds: (string | number)[]
): TreeSelectNode[] {
  if (excludeIds.length === 0) return nodes
  const excluded = new Set(excludeIds.map(String))
  return nodes
    .filter((n) => !excluded.has(String(n.id)))
    .map((n) => ({
      ...n,
      children: n.children ? pruneSubtrees(n.children, excludeIds) : undefined,
    }))
}

/** 深度优先展平(用于测试断言与键盘导航)。 */
export function flattenVisible(nodes: TreeSelectNode[]): TreeSelectNode[] {
  return nodes.flatMap((n) => [
    n,
    ...(n.children ? flattenVisible(n.children) : []),
  ])
}

/** 按 id 查节点名称(回显用)。 */
export function findNodeLabel(
  nodes: TreeSelectNode[],
  id: string | number | undefined | null
): string | undefined {
  if (id === undefined || id === null) return undefined
  return flattenVisible(nodes).find((n) => String(n.id) === String(id))?.label
}

/**
 * 把 buildTree 产物(保留原始行字段)递归映射为 TreeSelectNode。
 * buildTree 不做 id/label 归一化 —— dept 树节点是 {deptId, deptName, children},
 * 必须经此映射后才能喂给 TreeSelect。
 */
export function toTreeSelectNodes<T extends Record<string, unknown>>(
  rows: T[],
  keys: { idKey: keyof T; labelKey: keyof T; childrenKey?: keyof T }
): TreeSelectNode[] {
  const childrenKey = keys.childrenKey ?? ('children' as keyof T)
  return rows.map((r) => {
    const children = r[childrenKey] as T[] | undefined
    const node: TreeSelectNode = {
      id: String(r[keys.idKey]),
      label: (r[keys.labelKey] as string | undefined) ?? '',
    }
    if (children && children.length > 0)
      node.children = toTreeSelectNodes(children, keys)
    return node
  })
}
