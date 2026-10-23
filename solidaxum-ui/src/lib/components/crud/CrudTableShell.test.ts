import { cleanup, render, screen, fireEvent } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createRawSnippet, type Snippet, type Component } from 'svelte'
import { createTable, getCoreRowModel, type Table } from '@tanstack/table-core'
import { TABLE_DEFAULT_STATE } from '$lib/table-defaults.js'
import CrudTableShell from './CrudTableShell.svelte'

type Row = { id: string; name: string }

// Svelte 5 泛型组件在测试 render 中无法自动推断 TRow,需要显式收窄组件类型。
const CrudTableShellOfRow = CrudTableShell as unknown as Component<{
  table: Table<Row>
  isLoading: boolean
  error: string | null
  columnCount: number
  searchPlaceholder?: string
  emptyMessage?: string
  globalFilter: string
  onSearchInput?: (value: string) => void
  onRefresh?: () => void
  showSearch?: boolean
  showRefresh?: boolean
  selectable?: boolean
  cellRenderer: Snippet<[{ row: Row; columnId?: string }]>
  customCellIds?: Set<string>
  title?: string
  queryForm?: Snippet
  toolbar?: Snippet<[{ selectedCount: number }]>
  onRowClick?: (row: Row) => void
  rowClass?: (row: Row) => string
}>

afterEach(cleanup)

// 两行都是**条件渲染**的:不传 snippet 就整行不出现,断言会拿到 null。
// 故凡是要断言某一行的用例,都必须把对应的 stub 传进去。
const queryFormStub = createRawSnippet(() => ({
  render: () => `<div data-testid="stub-query">查询栏</div>`,
}))
const toolbarStub = createRawSnippet(() => ({
  render: () => `<button type="button">新增</button>`,
}))
const cellRendererStub = createRawSnippet(() => ({
  render: () => `<span>cell</span>`,
}))

const headingStub = createRawSnippet(() => ({
  render: () => `<div role="tablist"><button role="tab">已发布</button></div>`,
}))

/** 造一个最小可用的 TanStack 表 —— 只需能渲染出一行一列。 */
function makeTable(rows: Row[] = [{ id: '1', name: '甲' }]) {
  return createTable<Row>({
    data: rows,
    columns: [{ id: 'name', accessorKey: 'name', header: '名称' }],
    state: { ...TABLE_DEFAULT_STATE },
    onStateChange: () => {},
    getCoreRowModel: getCoreRowModel(),
    renderFallbackValue: null,
  })
}

function setup(props: Record<string, unknown> = {}) {
  return render(CrudTableShellOfRow, {
    props: {
      table: makeTable(),
      isLoading: false,
      error: null,
      columnCount: 1,
      globalFilter: '',
      showSearch: false,
      showRefresh: false,
      cellRenderer: cellRendererStub,
      ...props,
    },
  })
}

