import { designerApi } from '$lib/api/workflow/designer.js'

/**
 * 面板用到的下拉数据,进程内缓存一次:监听器名称列表在一次设计会话里不会变,
 * 每次选中节点都去拉一遍没有意义。失败时给空列表,面板退化为纯手输。
 */
let listenerNames: string[] | null = null
let listenerPending: Promise<string[]> | null = null

export function loadListenerNames(): Promise<string[]> {
  if (listenerNames) return Promise.resolve(listenerNames)
  if (!listenerPending) {
    listenerPending = designerApi
      .listenerList()
      .then((resp) => {
        listenerNames = resp.data ?? []
        return listenerNames
      })
      .catch(() => {
        listenerPending = null
        return []
      })
  }
  return listenerPending
}

/** 仅测试用:清掉缓存 */
export function resetOptionsCache() {
  listenerNames = null
  listenerPending = null
}
