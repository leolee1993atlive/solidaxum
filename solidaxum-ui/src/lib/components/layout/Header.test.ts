import { cleanup, render } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import Header from './Header.svelte'

afterEach(cleanup)

describe('Header 外壳', () => {
  it('使用 bg-header 令牌而非继承 bg-background', () => {
    const { container } = render(Header)
    const header = container.querySelector('header')
    expect(header?.className).toContain('bg-header')
  })

  it('左上角 24px 圆角 —— 主区域成为浮在品牌色上的卡片', () => {
    const { container } = render(Header)
    const header = container.querySelector('header')
    expect(header?.className).toContain('rounded-tl-[24px]')
  })

  it('外部传入的 class 仍能合并,不被覆盖', () => {
    const { container } = render(Header, { props: { class: 'custom-marker' } })
    const header = container.querySelector('header')
    expect(header?.className).toContain('custom-marker')
    expect(header?.className).toContain('bg-header')
  })
})
