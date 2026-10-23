<script lang="ts" generics="TRow">
  import type { Snippet } from 'svelte'
  import { cn } from '$lib/utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import * as Skeleton from '$lib/components/ui/skeleton/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import {
    ChevronRight,
    ChevronDown,
    RefreshCw,
    Search,
    ListTree,
  } from '@lucide/svelte'
  import {
    createTable,
    getCoreRowModel,
    getExpandedRowModel,
    getFilteredRowModel,
  } from '@tanstack/table-core'
  import type { ColumnDef, Table as TanstackTable } from '@tanstack/table-core'
  import { TABLE_DEFAULT_STATE } from '$lib/table-defaults.js'

  type Props = {
    /** 已组装好的树数据(带 children)。用 buildTree 归一化后传入。 */
    data: TRow[]
    columns: ColumnDef<TRow>[]
    isLoading: boolean
    error: string | null
    /** children 字段名,默认 children。 */
    childrenKey?: string
    /** 首列(带缩进+chevron)的 columnId,通常是名称列。 */
    treeColumnId: string
    searchPlaceholder?: string
    emptyMessage?: string
    globalFilter?: string
    onSearchInput?: (value: string) => void
    onRefresh?: () => void
    showSearch?: boolean
    showRefresh?: boolean
    cellRenderer: Snippet<[{ row: TRow; columnId?: string }]>
    customCellIds?: Set<string>
    /** 表格左上小标题,与工具栏同行。 */
    title?: string
    /** 查询栏,独立成行置于顶部。 */
    queryForm?: Snippet
    /** 统一工具栏,右对齐。本组件不做批量操作,故无参数。 */
    toolbar?: Snippet
  }

  let {
    data,
    columns,
    isLoading,
    error,
    childrenKey = 'children',
    treeColumnId,
    searchPlaceholder = '搜索...',
    emptyMessage = '未找到数据。',
    globalFilter = $bindable(''),
    onSearchInput,
    onRefresh,
    showSearch = true,
    showRefresh = true,
    cellRenderer,
    customCellIds,
    title,
    queryForm,
    toolbar,
  }: Props = $props()

  let expanded = $state<Record<string, boolean> | true>(true)

  const table = $derived(
    createTable<TRow>({
      data,
      columns,
      // 必须铺满 TABLE_DEFAULT_STATE:createTable 的 getState() 直接返回
      // options.state,不合并 initialState;getHeaderGroups 会读
      // getState().columnPinning.left,缺失即抛 "reading 'left' of undefined"。
      state: { ...TABLE_DEFAULT_STATE, expanded, globalFilter },
      onExpandedChange: (updater) => {
        expanded = typeof updater === 'function' ? updater(expanded) : updater
      },
      onGlobalFilterChange: (updater) => {
        globalFilter =
          typeof updater === 'function' ? updater(globalFilter) : updater
      },
      getSubRows: (row) =>
        (row as Record<string, unknown>)[childrenKey] as TRow[] | undefined,
      getCoreRowModel: getCoreRowModel(),
      getExpandedRowModel: getExpandedRowModel(),
      getFilteredRowModel: getFilteredRowModel(),
      onStateChange: () => {},
      renderFallbackValue: null,
    }) as TanstackTable<TRow>
  )

  const columnCount = $derived(columns.length)
  function toggleAll() {
    table.toggleAllRowsExpanded(!table.getIsAllRowsExpanded())
  }

  function handleInput(event: Event) {
    const value = (event.target as HTMLInputElement).value
    globalFilter = value
    onSearchInput?.(value)
  }

  function isCustom(id: string): boolean {
    return id === 'actions' || (customCellIds?.has(id) ?? false)
  }
</script>

