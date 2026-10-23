/**
 * 品牌与部署期常量的唯一出口。
 *
 * 存在的理由:下游部署可能把本仓库的共享面整体 vendor 覆盖。品牌串若散在
 * client.ts / sidebar-data.ts 里,每次同步都会撞成冲突。集中到本文件后,
 * 下游只需覆写这一个文件。
 *
 * APP_NAME 跟随产物名:构建脚本按 repo-name(.repo-name 覆盖文件 → git
 * remote)注入 VITE_APP_NAME;直接 pnpm dev/build 时兜底 'SolidAxum'。
 */
export const APP_NAME: string = import.meta.env.VITE_APP_NAME || 'SolidAxum'
export const TOKEN_KEY = 'solidaxum_token'
export const CLIENT_ID_KEY = 'solidaxum_client_id'
export const DEFAULT_CLIENT_ID = import.meta.env.VITE_CLIENT_ID || ''
// 下游覆写点:登录端点默认走本仓库后端。
export const LOGIN_ENDPOINT = '/auth/login'

/** 后端菜单未覆盖、但前端仍会用到的路径标题兜底表。 */
export const FALLBACK_TITLES: ReadonlyArray<readonly [string, string]> = [
  ['/dashboard', '首页'],
]

/** 兜底反查路径标题(菜单树未命中时调用)。最长前缀匹配,未命中返回 undefined。 */
export function getPageTitle(pathname: string): string | undefined {
  let best: { url: string; title: string } | undefined
  for (const [url, title] of FALLBACK_TITLES) {
    if (pathname === url || pathname.startsWith(url + '/')) {
      if (!best || url.length > best.url.length) best = { url, title }
    }
  }
  return best?.title
}
