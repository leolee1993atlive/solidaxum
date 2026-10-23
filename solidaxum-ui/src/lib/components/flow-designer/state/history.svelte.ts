import { cloneGraph, type Graph } from '$lib/flow/snapshot.js'

/** 历史步数上限,超出丢最旧;几十个节点的快照只有几 KB,100 步绰绰有余。 */
export const HISTORY_LIMIT = 100
/** 同一合并键在这个窗口内的连续提交合并成一步(面板逐字符输入 → 一次撤销整段回退)。 */
export const COALESCE_WINDOW_MS = 1000

/**
 * 撤销 / 重做栈(设计 §7):present 是最近一次提交的整图快照,past / future 各存
 * 快照;提交点由 DesignerStore 显式调用,不监听数组变化。
 */
export class History {
  private past = $state.raw<Graph[]>([])
  private future = $state.raw<Graph[]>([])
  private present: Graph | null = null
  private lastKey: string | null = null
  private lastAt = 0

  readonly canUndo = $derived(this.past.length > 0)
  readonly canRedo = $derived(this.future.length > 0)

  /** 加载后调用:以当前图为第 0 步,清空两个栈。 */
  reset(g: Graph) {
    this.present = cloneGraph(g)
    this.past = []
    this.future = []
    this.lastKey = null
  }

  /**
   * 记录一次提交。key 与上一次相同且间隔 < COALESCE_WINDOW_MS 时只替换 present,
   * 不把旧 present 压进 past。任何提交都清空 redo 栈。
   */
  push(g: Graph, key?: string, now = Date.now()) {
    const snap = cloneGraph(g)
    if (this.present === null) {
      this.present = snap
      return
    }
    const coalesce =
      key !== undefined &&
      key === this.lastKey &&
      now - this.lastAt < COALESCE_WINDOW_MS
    if (!coalesce) {
      this.past = [...this.past, this.present].slice(-HISTORY_LIMIT)
    }
    this.present = snap
    this.future = []
    this.lastKey = key ?? null
    this.lastAt = now
  }

  /** 回到上一步;没有可撤销的返回 null。返回的是新拷贝,调用方可直接放上画布。 */
  undo(): Graph | null {
    if (this.present === null || this.past.length === 0) return null
    const prev = this.past[this.past.length - 1]
    this.future = [this.present, ...this.future]
    this.past = this.past.slice(0, -1)
    this.present = prev
    this.lastKey = null
    return cloneGraph(prev)
  }

  redo(): Graph | null {
    if (this.present === null || this.future.length === 0) return null
    const next = this.future[0]
    this.past = [...this.past, this.present]
    this.future = this.future.slice(1)
    this.present = next
    this.lastKey = null
    return cloneGraph(next)
  }
}
