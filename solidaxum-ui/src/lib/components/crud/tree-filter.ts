/**
 * 树过滤:命中节点连同其祖先一起保留 —— 否则命中的子节点会因父节点被滤掉而消失。
 * 未命中但有命中后代的父节点也保留,children 只留命中的分支。
 *
 * 提取自 menu 页的内联实现,dept 页复用。
 * 纯函数:不改动入参,只在需要时浅拷贝命中节点。
 */
export function filterTreeNodes<T extends Record<string, unknown>>(
  nodes: T[],
  hit: (node: T) => boolean,
  childrenKey = 'children'
): T[] {
  const walk = (list: T[]): T[] => {
    const out: T[] = []
    for (const n of list) {
      const kids = walk((n[childrenKey] as T[] | undefined) ?? [])
      if (hit(n) || kids.length > 0) out.push({ ...n, [childrenKey]: kids })
    }
    return out
  }
  // 无人被滤掉(未输入任何筛选条件的常见场景)时原样返回,避免整棵树无谓拷贝。
  return hasFiltered(nodes, hit, childrenKey) ? walk(nodes) : nodes
}

/** 是否存在任一节点(含后代)不命中 —— 有则必须走 walk 重建。 */
function hasFiltered<T extends Record<string, unknown>>(
  nodes: T[],
  hit: (node: T) => boolean,
  childrenKey: string
): boolean {
  for (const n of nodes) {
    if (!hit(n)) return true
    const kids = (n[childrenKey] as T[] | undefined) ?? []
    if (hasFiltered(kids, hit, childrenKey)) return true
  }
  return false
}
