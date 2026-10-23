/**
 * 隐藏路由(getRouters 里 hidden=true 的叶子菜单)查找表。
 * 这些页面不在侧边栏菜单树里,但后端仍返回其 meta.title / meta.activeMenu:
 * - pathToTitle 用它给 tab/文档标题取名(否则回退成原始 path);
 * - NavTreeItem 用 activeMenu 计算激活路径,让所属菜单保持展开/高亮
 *   (对齐 Vue 原版 el-menu 的 default-active = meta.activeMenu 行为)。
 * 数据由 auth 在加载菜单时注入(setHiddenRouteMap),模式与 tab-cache.ts 一致。
 */
export type HiddenRouteInfo = {
  title: string
  /** 激活路径:隐藏页打开时应高亮/展开的菜单路径(DB sys_menu.active_menu)。 */
  activeMenu?: string
}

let routes: Record<string, HiddenRouteInfo> = {}
/** 动态段路径模式编译出的正则缓存。 */
const regexCache = new Map<string, RegExp>()

export function setHiddenRouteMap(map: Record<string, HiddenRouteInfo>): void {
  routes = map
  regexCache.clear()
}

/**
 * 把可能含 SvelteKit 动态段的路径模式编译成正则:
 * [id] 匹配必填一段,[[id]] 匹配可选一段(可整体缺席)。
 */
function patternToRegex(pattern: string): RegExp {
  let re = regexCache.get(pattern)
  if (!re) {
    let src = '^'
    for (const seg of pattern.split('/')) {
      if (seg === '') continue // 前导 / 产生的空段
      if (/^\[\[.+\]\]$/.test(seg)) {
        src += '(?:/[^/]+)?'
      } else if (/^\[.+\]$/.test(seg)) {
        src += '/[^/]+'
      } else {
        src += '/' + seg.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      }
    }
    src += '$'
    re = new RegExp(src)
    regexCache.set(pattern, re)
  }
  return re
}

/** 查当前 URL 对应的隐藏路由信息:先精确命中,再按动态段模式匹配。 */
export function hiddenRouteFor(pathname: string): HiddenRouteInfo | undefined {
  const exact = routes[pathname]
  if (exact) return exact
  for (const pattern in routes) {
    if (patternToRegex(pattern).test(pathname)) return routes[pattern]
  }
  return undefined
}
