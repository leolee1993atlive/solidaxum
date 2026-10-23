// @vitest-environment jsdom
//
// 本组件渲染走 sanitizeHtml(DOMPurify),happy-dom 下 DOMPurify 输出错乱,
// 故切 jsdom。理由见 src/lib/html/sanitize.test.ts 顶部注释。
import { cleanup, render } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import SafeHtml from './SafeHtml.svelte'

afterEach(cleanup)

describe('SafeHtml', () => {
  it('渲染白名单内的富文本', () => {
    const { container } = render(SafeHtml, {
      props: { html: '<h2>标题</h2><p><strong>粗</strong></p>' },
    })
    expect(container.querySelector('h2')?.textContent).toBe('标题')
    expect(container.querySelector('strong')?.textContent).toBe('粗')
  })

  it('script 进不来', () => {
    const { container } = render(SafeHtml, {
      props: { html: '<p>正文</p><script>window.__pwned = 1</script>' },
    })
    expect(container.querySelector('script')).toBeNull()
    expect(container.textContent).toContain('正文')
  })

  it('事件属性进不来', () => {
    const { container } = render(SafeHtml, {
      props: {
        html: '<img src="https://a.com/x.png" onerror="window.__pwned = 1">',
      },
    })
    expect(container.querySelector('img')?.hasAttribute('onerror')).toBe(false)
  })

  it('带 .rich-text,拿得到富文本样式', () => {
    const { container } = render(SafeHtml, { props: { html: '<p>x</p>' } })
    expect(container.querySelector('.rich-text')).not.toBeNull()
  })

  it('空内容不炸', () => {
    const { container } = render(SafeHtml, { props: { html: '' } })
    expect(container.querySelector('.rich-text')?.innerHTML).toBe('')
  })
})
