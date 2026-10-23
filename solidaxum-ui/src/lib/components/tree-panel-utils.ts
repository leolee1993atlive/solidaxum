import type { TreeSelectNode } from './tree-select-types.js'

/**
 * 按名称过滤树:命中节点保留其**整棵子树**,并保留其**祖先链**(否则命中节点会因父节点未命中而消失)。
 * 对齐 Vue TreePanel 的 el-tree `filter-node-method` 行为。
 */
export function filterTreeByLabel(
  nodes: TreeSelectNode[],
  keyword: string
): TreeSelectNode[] {
  const kw = keyword.trim()
  if (kw === '') return nodes
  const out: TreeSelectNode[] = []
  for (const node of nodes) {
    if (node.label.includes(kw)) {
      out.push(node)
      continue
    }
    const children = node.children ? filterTreeByLabel(node.children, kw) : []
    if (children.length > 0) out.push({ ...node, children })
  }
  return out
}

/** 收集全部含子节点的节点 id —— 面板默认全展开(对齐 Vue default-expand-all)。 */
export function collectExpandableIds(
  nodes: TreeSelectNode[]
): (string | number)[] {
  return nodes.flatMap((n) =>
    n.children && n.children.length > 0
      ? [n.id, ...collectExpandableIds(n.children)]
      : []
  )
}
