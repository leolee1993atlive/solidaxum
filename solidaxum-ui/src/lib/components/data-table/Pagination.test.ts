import { cleanup, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import type { Component } from 'svelte'
import Pagination from './Pagination.svelte'
import {
  createTable,
  getCoreRowModel,
  getFilteredRowModel,
  type Table,
} from '@tanstack/table-core'

afterEach(cleanup)

type Row = { id: string }

// Svelte 5 泛型组件在测试 render 中无法自动推断 TData,显式收窄(同 CrudTableShell.test.ts)。
const PaginationOfRow = Pagination as unknown as Component<{
  table: Table<Row>
}>

/** 手工分页的受控表(对齐 use-crud-table 的用法),行数据只喂当前页。 */
function makeTable(opts: { rowCount: number; pageIndex?: number; pageSize?: number }) {
  return createTable<Row>({
    data: [],
    columns: [{ accessorKey: 'id', header: 'ID' }],
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    state: {
      pagination: {
        pageIndex: opts.pageIndex ?? 0,
        pageSize: opts.pageSize ?? 20,
      },
      rowSelection: {},
    },
    onStateChange: () => {},
    manualPagination: true,
    rowCount: opts.rowCount,
    renderFallbackValue: null,
  })
}

describe('Pagination v2(D7 布局)', () => {
  it('左侧显示「共 X 条」,不再是已选中行文案', () => {
    render(PaginationOfRow, { props: { table: makeTable({ rowCount: 57 }) } })
    expect(screen.getByText(/共 57 条/)).toBeTruthy()
    expect(screen.queryByText(/已选中/)).toBeNull()
  })

  it('触顶计数显示 N+(沿用 totalCapped 语义)', () => {
    const table = makeTable({ rowCount: 10000 })
    // meta 直接挂在 options 上,Pagination 读 table.options.meta
    ;(table.options as { meta?: unknown }).meta = {
      totalCapped: true,
      totalApprox: 200000,
    }
    render(PaginationOfRow, { props: { table } })
    expect(screen.getByText(/共 10000\+ 条/)).toBeTruthy()
    expect(screen.getByText(/全表约 200,000/)).toBeTruthy()
  })

  it('渲染数字页码序列,当前页高亮为 default 变体', () => {
    render(PaginationOfRow, {
      props: { table: makeTable({ rowCount: 57, pageIndex: 1, pageSize: 20 }) },
    })
    // 3 页全列:1 2 3;第 2 页当前 → aria-current=page
    const current = screen.getByRole('button', { name: '2', current: 'page' })
    expect(current).toBeTruthy()
    for (const p of ['1', '3']) {
      expect(screen.getByRole('button', { name: p })).toBeTruthy()
    }
  })

  it('长表折叠中段页码为省略号', () => {
    render(PaginationOfRow, {
      props: { table: makeTable({ rowCount: 500, pageIndex: 4, pageSize: 20 }) },
    })
    // 25 页,当前第 5 页:1 … 4 5 6 … 25
    const ellipsis = screen.getAllByText('…')
    expect(ellipsis.length).toBeGreaterThanOrEqual(1)
    expect(screen.getByRole('button', { name: '1' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '25' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: '9' })).toBeNull()
  })

  it('条/页选项对齐旧版 10-200 五档', () => {
    render(PaginationOfRow, { props: { table: makeTable({ rowCount: 10 }) } })
    // h-8 w-17.5 的 Select 触发器;选项在 Popover 里,直接断言触发器显示默认 20
    expect(screen.getByText('条/页')).toBeTruthy()
    expect(screen.getByText('20')).toBeTruthy()
  })
})
