import type { RouterVo } from '$lib/api/types.js'

function normalizeBase(path: string): string {
  if (!path) return '/'
  return path.startsWith('/') ? path : `/${path}`
}
function joinPath(base: string, child: string): string {
  if (!child) return base
  if (child.startsWith('http://') || child.startsWith('https://')) return child
  if (child.startsWith('/')) return child
  const b = base.endsWith('/') ? base.slice(0, -1) : base
  return `${b}/${child}`
}

/** path -> 是否可缓存(cacheable = meta.noCache !== true)。仅叶子路径入表。 */
export function routersToCacheMap(
  routers: RouterVo[]
): Record<string, boolean> {
  const map: Record<string, boolean> = {}
  const walk = (rs: RouterVo[], parentPath: string) => {
    for (const r of rs) {
      if (r.meta?.link) continue
      const base =
        parentPath === '' ? normalizeBase(r.path) : joinPath(parentPath, r.path)
      const children = (r.children ?? []).filter((c) => !c.meta?.link)
      if (children.length === 0) {
        map[base] = r.meta?.noCache !== true
      } else {
        walk(children, base)
      }
    }
  }
  walk(routers, '')
  return map
}
