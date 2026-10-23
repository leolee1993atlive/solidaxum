import { untrack } from 'svelte'
import { page } from '$app/state'
import { isApiClientNotifiedError } from '$lib/api/client.js'
import type { ListResult } from '$lib/api/types.js'
import { isCacheable } from '$lib/stores/tab-cache.js'
import { TABLE_DEFAULT_STATE } from '$lib/table-defaults.js'
import type {
  ColumnDef,
  PaginationState,
  RowSelectionState,
  SortingState,
  Table,
  Row,
  VisibilityState,
} from '@tanstack/table-core'
import { createTable, getCoreRowModel } from '@tanstack/table-core'

/** 从 TanStack rowSelection 映射取出选中的业务主键。 */
export function selectedIdsFrom(rowSelection: Record<string, boolean>): string[] {
  return Object.entries(rowSelection)
    .filter(([, v]) => v)
    .map(([k]) => k)
}

/**
 * 按 sortableIds 给列打上 enableSorting。
 * table-core 默认每列都可排序,而 CrudTableShell 的表头用 ColumnHeader 渲染
 * —— 它只在 getCanSort() 为真时画排序下拉。不关掉非白名单列,就会给每一列
 * 都长出一个「点了被 sortableIds.has() 过滤掉」的控件。
 * 列标识按 id 优先、accessorKey 兜底,与 table-core 的 getColumnId 一致。
 * 返回新数组,不就地改调用方传进来的列(可能是 $derived 的值)。
 */
export function withSortingFlags<TRow>(
  columns: ColumnDef<TRow>[],
  sortableIds: Set<string>
): ColumnDef<TRow>[] {
  return columns.map((c) => {
    const id = String(c.id ?? (c as { accessorKey?: string }).accessorKey ?? '')
    return { ...c, enableSorting: sortableIds.has(id) }
  })
}

export type FetchPageResult<TRow> = {
  rows: TRow[]
  totalRowCount: number
  /** 后端总数触到计数上限:totalRowCount 是**下界**,真实总数 ≥ 它。 */
  totalCapped?: boolean
  /** 触顶时的表级估算行数(不精确、不认筛选条件),仅供展示量级。 */
  totalApprox?: number
}

/** `ListResult` → `FetchPageResult` 的直译。
 *
 *  各列表页原先都手写 `{ rows: resp.rows, totalRowCount: resp.total }`,
 *  于是计数口径两个字段在每一页都被悄悄丢掉。收到一处,加字段时不必再改 28 个页面。 */
export function fromListResult<TRow>(
  r: ListResult<TRow>
): FetchPageResult<TRow> {
  return {
    rows: r.rows,
    totalRowCount: r.total,
    totalCapped: r.totalCapped,
    totalApprox: r.totalApprox,
  }
}

export type FetchPageFn<TRow> = (params: {
  pageNo: number
  pageSize: number
  keyword?: string
  sort?: string
  order?: 'asc' | 'desc'
  /** 额外筛选字段(来自页面 QueryForm 等),由 getExtraParams 提供。 */
  extra?: Record<string, string | number | undefined | null>
}) => Promise<FetchPageResult<TRow>>

export type UseCrudTableOptions<TRow> = {
  /**
   * 列定义。表格在 `$derived` 里读 `options.columns`,所以传 **getter**
   * (`get columns() { return derivedCols }`)即可让列随状态变化(如双 Tab 的条件列);
   * 传普通数组则等同固定列,行为与以前一致。
   */
  columns: ColumnDef<TRow>[]
  sortableIds: Set<string>
  fetchPage: FetchPageFn<TRow>
  initialPageSize?: number
  /**
   * 可选:返回业务主键字符串。提供后才启用行选择,且用业务主键作为 row id,
   * 避免 manualPagination 下跨页选择状态串页。
   */
  getRowId?: (row: TRow) => string
  /**
   * 可选:返回额外筛选参数(如多字段 QueryForm 的值)。
   * 返回值的变化(浅比较)会触发重新拉取。用于 RuoYi 风格的精确字段筛选。
   */
  getExtraParams?: () => Record<string, string | number | undefined | null>
  /**
   * 可选:默认隐藏的列 id 集(D4 列设置承载的隐藏列)。用户可在
   * CrudTableShell 的列设置里重新打开。
   */
  hiddenColumnIds?: Set<string>
  /**
   * 可选:持久化页面级筛选(QueryForm)。提供后且页面 cacheable 时,filters 也会存/取。
   */
  cacheExtra?: {
    get: () => Record<string, string | number | undefined | null>
    set: (v: Record<string, string | number | undefined | null>) => void
  }
  /**
   * 可选:缓存命名空间后缀。同一页面存在多个表格实例时用于区分 sessionStorage 键,
   * 避免互相覆盖对方缓存的分页/排序状态。
   */
  cacheScope?: string
}

