<script lang="ts" generics="TRow">
  import type { Snippet } from 'svelte'
  import { cn } from '$lib/utils.js'
  import { selectedIdsFrom } from '$lib/composables/use-crud-table.svelte.js'
  import {
    ColumnHeader,
    Pagination as DataTablePagination,
  } from '$lib/components/data-table/index.js'
  import type { SegmentedOption } from '$lib/components/crud/index.js'
  import { SegmentedControl } from '$lib/components/crud/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import * as Popover from '$lib/components/ui/popover/index.js'
  import * as Skeleton from '$lib/components/ui/skeleton/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import {
    Columns3,
    Download,
    Maximize,
    Minimize,
    RefreshCw,
    Search,
    Trash2,
  } from '@lucide/svelte'
  import type { Table as TanstackTable } from '@tanstack/table-core'

  type Props = {
    table: TanstackTable<TRow>
    isLoading: boolean
    error: string | null
    columnCount: number
    searchPlaceholder?: string
    emptyMessage?: string
    globalFilter: string
    onSearchInput?: (value: string) => void
    onRefresh?: () => void
    /** 是否显示关键词搜索框(默认 true)。用多字段 QueryForm 的页面可关掉。 */
    showSearch?: boolean
    /** 是否显示刷新按钮(默认 true)。 */
    showRefresh?: boolean
    /** 是否启用行多选(默认 false)。现有页面不传时保持原行为。 */
    selectable?: boolean
    /** 分段状态过滤(如「全部/待提交/返回重报」),渲染在标题右侧。 */
    segmented?: {
      options: SegmentedOption[]
      value: string
      onchange: (value: string) => void
    }
    /** 渲染导出按钮;权限由页面侧判定后才传入。 */
    onExport?: () => void
    /** 渲染批量删除按钮(红色实心);未勾选行时禁用。 */
    onBatchDelete?: () => void
    /** 渲染列设置 Popover(读写 columnVisibility,承载隐藏列)。 */
    columnSettings?: boolean
    /** 渲染全屏切换按钮(容器 fixed 全屏)。 */
    fullscreen?: boolean
    cellRenderer: Snippet<[{ row: TRow; columnId?: string }]>
    customCellIds?: Set<string>
    /** 表格左上小标题,与工具栏同行。 */
    title?: string
    /** 占标题的位置、取代标题的自定义内容(如已发布/未发布分段页签);与 title 同时传时只渲染 heading。 */
    heading?: Snippet
    /** 查询栏,独立成行置于顶部。 */
    queryForm?: Snippet
    /** 统一工具栏(页面级操作 + 批量操作),右对齐。接收当前选中行数。 */
    toolbar?: Snippet<[{ selectedCount: number }]>
    /** 可选:点击数据行(整行)回调,用于主从联动选中。 */
    onRowClick?: (row: TRow) => void
    /** 可选:返回数据行附加 class(如选中行高亮)。 */
    rowClass?: (row: TRow) => string
    /**
     * 视口适配:卡片撑满父容器剩余高度,表格区内部滚动(粘性表头),
     * 分页钉在卡片底部——整页不再滚动。要求父链高度锁定
     * (路由级 pageFit 布局,见 (app)/+layout 的 FIT_HEIGHT_PATHS)。
     */
    fit?: boolean
  }

  let {
    table,
    isLoading,
    error,
    columnCount,
    searchPlaceholder = '搜索...',
    emptyMessage = '未找到数据。',
    globalFilter = $bindable(),
    onSearchInput,
    onRefresh,
    showSearch = true,
    showRefresh = true,
    selectable = false,
    segmented,
    onExport,
    onBatchDelete,
    columnSettings = false,
    fullscreen = false,
    cellRenderer,
    customCellIds,
    title,
    heading,
    queryForm,
    toolbar,
    onRowClick,
    rowClass,
    fit = false,
  }: Props = $props()

  const columns = $derived(table.getAllColumns())
  const effectiveColumnCount = $derived(selectable ? columnCount + 1 : columnCount)
  const selectedCount = $derived(selectedIdsFrom(table.getState().rowSelection).length)

  // 列设置可选列:排除操作列与显式 enableHiding:false 的列
  // (选择列不是 column def,天然不在 getAllColumns 里)。
  const settableColumns = $derived(
    columns.filter(
      (c) => c.id !== 'actions' && c.getCanHide() && String(c.columnDef.header ?? '') !== ''
    )
  )

  const hasToolbarRow = $derived(
    Boolean(
      title ||
        heading ||
        toolbar ||
        showSearch ||
        (showRefresh && onRefresh) ||
        segmented ||
        onExport ||
        onBatchDelete ||
        columnSettings ||
        fullscreen
    )
  )

  let isFullscreen = $state(false)

  function handleInput(event: Event) {
    const value = (event.target as HTMLInputElement).value
    globalFilter = value
    onSearchInput?.(value)
  }
