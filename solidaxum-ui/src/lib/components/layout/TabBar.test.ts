import { cleanup, render } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('$app/state', () => ({
  page: { url: new URL('http://localhost/index') },
}))
vi.mock('$app/navigation', () => ({
  goto: vi.fn(() => Promise.resolve()),
}))

import TabBar from './TabBar.svelte'
import { TabsStore } from '$lib/stores/tabs.svelte.js'

afterEach(cleanup)

function setup() {
  const tabs = new TabsStore()
  tabs.open('/index', '首页', true)
  tabs.open('/system/user', '用户管理', true)
  return render(TabBar, { props: { tabs, onRefresh: () => {} } })
}

describe('TabBar 外壳', () => {
  it('容器用 bg-header 淡青底而非 bg-background', () => {
    const { container } = setup()
    const bar = container.querySelector('[data-testid="tabbar"]')
    expect(bar?.className).toContain('bg-header')
    expect(bar?.className).not.toContain('bg-background')
  })

  it('去掉下边框 —— 青绿的标签栏与内容区靠底色过渡,不用分隔线', () => {
    const { container } = setup()
    const bar = container.querySelector('[data-testid="tabbar"]')
    expect(bar?.className).not.toContain('border-b')
  })

  it('左右留白 1.25rem,与青绿一致', () => {
    const { container } = setup()
    const bar = container.querySelector('[data-testid="tabbar"]')
    expect(bar?.className).toContain('px-5')
  })

  it('标签轨道为白底,首尾切 8px 圆角', () => {
    const { container } = setup()
    const track = container.querySelector('[data-testid="tabbar-track"]')
    expect(track?.className).toContain('bg-card')
    expect(track?.className).toContain('rounded-xl')
  })

  it('仍然渲染出所有标签', () => {
    const { getByText } = setup()
    expect(getByText('首页')).toBeInTheDocument()
    expect(getByText('用户管理')).toBeInTheDocument()
  })
})
