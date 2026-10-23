import { describe, it, expect, beforeEach } from 'vitest'
import { TabsStore, HOME_TAB } from './tabs.svelte.js'

beforeEach(() => sessionStorage.clear())

describe('TabsStore', () => {
  it('starts with affix home tab', () => {
    const s = new TabsStore()
    expect(s.tabs.map((t) => t.path)).toEqual([HOME_TAB.path])
    expect(s.tabs[0].affix).toBe(true)
  })
  it('opens and dedupes by path', () => {
    const s = new TabsStore()
    s.open('/system/user', '用户管理', true)
    s.open('/system/user', '用户管理', true)
    expect(s.tabs.filter((t) => t.path === '/system/user')).toHaveLength(1)
  })
  // Regression: open() must NOT reassign this.tabs when nothing changed.
  // Reassigning a $state array unconditionally caused a layout $effect that
  // reads tabs (via pathToTitle/auth.navTree) to re-run and call open() again,
  // producing an infinite update loop (effect_update_depth_exceeded) that froze
  // navigation. open() must be a no-op (same array identity) when the tab is
  // already present with identical title + cacheable.
  it('does not reassign tabs when open() is a no-op (same title)', () => {
    const s = new TabsStore()
    s.open('/a', 'A', false)
    const before = s.tabs
    s.open('/a', 'A', false)
    expect(s.tabs).toBe(before) // identical reference => no $state invalidation
  })
  it('closeOthers keeps affix + target', () => {
    const s = new TabsStore()
    s.open('/a', 'A', false)
    s.open('/b', 'B', false)
    s.closeOthers('/b')
    expect(s.tabs.map((t) => t.path)).toEqual([HOME_TAB.path, '/b'])
  })
  it('closeRight keeps left of target', () => {
    const s = new TabsStore()
    s.open('/a', 'A', false)
    s.open('/b', 'B', false)
    s.open('/c', 'C', false)
    s.closeRight('/a')
    expect(s.tabs.map((t) => t.path)).toEqual([HOME_TAB.path, '/a'])
  })
  it('closeAll keeps only affix', () => {
    const s = new TabsStore()
    s.open('/a', 'A', false)
    s.closeAll()
    expect(s.tabs.map((t) => t.path)).toEqual([HOME_TAB.path])
  })
  // Regression: 批量关闭当前激活的 tab 后,goto(目标) 是异步的;在导航落地
  // 之前,布局 $effect 仍读到旧 page.url.pathname 并重新 open 那条路径,
  // 导致"全部关闭"后当前 tab 又被加回来("关不掉")。close 类操作必须把删除的
  // 路径加入护栏,使 open() 在同一导航周期内忽略它们,直到 clearClosingGuard()。
  it('open() ignores just-closed paths until clearClosingGuard', () => {
    const s = new TabsStore()
    s.open('/system/user', '用户管理', true)
    s.closeAll()
    // 模拟陈旧 $effect 用旧 path 再 open
    s.open('/system/user', '用户管理', true)
    expect(s.tabs.map((t) => t.path)).toEqual([HOME_TAB.path])
    // 导航落地后清护栏,真正的再次访问应能重新打开
    s.clearClosingGuard()
    s.open('/system/user', '用户管理', true)
    expect(s.tabs.map((t) => t.path)).toEqual([HOME_TAB.path, '/system/user'])
  })
  it('close returns the next path to navigate to', () => {
    const s = new TabsStore()
    s.open('/a', 'A', false)
    s.open('/b', 'B', false)
    expect(s.close('/b')).toBe('/a') // 关右邻,回左邻
  })
  it('persists to sessionStorage and reloads', () => {
    const s1 = new TabsStore()
    s1.open('/a', 'A', false)
    const s2 = new TabsStore()
    expect(s2.tabs.map((t) => t.path)).toContain('/a')
  })
})
