export type TreeOptions<K extends string = string> = {
  idKey: K
  parentKey: K
  /** 顶层节点的 parent 值(如 0 或 '0')。 */
  rootValue: unknown
  childrenKey?: string
}

/**
 * 把扁平行按 parentKey 组装成树。
 * - 若行本身已带 children(后端已组装),且其 parent==rootValue,则作为顶层原样保留。
 * - parent 不在集合内的行按顶层处理(孤儿提升)。
 * 不修改入参(浅拷贝行,新增 children)。
 */
export function buildTree<T extends Record<string, unknown>>(
  rows: T[],
  opts: TreeOptions
): T[] {
  const childrenKey = opts.childrenKey ?? 'children'
  // 已是树:任一行带非空 children 数组 → 认为后端已组装,直接返回顶层过滤
  const alreadyNested = rows.some(
    (r) =>
      Array.isArray(r[childrenKey]) && (r[childrenKey] as unknown[]).length > 0
  )
  if (alreadyNested) {
    return rows.filter(
      (r) =>
        eq(r[opts.parentKey], opts.rootValue) || !hasParentIn(r, rows, opts)
    )
  }
  const byId = new Map<unknown, T & Record<string, unknown>>()
  for (const r of rows) byId.set(r[opts.idKey], { ...r })
  const roots: T[] = []
  for (const r of byId.values()) {
    const pid = r[opts.parentKey]
    const parent = byId.get(pid)
    if (parent && !eq(pid, opts.rootValue)) {
      const arr = (parent[childrenKey] as T[] | undefined) ?? []
      arr.push(r as T)
      ;(parent as Record<string, unknown>)[childrenKey] = arr
    } else {
      roots.push(r as T)
    }
  }
  return roots
}

/** Compare IDs, tolerating numeric/string coercion (e.g. 0 vs '0'), but never matching null/undefined. */
function eq(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (a == null || b == null) return false
  return String(a) === String(b)
}
function hasParentIn<T extends Record<string, unknown>>(
  r: T,
  rows: T[],
  opts: TreeOptions
): boolean {
  return rows.some((x) => eq(x[opts.idKey], r[opts.parentKey]))
}
