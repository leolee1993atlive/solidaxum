// @vitest-environment jsdom
//
// ProseMirror 在 happy-dom 下行为异常(Editor 初始化触发 onUpdate 死循环,
// effect_update_depth_exceeded)。jsdom 的 DOM 实现更完整、更贴近浏览器。
import { tick } from 'svelte'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import RichTextEditor from './RichTextEditor.svelte'
import { TOOLBAR_ITEMS, URL_TOOLBAR_ITEMS } from './rich-text.js'

afterEach(cleanup)

/** ProseMirror 的可编辑区。readonly 时 contenteditable="false",所以不能按值选。 */
function editable(container: HTMLElement): HTMLElement {
  const el = container.querySelector('[contenteditable]')
  if (!el) throw new Error('编辑区没渲染出来')
  return el as HTMLElement
}

describe('RichTextEditor', () => {
  it('传入的 HTML 渲染进编辑区', async () => {
    const { container } = render(RichTextEditor, {
      props: { value: '<p><strong>粗体</strong></p>' },
    })
    await tick()
    expect(editable(container).innerHTML).toContain('<strong>粗体</strong>')
  })

  it('列表与标题都进得去(schema 没把它们过滤掉)', async () => {
    const { container } = render(RichTextEditor, {
      props: { value: '<h2>标题</h2><ul><li>项</li></ul>' },
    })
    await tick()
    const html = editable(container).innerHTML
    expect(html).toContain('<h2>')
    expect(html).toContain('<li>')
  })

  it('D7:代码块被关掉,pre 内容退化成普通段落', async () => {
    const { container } = render(RichTextEditor, {
      props: { value: '<pre><code>x = 1</code></pre>' },
    })
    await tick()
    const html = editable(container).innerHTML
    expect(html).not.toContain('<pre')
    expect(editable(container).textContent).toContain('x = 1')
  })

  it('白名单外的 h4 被 schema 挡在编辑器外(heading 限了 1-3)', async () => {
    const { container } = render(RichTextEditor, {
      props: { value: '<h4>四级</h4>' },
    })
    await tick()
    expect(editable(container).innerHTML).not.toContain('<h4')
  })

  it('readonly 时编辑区不可编辑', async () => {
    const { container } = render(RichTextEditor, {
      props: { value: '<p>只读</p>', readonly: true },
    })
    await tick()
    expect(editable(container).getAttribute('contenteditable')).toBe('false')
  })

  it('placeholder 只在空内容时出现', async () => {
    const empty = render(RichTextEditor, {
      props: { value: '', placeholder: '请输入公告内容' },
    })
    await tick()
    expect(empty.container.textContent).toContain('请输入公告内容')
    cleanup()

    const filled = render(RichTextEditor, {
      props: { value: '<p>有内容</p>', placeholder: '请输入公告内容' },
    })
    await tick()
    expect(filled.container.textContent).not.toContain('请输入公告内容')
  })

  it('minHeight 落到编辑区的内联样式上,默认 192', async () => {
    const { container } = render(RichTextEditor, { props: { value: '' } })
    await tick()
    expect(editable(container).style.minHeight).toBe('192px')
  })

  it('卸载时销毁实例,不留 ProseMirror 视图(弹窗反复开关会泄漏)', async () => {
    const { container, unmount } = render(RichTextEditor, {
      props: { value: '<p>x</p>' },
    })
    await tick()
    expect(container.querySelector('[contenteditable]')).not.toBeNull()
    unmount()
    await tick()
    expect(container.querySelector('[contenteditable]')).toBeNull()
  })

  it('工具栏按 rich-text.ts 的表逐项渲染', async () => {
    render(RichTextEditor, { props: { value: '' } })
    await tick()
    for (const item of [...TOOLBAR_ITEMS, ...URL_TOOLBAR_ITEMS]) {
      expect(screen.getByLabelText(item.label)).toBeInTheDocument()
    }
  })

  it('readonly 时整条工具栏不出现', async () => {
    render(RichTextEditor, { props: { value: '<p>x</p>', readonly: true } })
    await tick()
    expect(screen.queryByLabelText('粗体')).not.toBeInTheDocument()
  })

  it('点粗体后按钮进入按下态', async () => {
    render(RichTextEditor, { props: { value: '<p>甲</p>' } })
    await tick()
    const bold = screen.getByLabelText('粗体')
    expect(bold.getAttribute('aria-pressed')).toBe('false')
    await fireEvent.click(bold)
    await tick()
    // onTransaction 回调由 tiptap 内部异步调度,单靠 tick 不保证已跑完并驱动
    // Svelte 重渲染 aria-pressed。补一个微任务轮询,最多等 50ms 仍没翻就算红。
    for (let i = 0; i < 50; i++) {
      if (bold.getAttribute('aria-pressed') === 'true') break
      await Promise.resolve()
    }
    expect(bold.getAttribute('aria-pressed')).toBe('true')
  })

  it('点插入链接弹出地址输入框', async () => {
    render(RichTextEditor, { props: { value: '<p>甲</p>' } })
    await tick()
    await fireEvent.click(screen.getByLabelText('插入链接'))
    expect(
      screen.getByPlaceholderText(/https:\/\/example.com/)
    ).toBeInTheDocument()
  })

  it('非法地址被拒,不写进文档', async () => {
    render(RichTextEditor, { props: { value: '<p>甲</p>' } })
    await tick()
    await fireEvent.click(screen.getByLabelText('插入图片'))
    const input = screen.getByPlaceholderText(/图片地址/)
    await fireEvent.input(input, { target: { value: 'javascript:alert(1)' } })
    await fireEvent.click(screen.getByRole('button', { name: '确定' }))
    await tick()
    expect(
      document.querySelector('[contenteditable]')?.innerHTML
    ).not.toContain('img')
  })

  it('未选中文字时插入链接,地址本身成为链接文字', async () => {
    render(RichTextEditor, { props: { value: '<p>甲</p>' } })
    await tick()
    await fireEvent.click(screen.getByLabelText('插入链接'))
    const input = screen.getByPlaceholderText(/https:\/\/example.com/)
    await fireEvent.input(input, { target: { value: 'https://a.com/x' } })
    await fireEvent.click(screen.getByRole('button', { name: '确定' }))
    await tick()
    const html = document.querySelector('[contenteditable]')?.innerHTML ?? ''
    expect(html).toContain('href="https://a.com/x"')
    expect(html).toContain('https://a.com/x</a>')
  })

  it('aria-pressed 只给可切换的按钮,撤销这类一次性动作不带', async () => {
    render(RichTextEditor, { props: { value: '<p>甲</p>' } })
    await tick()
    expect(screen.getByLabelText('粗体').hasAttribute('aria-pressed')).toBe(
      true
    )
    for (const label of ['撤销', '重做', '清除格式', '分割线', '移除链接']) {
      expect(screen.getByLabelText(label).hasAttribute('aria-pressed')).toBe(
        false
      )
    }
  })

  it('改 placeholder 不重建编辑器(选区与撤销栈不能因为改个文案就没)', async () => {
    const { container, rerender } = render(RichTextEditor, {
      props: { value: '<p>甲</p>', placeholder: '旧文案' },
    })
    await tick()
    const before = editable(container)
    await rerender({ value: '<p>甲</p>', placeholder: '新文案' })
    await tick()
    expect(editable(container)).toBe(before)
  })

  it('合法地址写进文档并收起输入框', async () => {
    render(RichTextEditor, { props: { value: '<p>甲</p>' } })
    await tick()
    await fireEvent.click(screen.getByLabelText('插入图片'))
    const input = screen.getByPlaceholderText(/图片地址/)
    await fireEvent.input(input, { target: { value: 'https://a.com/x.png' } })
    await fireEvent.click(screen.getByRole('button', { name: '确定' }))
    await tick()
    expect(document.querySelector('[contenteditable]')?.innerHTML).toContain(
      'src="https://a.com/x.png"'
    )
    expect(screen.queryByPlaceholderText(/图片地址/)).not.toBeInTheDocument()
  })
})