<!-- 描边与 content-card 钩子同 CrudTableShell —— 见该文件顶部注释。 -->
<div data-slot="content-card" class="bg-card rounded-lg border">
  {#if queryForm}
    <div data-testid="shell-query" class="p-3">
      {@render queryForm()}
    </div>
  {/if}

  <!-- 小标题左、工具栏右,同一行;与表格之间刻意不留分割线。
       本行无条件渲染(展开/折叠全部按钮恒在),与 CrudTableShell 的
       hasToolbarRow 条件渲染不对称 —— 属有意设计:树表总有折叠控件。 -->
  <div
    data-testid="shell-toolbar"
    class="flex flex-wrap items-center gap-2 px-3 pt-3"
  >
    {#if title}
      <h3 data-testid="shell-title" class="text-base font-medium">{title}</h3>
    {/if}
    {#if showSearch}
      <div class="relative max-w-md min-w-0 flex-1">
        <Search
          class="text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4"
        />
        <Input
          placeholder={searchPlaceholder}
          aria-label={searchPlaceholder}
          class="pl-9"
          type="search"
          value={globalFilter}
          oninput={handleInput}
        />
      </div>
    {/if}
    <div
      data-testid="shell-actions"
      class="ml-auto flex flex-wrap items-center gap-2"
    >
      {#if toolbar}{@render toolbar()}{/if}
      <Button type="button" variant="outline" size="sm" onclick={toggleAll}>
        <ListTree class="mr-2 h-4 w-4" />{table.getIsAllRowsExpanded()
          ? '折叠全部'
          : '展开全部'}
      </Button>
      {#if showRefresh && onRefresh}
        <Button
          type="button"
          variant="outline"
          size="icon"
          title="刷新列表"
          aria-label="刷新列表"
          onclick={() => onRefresh()}
        >
          <RefreshCw class="h-4 w-4" />
        </Button>
      {/if}
    </div>
  </div>

  {#if isLoading}
    <div class="m-3 overflow-hidden rounded-lg border">
      <Table.Root>
        <Table.Header>
          <Table.Row
            >{#each Array(columnCount) as _}<Table.Head
                ><Skeleton.Root class="h-4 w-16" /></Table.Head
              >{/each}</Table.Row
          >
        </Table.Header>
        <Table.Body>
          {#each Array(5) as _}<Table.Row
              >{#each Array(columnCount) as _}<Table.Cell
                  ><Skeleton.Root class="h-4 w-full" /></Table.Cell
                >{/each}</Table.Row
            >{/each}
        </Table.Body>
      </Table.Root>
    </div>
  {:else if error}
    <div class="flex h-64 items-center justify-center">
      <div class="text-destructive">{error}</div>
    </div>
  {:else}
    <div class="m-3 overflow-hidden rounded-lg border">
      <Table.Root>
        <Table.Header>
          {#each table.getHeaderGroups() as headerGroup}
            <Table.Row>
              {#each headerGroup.headers as header}
                <Table.Head
                  >{#if !header.isPlaceholder}{header.column.columnDef
                      .header}{/if}</Table.Head
                >
              {/each}
            </Table.Row>
          {/each}
        </Table.Header>
        <Table.Body>
          {#each table.getRowModel().rows as row (row.id)}
            <Table.Row>
              {#each row.getVisibleCells() as cell}
                <Table.Cell
                  class={cn(
                    cell.column.columnDef.meta?.cellClass,
                    // 操作列按钮容器是 flex,默认靠左;2K 宽屏下 actions 列被撑宽,
                    // 与居中的「操作」表头错位。强制其内部直接子级 flex 容器居中。
                    cell.column.id === 'actions' && '[&>div]:justify-center'
                  )}
                >
                  {#if cell.column.id === treeColumnId}
                    <div
                      class="flex items-center"
                      style="padding-left: {row.depth * 20}px"
                    >
                      {#if row.getCanExpand()}
                        <button
                          type="button"
                          class="mr-1 inline-flex size-5 items-center justify-center"
                          onclick={() => row.toggleExpanded()}
                          aria-label="{row.getIsExpanded()
                            ? '折叠'
                            : '展开'} {row.getValue(treeColumnId)}"
                          aria-expanded={row.getIsExpanded()}
                        >
                          {#if row.getIsExpanded()}<ChevronDown
                              class="size-4"
                            />{:else}<ChevronRight class="size-4" />{/if}
                        </button>
                      {:else}
                        <span class="mr-1 inline-block size-5"></span>
                      {/if}
                      {#if isCustom(cell.column.id)}
                        {@render cellRenderer({
                          row: row.original,
                          columnId: cell.column.id,
                        })}
                      {:else}
                        <span>{cell.getValue()}</span>
                      {/if}
                    </div>
                  {:else if isCustom(cell.column.id)}
                    {@render cellRenderer({
                      row: row.original,
                      columnId: cell.column.id,
                    })}
                  {:else}
                    {cell.getValue()}
                  {/if}
                </Table.Cell>
              {/each}
            </Table.Row>
          {:else}
            <Table.Row
              ><Table.Cell colspan={columnCount} class="h-24 text-center"
                >{emptyMessage}</Table.Cell
              ></Table.Row
            >
          {/each}
        </Table.Body>
      </Table.Root>
    </div>
  {/if}
</div>