</script>

<!-- 单张白卡承载「查询栏 + 工具栏 + 表格 + 分页」,对齐青绿:内容区是一整块
     面板,不是几块浮在渐变上的碎片。
     border 是给老皮肤的兜底,不能删:default / de-swiss-design 亮色下 --card
     与 --background 同为 oklch(1 0 0),没有描边白卡坐在白底上会彻底看不见。
     青绿自己不要这圈外框(层次由白底与青绿渐变的反差给出),在 teal.css 里
     按 data-slot='content-card' 抹平 —— 见该文件对应规则。 -->
<div
  data-slot="content-card"
  class={cn(
    'bg-card rounded-lg border',
    fit && 'flex min-h-0 flex-1 flex-col',
    isFullscreen &&
      'fixed inset-0 z-50 flex flex-col overflow-auto rounded-none bg-background p-2'
  )}
>
  {#if queryForm}
    <div data-testid="shell-query" class="p-3">
      {@render queryForm()}
    </div>
  {/if}

  {#if hasToolbarRow}
    <!-- 小标题左、工具栏右,同一行。与表格之间刻意不留分割线 ——
         表格自带的圆角描边已经提供了边界。 -->
    <div
      data-testid="shell-toolbar"
      class="flex flex-wrap items-center gap-2 px-3 pt-3"
    >
      {#if heading}
        <div data-testid="shell-heading">{@render heading()}</div>
      {:else if title}
        <h3 data-testid="shell-title" class="text-base font-medium">{title}</h3>
      {/if}
      {#if segmented}
        <SegmentedControl
          options={segmented.options}
          value={segmented.value}
          onchange={segmented.onchange}
        />
      {/if}
      {#if showSearch}
        <div class="relative max-w-md min-w-0 flex-1">
          <Search
            class="text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4"
          />
          <Input
            placeholder={searchPlaceholder}
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
        {#if toolbar}{@render toolbar({ selectedCount })}{/if}
        {#if onExport}
          <Button type="button" variant="outline" onclick={onExport}>
            <Download class="h-4 w-4" />导出
          </Button>
        {/if}
        {#if onBatchDelete}
          <Button
            type="button"
            variant="destructive"
            disabled={selectable && selectedCount === 0}
            onclick={onBatchDelete}
          >
            <Trash2 class="h-4 w-4" />删除
          </Button>
        {/if}
        {#if columnSettings}
          <Popover.Root>
            <Popover.Trigger
              title="列设置"
              aria-label="列设置"
              class="inline-flex size-9 items-center justify-center rounded-md border text-sm font-medium transition-colors hover:bg-accent hover:text-accent-foreground"
            >
              <Columns3 class="h-4 w-4" />
            </Popover.Trigger>
            <Popover.Content align="end" class="w-48 p-2">
              <div class="text-muted-foreground mb-1 px-2 text-xs">列显示</div>
              <div class="flex flex-col gap-1">
                {#each settableColumns as column (column.id)}
                  <label
                    class="hover:bg-accent flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm"
                  >
                    <Checkbox
                      checked={column.getIsVisible()}
                      onCheckedChange={() => column.toggleVisibility()}
                    />
                    {String(column.columnDef.header ?? column.id)}
                  </label>
                {/each}
                {#if settableColumns.length === 0}
                  <div class="text-muted-foreground px-2 py-1 text-xs">
                    无可配置列
                  </div>
                {/if}
              </div>
            </Popover.Content>
          </Popover.Root>
        {/if}
        {#if fullscreen}
          <Button
            type="button"
            variant="outline"
            size="icon"
            title={isFullscreen ? '退出全屏' : '全屏'}
            aria-label={isFullscreen ? '退出全屏' : '全屏'}
            onclick={() => (isFullscreen = !isFullscreen)}
          >
            {#if isFullscreen}
              <Minimize class="h-4 w-4" />
            {:else}
              <Maximize class="h-4 w-4" />
            {/if}
          </Button>
        {/if}
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
  {/if}

  {#if isLoading}
    <div class="m-3 overflow-hidden rounded-lg border">
      <Table.Root>
        <Table.Header>
          <Table.Row>
            {#each Array(effectiveColumnCount) as _}
              <Table.Head><Skeleton.Root class="h-4 w-16" /></Table.Head>
            {/each}
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each Array(5) as _}
            <Table.Row>
              {#each Array(effectiveColumnCount) as _}
                <Table.Cell><Skeleton.Root class="h-4 w-full" /></Table.Cell>
              {/each}
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    </div>
  {:else if error}
    <div class="flex h-64 items-center justify-center">
      <div class="text-destructive">{error}</div>
    </div>
  {:else}
    <!-- 表格自带一层圆角描边,与外层白卡的边框拉开距离(m-3),
         overflow-hidden 让淡青表头跟着圆角裁切。fit 时改为纵向滚动区:
         flex-1 撑满卡片剩余高、粘性表头随皮肤(青绿淡青 / 其他皮肤补白底)。 -->
    <div
      class={cn(
        'm-3 overflow-hidden rounded-lg border',
        fit &&
          'mb-0 min-h-0 flex-1 overflow-auto [&_thead]:sticky [&_thead]:top-0 [&_thead]:z-10 [&_thead]:bg-card'
      )}
    >
      <Table.Root>
        <Table.Header>
          {#each table.getHeaderGroups() as headerGroup}
            <Table.Row>
              {#if selectable}
                <Table.Head>
                  <Checkbox
                    aria-label="全选"
                    checked={table.getIsAllRowsSelected()}
                    indeterminate={table.getIsSomeRowsSelected()}
                    onCheckedChange={(v) => table.toggleAllRowsSelected(v === true)}
                  />
                </Table.Head>
              {/if}
              {#each headerGroup.headers as header}
                <Table.Head>
                  {#if !header.isPlaceholder}
                    <!-- ColumnHeader 在 getCanSort() 为假时退化为纯文本,
                         所以非排序列的观感与改造前完全一致。 -->
                    <ColumnHeader
                      column={header.column}
                      title={String(header.column.columnDef.header ?? '')}
                    />
                  {/if}
                </Table.Head>
              {/each}
            </Table.Row>
          {/each}
        </Table.Header>
        <Table.Body>
          {#each table.getRowModel().rows as row}
            <Table.Row
              class={rowClass?.(row.original)}
              onclick={onRowClick ? () => onRowClick(row.original) : undefined}
            >
              {#if selectable}
                <!--
                  勾选框自身的点击不能冒泡到 <Table.Row onclick={onRowClick}>：
                  selectable 与 onRowClick(主从联动)同时启用时,点一下勾选框
                  会连带触发行选中。
                -->
                <Table.Cell onclick={(e: Event) => e.stopPropagation()}>
                  <Checkbox
                    aria-label="选择行"
                    checked={row.getIsSelected()}
                    onCheckedChange={() => row.toggleSelected()}
                  />
                </Table.Cell>
              {/if}
              {#each row.getVisibleCells() as cell}
                <Table.Cell
                  class={cn(
                    cell.column.columnDef.meta?.cellClass,
                    // 操作列按钮容器是 flex,默认靠左;2K 宽屏下 actions 列被撑宽,
                    // 与居中的「操作」表头错位。强制其内部直接子级 flex 容器居中。
                    cell.column.id === 'actions' && '[&>div]:justify-center'
                  )}
                >
                  {#if cell.column.id === 'actions' || (customCellIds && customCellIds.has(cell.column.id))}
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
            <Table.Row>
              <Table.Cell colspan={effectiveColumnCount} class="h-24 text-center">
                {emptyMessage}
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    </div>

    <!-- 表格已自带边框,分页不再需要 border-t 再画一条 -->
    <div class="px-3 pb-3">
      <DataTablePagination {table} />
    </div>
  {/if}
</div>
