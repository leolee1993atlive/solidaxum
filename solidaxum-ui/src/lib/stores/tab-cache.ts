let cacheMap: Record<string, boolean> = {}

export function setCacheMap(map: Record<string, boolean>): void {
  cacheMap = map
}
/** 未知/隐藏路径默认不缓存。 */
export function isCacheable(path: string): boolean {
  return cacheMap[path] === true
}
