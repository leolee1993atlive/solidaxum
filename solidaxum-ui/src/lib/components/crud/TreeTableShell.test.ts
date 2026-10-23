import { cleanup, render } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import { createRawSnippet } from 'svelte'
import TreeTableShell from './TreeTableShell.svelte'

afterEach(cleanup)

type Row = { id: string; name: string; children?: Row[] }

// 查询栏是条件渲染的,要断言它就必须把 stub 传进去。
// 工具栏行则**恒定渲染**(展开/折叠全部按钮总在),不需要 stub。
const queryFormStub = createRawSnippet(() => ({
  render: () => `<div data-testid="stub-query">查询栏</div>`,
}))

const cellRendererStub = createRawSnippet(() => ({
  render: () => `<span>cell</span>`,
}))

const toolbarStub = createRawSnippet(() => ({
  render: () => `<button data-testid="stub-toolbar">工具栏内容</button>`,
}))

function setup(props: Record<string, unknown> = {}) {
  return render(TreeTableShell, {
    props: {
      data: [{ id: '1', name: '总公司', children: [{ id: '2', name: '研发部' }] }] as Row[],
      columns: [{ id: 'name', accessorKey: 'name', header: '名称' }],
      isLoading: false,
      error: null,
      treeColumnId: 'name',
      showSearch: false,
      showRefresh: false,
      cellRenderer: cellRendererStub,
      ...props,
    },
  })
}

describe('TreeTableShell 与 CrudTableShell 同形状', () => {
  it('外层是单张白卡 —— 补上一轮只做了 CrudTableShell 的遗漏', () => {
    const { container } = setup()
    const card = container.firstElementChild as HTMLElement
    expect(card.className).toContain('bg-card')
    expect(card.className).toContain('rounded-lg')
    expect(card.className).not.toContain('space-y-4')
  })

  it('表格自带内嵌圆角框', () => {
    const { container } = setup()
    const box = container.querySelector('table')!.closest('div.rounded-lg')
    expect(box?.className).toContain('m-3')
    expect(box?.className).toContain('overflow-hidden')
  })

  it('渲染左对齐小标题', () => {
    const { container } = setup({ title: '部门列表' })
    expect(
      container.querySelector('[data-testid="shell-title"]')?.textContent
    ).toBe('部门列表')
  })

  it('查询栏独立成行,与下方工具栏之间无分割线', () => {
    const { container } = setup({ queryForm: queryFormStub })
    const row = container.querySelector('[data-testid="shell-query"]')
    expect(row?.className).not.toContain('border-b')
  })

  it('工具栏行与表格之间没有分割线', () => {
    const { container } = setup({ title: '部门列表' })
    const row = container.querySelector('[data-testid="shell-toolbar"]')!
    expect(row.className).not.toContain('border-b')
    expect(row.className).not.toContain('border-t')
  })

  it('展开/折叠全部按钮并入工具栏右侧组', () => {
    const { container, getByRole } = setup()
    const actions = container.querySelector('[data-testid="shell-actions"]')!
    expect(actions.contains(getByRole('button', { name: /展开全部|折叠全部/ }))).toBe(true)
  })

  it('toolbar snippet 渲染进右侧组', () => {
    const { container } = setup({ toolbar: toolbarStub })
    const actions = container.querySelector('[data-testid="shell-actions"]')!
    expect(actions.querySelector('[data-testid="stub-toolbar"]')).not.toBeNull()
  })
})
