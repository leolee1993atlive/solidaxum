import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

// happy-dom 环境下 import.meta.url 不是 file://,沿用本仓
// menu-route-drift/menu-coverage 的 process.cwd() 源码扫描惯例。
const layout = readFileSync(
  join(process.cwd(), 'src', 'routes', '(app)', '+layout.svelte'),
  'utf8'
)

describe('外壳骨架(+layout.svelte)', () => {
  it('侧栏定宽 184px —— 复刻 Vben 的 sidebar.width', () => {
    expect(layout).toContain('--sidebar-width: 184px')
  })

  it('根容器铺 --gradient-shell', () => {
    expect(layout).toContain('var(--gradient-shell)')
  })

  it('SidebarInset 透明 —— Header 圆角缺口才能透出根渐变', () => {
    expect(layout).toMatch(/<SidebarInset[^>]*bg-transparent/)
  })

  it('内容包裹层铺 --gradient-content', () => {
    expect(layout).toContain('var(--gradient-content)')
  })
})
