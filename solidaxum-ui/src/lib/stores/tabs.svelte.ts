export type Tab = {
  path: string
  title: string
  cacheable: boolean
  affix?: boolean
}

// 首页固定页签。HOME 路径以实际 dashboard 路由为准(此处为 /index)。
export const HOME_TAB: Tab = {
  path: '/index',
  title: '首页',
  cacheable: false,
  affix: true,
}

const KEY = 'axum_tabs'

export class TabsStore {
  tabs = $state<Tab[]>([HOME_TAB])
  /**
   * 批量关闭操作的护栏。closeAll/closeOthers/closeRight 删除的路径会进
   * 入此集合,使 open() 在同一导航周期内忽略它们。否则 goto(target) 尚
   * 未完成时布局 $effect 仍读到旧的 page.url.pathname 并把刚关掉的 tab
   * 重新 open 出来(实测会"关不掉")。goto 完成后由 clearClosingGuard() 清空。
   */
  private closingPaths = new Set<string>()

  constructor() {
    if (typeof sessionStorage !== 'undefined') {
      const raw = sessionStorage.getItem(KEY)
      if (raw) {
        try {
          const arr = JSON.parse(raw) as Tab[]
          if (Array.isArray(arr) && arr.length > 0) {
            // 确保 affix 首页始终在首位且存在
            this.tabs = [
              HOME_TAB,
              ...arr.filter((t) => t.path !== HOME_TAB.path),
            ]
          }
        } catch {
          /* ignore */
        }
      }
    }
  }

  private persist() {
    if (typeof sessionStorage !== 'undefined') {
      sessionStorage.setItem(
        KEY,
        JSON.stringify(this.tabs.filter((t) => !t.affix))
      )
    }
  }

  private clearCache(path: string) {
    if (typeof sessionStorage === 'undefined') return
    sessionStorage.removeItem(`crud:${path}`)
    sessionStorage.removeItem(`crud:extra:${path}`)
    sessionStorage.removeItem(`scroll:${path}`)
  }

  /** 清空批量关闭护栏。TabBar 在 goto 完成后调用,恢复 open() 正常行为。 */
  clearClosingGuard() {
    this.closingPaths.clear()
  }

  open(path: string, title: string, cacheable: boolean) {
    if (path === HOME_TAB.path) return // 首页固定,不更新标题/状态
    // 忽略刚被批量关闭的路径(防止陈旧 $effect 把关掉的 tab 重新 open 出来)。
    if (this.closingPaths.has(path)) return
    const existing = this.tabs.find((t) => t.path === path)
    if (existing) {
      // 标题/cacheable 变了才写,避免 layout $effect 因重赋值 tabs 而死循环。
      if (existing.title === title && existing.cacheable === cacheable) return
      this.tabs = this.tabs.map((t) =>
        t.path === path ? { ...t, title, cacheable } : t
      )
    } else {
      this.tabs = [...this.tabs, { path, title, cacheable }]
    }
    this.persist()
  }

  /** 关闭一个 tab,返回应导航到的 path(若关的是当前页,调用方据此 goto)。 */
  close(path: string): string {
    const idx = this.tabs.findIndex((t) => t.path === path)
    if (idx < 0 || this.tabs[idx].affix) return path
    const next =
      this.tabs[idx - 1]?.path ?? this.tabs[idx + 1]?.path ?? HOME_TAB.path
    this.clearCache(path)
    this.closingPaths.add(path)
    this.tabs = this.tabs.filter((t) => t.path !== path)
    this.persist()
    return next
  }

  closeOthers(path: string) {
    for (const t of this.tabs) {
      if (!t.affix && t.path !== path) {
        this.clearCache(t.path)
        this.closingPaths.add(t.path)
      }
    }
    this.tabs = this.tabs.filter((t) => t.affix || t.path === path)
    this.persist()
  }

  closeRight(path: string) {
    const idx = this.tabs.findIndex((t) => t.path === path)
    if (idx < 0) return
    for (let i = idx + 1; i < this.tabs.length; i++) {
      const t = this.tabs[i]
      if (!t.affix) {
        this.clearCache(t.path)
        this.closingPaths.add(t.path)
      }
    }
    this.tabs = this.tabs.filter((t, i) => i <= idx || t.affix)
    this.persist()
  }

  closeAll() {
    for (const t of this.tabs) {
      if (!t.affix) {
        this.clearCache(t.path)
        this.closingPaths.add(t.path)
      }
    }
    this.tabs = this.tabs.filter((t) => t.affix)
    this.persist()
  }
}