export type UseCrudTableReturn<TRow> = {
  table: Table<TRow>
  isLoading: boolean
  error: string | null
  globalFilter: string
  sorting: SortingState
  pagination: PaginationState
  /** 当前选中的业务主键 id 数组。 */
  get selectedIds(): string[]
  /** 清空当前行选择。 */
  clearSelection: () => void
  reload: () => void
  resetPagination: () => void
}

export function useCrudTable<TRow>(
  options: UseCrudTableOptions<TRow>
): UseCrudTableReturn<TRow> {
  const {
    sortableIds,
    fetchPage,
    // 默认 20/条,对齐旧版 vxe pager(reD7,风格平价计划 §B5)。
    initialPageSize = 20,
    getRowId,
    getExtraParams,
    cacheExtra,
    cacheScope,
  } = options

  // State
  let tableRows = $state<TRow[]>([])
  let totalRowCount = $state(0)
  let totalCapped = $state(false)
  let totalApprox = $state<number | undefined>(undefined)
  let isLoading = $state(false)
  let error = $state<string | null>(null)
  let globalFilter = $state('')
  let sorting = $state<SortingState>([])
  let pagination = $state<PaginationState>({
    pageIndex: 0,
    pageSize: initialPageSize,
  })
  let rowSelection = $state<RowSelectionState>({})
  // 列显隐受控:table 实例是 $derived 重建的,不受控的话列设置勾选
  // 会在每次翻页/刷新时被内部态重置回默认。
  // hiddenColumnIds → 初始即隐藏(默认可见的补 false 无意义,只声明隐藏集)。
  let columnVisibility = $state<VisibilityState>(
    Object.fromEntries(
      [...(options.hiddenColumnIds ?? [])].map((id) => [id, false])
    )
  )

  const cacheKey = (() => {
    const path = typeof page !== 'undefined' ? page.url.pathname : ''
    if (!path || !isCacheable(path)) return null
    return cacheScope ? `crud:${path}#${cacheScope}` : `crud:${path}`
  })()

  if (cacheKey && typeof sessionStorage !== 'undefined') {
    const raw = sessionStorage.getItem(cacheKey)
    if (raw) {
      try {
        const s = JSON.parse(raw) as {
          globalFilter?: string
          sorting?: SortingState
          pagination?: PaginationState
        }
        if (s.globalFilter != null) globalFilter = s.globalFilter
        if (s.sorting) sorting = s.sorting
        if (s.pagination) {
          pagination = {
            pageIndex: s.pagination.pageIndex ?? 0,
            pageSize: s.pagination.pageSize ?? initialPageSize,
          }
        }
      } catch {
        /* ignore */
      }
    }
    if (cacheExtra) {
      const extraRaw = sessionStorage.getItem(`crud:extra:${cacheKey.slice(5)}`)
      if (extraRaw) {
        try {
          const extra = JSON.parse(extraRaw) as Record<
            string,
            string | number | undefined | null
          >
          cacheExtra.set(extra)
        } catch {
          /* ignore */
        }
      }
    }
  }

  // Track reload trigger
  let reloadTrigger = $state(0)

  // Fetch data effect
  $effect(() => {
    const pageNo = pagination.pageIndex + 1
    const pageSize = pagination.pageSize
    const keyword = globalFilter.trim() || undefined
    const s0 = sorting[0]
    const sort =
      s0 && sortableIds.has(String(s0.id)) ? String(s0.id) : undefined
    const order = sort ? (s0.desc ? 'desc' : 'asc') : undefined
    // 额外筛选参数(多字段 QueryForm);读取以建立响应式依赖。
    const extra = getExtraParams?.()

    // Include reloadTrigger in dependencies
    void reloadTrigger

    untrack(() => {
      void doFetch({ pageNo, pageSize, keyword, sort, order, extra })
    })
  })

  // Persist table state for cacheable pages
  $effect(() => {
    if (!cacheKey || typeof sessionStorage === 'undefined') return
    const snapshot = JSON.stringify({ globalFilter, sorting, pagination })
    sessionStorage.setItem(cacheKey, snapshot)
    if (cacheExtra) {
      sessionStorage.setItem(
        `crud:extra:${cacheKey.slice(5)}`,
        JSON.stringify(cacheExtra.get())
      )
    }
  })

  async function doFetch(params: {
    pageNo: number
    pageSize: number
    keyword?: string
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    isLoading = true
    error = null
    try {
      const pageResult = await fetchPage(params)
      tableRows = pageResult.rows
      totalRowCount = pageResult.totalRowCount
      totalCapped = pageResult.totalCapped ?? false
      totalApprox = pageResult.totalApprox
    } catch (e) {
      if (isApiClientNotifiedError(e)) {
        error = null
      } else {
        error = e instanceof Error ? e.message : '加载失败'
      }
    } finally {
      isLoading = false
    }
  }

  const table = $derived(
    createTable({
      data: tableRows,
      // 读 options.columns(而非解构出的定值):调用方传 getter 时列才能随状态变化。
      // withSortingFlags 按 sortableIds 注入 enableSorting:不在白名单的列
      // getCanSort() 为假,ColumnHeader 退化为纯文本,避免装饰表头从 0 列扩到全部列。
      columns: withSortingFlags(options.columns, sortableIds),
      ...(getRowId ? { getRowId: (row: TRow) => getRowId(row) } : {}),
      state: { ...TABLE_DEFAULT_STATE, sorting, globalFilter, pagination, rowSelection, columnVisibility },
      onSortingChange: (updater) => {
        sorting = typeof updater === 'function' ? updater(sorting) : updater
      },
      onColumnVisibilityChange: (updater) => {
        columnVisibility =
          typeof updater === 'function' ? updater(columnVisibility) : updater
      },
      onGlobalFilterChange: (updater) => {
        globalFilter =
          typeof updater === 'function' ? updater(globalFilter) : updater
      },
      onPaginationChange: (updater) => {
        pagination =
          typeof updater === 'function' ? updater(pagination) : updater
      },
      onRowSelectionChange: (updater) => {
        rowSelection = typeof updater === 'function' ? updater(rowSelection) : updater
      },
      onStateChange: () => {},
      manualPagination: true,
      manualFiltering: true,
      manualSorting: true,
      rowCount: totalRowCount,
      // 计数口径经 meta 递给分页组件:触顶时 rowCount 只是下界,
      // 「共 N 条 / 共 N 页」必须显示成「N+」,否则一个被截断的数看着像精确值。
      meta: { totalCapped, totalApprox },
      getCoreRowModel: getCoreRowModel(),
      renderFallbackValue: null,
      enableRowSelection: Boolean(getRowId),
    })
  )

  function reload() {
    reloadTrigger += 1
  }

  function resetPagination() {
    pagination = { ...pagination, pageIndex: 0 }
  }

  return {
    get table() {
      return table
    },
    get isLoading() {
      return isLoading
    },
    get error() {
      return error
    },
    get globalFilter() {
      return globalFilter
    },
    set globalFilter(value: string) {
      globalFilter = value
    },
    get sorting() {
      return sorting
    },
    set sorting(value: SortingState) {
      sorting = value
    },
    get pagination() {
      return pagination
    },
    set pagination(value: PaginationState) {
      pagination = value
    },
    get selectedIds() {
      return selectedIdsFrom(rowSelection)
    },
    clearSelection: () => {
      rowSelection = {}
    },
    reload,
    resetPagination,
  }
}
