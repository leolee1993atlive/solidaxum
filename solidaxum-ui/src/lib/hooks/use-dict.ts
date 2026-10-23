import type { DictDataVo } from '$lib/api/system/dict.js'
import { dictApi } from '$lib/api/system/dict.js'

/** 内存缓存: dictType -> DictDataVo[]。导出供测试清空。 */
export const dictCache = new Map<string, DictDataVo[]>()

type Fetcher = (dictType: string) => Promise<DictDataVo[]>

/** 纯函数: 带缓存的拉取,fetcher 注入便于测试。 */
export async function fetchDict(
  dictType: string,
  fetcher: Fetcher = (t) => dictApi.getByType(t).then((r) => r.data ?? [])
): Promise<DictDataVo[]> {
  const cached = dictCache.get(dictType)
  if (cached) return cached
  const list = await fetcher(dictType)
  dictCache.set(dictType, list)
  return list
}

/** 纯函数: 根据值查标签。 */
export function resolveDictLabel(
  list: DictDataVo[],
  value: string | number | undefined | null
): string {
  if (value === undefined || value === null) return ''
  const s = String(value)
  const hit = list.find((d) => d.dictValue === s)
  return hit ? hit.dictLabel : s
}

/** 纯函数: 根据值查 listClass(用于 Badge 样式)。 */
export function resolveDictClass(
  list: DictDataVo[],
  value: string | number | undefined | null
): string {
  if (value === undefined || value === null) return ''
  const s = String(value)
  const hit = list.find((d) => d.dictValue === s)
  return hit?.listClass ?? ''
}

/** 纯函数: 根据值查 cssClass(RuoYi dict-tag 的自定义 class 直传通道)。 */
export function resolveDictCssClass(
  list: DictDataVo[],
  value: string | number | undefined | null
): string {
  if (value === undefined || value === null) return ''
  const s = String(value)
  const hit = list.find((d) => d.dictValue === s)
  return hit?.cssClass ?? ''
}