describe('CrudTableShell 新形状', () => {
  it('渲染左对齐小标题', () => {
    const { container } = setup({ title: '用户列表', toolbar: toolbarStub })
    const el = container.querySelector('[data-testid="shell-title"]')
    expect(el?.textContent).toBe('用户列表')
  })

  it('heading 占标题的位置并取代 title;只传 heading 也撑起工具栏行', () => {
    const { container } = setup({ title: '定义列表', heading: headingStub })
    const heading = container.querySelector('[data-testid="shell-heading"]')
    expect(heading?.querySelector('[role="tab"]')?.textContent).toBe('已发布')
    expect(container.querySelector('[data-testid="shell-title"]')).toBeNull()
    const row = container.querySelector('[data-testid="shell-toolbar"]')
    expect(row?.firstElementChild).toBe(heading)

    const only = setup({ heading: headingStub })
    expect(
      only.container.querySelector('[data-testid="shell-toolbar"]')
    ).not.toBeNull()
  })

  it('不传 title 时不渲染标题元素,工具栏仍右对齐', () => {
    const { container } = setup({ toolbar: toolbarStub })
    expect(container.querySelector('[data-testid="shell-title"]')).toBeNull()
    const actions = container.querySelector('[data-testid="shell-actions"]')
    expect(actions?.className).toContain('ml-auto')
  })

  it('查询栏独立成行,与下方工具栏之间无分割线', () => {
    const { container } = setup({ queryForm: queryFormStub })
    const row = container.querySelector('[data-testid="shell-query"]')
    expect(row?.className).not.toContain('border-b')
  })

  it('三者都不传时两行都不渲染,不留空白条', () => {
    const { container } = setup()
    expect(container.querySelector('[data-testid="shell-query"]')).toBeNull()
    expect(container.querySelector('[data-testid="shell-toolbar"]')).toBeNull()
  })

  it('工具栏行与表格之间没有分割线', () => {
    const { container } = setup({ title: '用户列表', toolbar: toolbarStub })
    const row = container.querySelector('[data-testid="shell-toolbar"]')!
    expect(row.className).not.toContain('border-b')
    expect(row.className).not.toContain('border-t')
  })

  it('外层仍是单张白卡,表格自带内嵌圆角框', () => {
    const { container } = setup()
    const card = container.firstElementChild as HTMLElement
    expect(card.className).toContain('bg-card')
    const box = container.querySelector('table')!.closest('div.rounded-lg')
    expect(box?.className).toContain('m-3')
    expect(box?.className).toContain('border')
  })
})

describe('CrudTableShell 工具栏 v2', () => {
  it('segmented 渲染在标题右侧,点击回传选项值', async () => {
    const onchange = vi.fn()
    const { container } = setup({
      title: '数据列表',
      segmented: {
        options: [
          { label: '全部', value: '' },
          { label: '待提交', value: '1' },
        ],
        value: '',
        onchange,
      },
    })
    const toolbar = container.querySelector('[data-testid="shell-toolbar"]')!
    // 标题之后、右侧动作区之前
    expect(toolbar.children[1].getAttribute('data-slot')).toBe('segmented-control')
    await fireEvent.click(screen.getByRole('button', { name: '待提交' }))
    expect(onchange).toHaveBeenCalledWith('1')
  })

  it('onExport 渲染导出按钮并触发回调', async () => {
    const onExport = vi.fn()
    setup({ title: '列表', onExport })
    await fireEvent.click(screen.getByRole('button', { name: '导出' }))
    expect(onExport).toHaveBeenCalledTimes(1)
  })

  it('onBatchDelete 未勾选行时禁用,不传 selectable 时可点', async () => {
    const onBatchDelete = vi.fn()
    setup({ title: '列表', onBatchDelete })
    const btn = screen.getByRole('button', { name: /删除/ })
    expect(btn.hasAttribute('disabled')).toBe(false)
    await fireEvent.click(btn)
    expect(onBatchDelete).toHaveBeenCalledTimes(1)
  })

  it('columnSettings 渲染列设置入口,默认不传则不渲染', () => {
    setup({ title: '列表', columnSettings: true })
    expect(screen.getByRole('button', { name: '列设置' })).toBeTruthy()
  })

  it('fullscreen 渲染全屏切换按钮,点击后容器变 fixed 全屏', async () => {
    const { container } = setup({ title: '列表', fullscreen: true })
    const card = container.firstElementChild as HTMLElement
    expect(card.className).not.toContain('fixed')
    await fireEvent.click(screen.getByRole('button', { name: '全屏' }))
    expect(card.className).toContain('fixed')
    await fireEvent.click(screen.getByRole('button', { name: '退出全屏' }))
    expect(card.className).not.toContain('fixed')
  })

  it('新 props 全不传时行为与旧版一致(无导出/删除/列设置/全屏钮)', () => {
    setup({ title: '列表' })
    expect(screen.queryByRole('button', { name: '导出' })).toBeNull()
    expect(screen.queryByRole('button', { name: '列设置' })).toBeNull()
    expect(screen.queryByRole('button', { name: '全屏' })).toBeNull()
  })
})
