<script lang="ts" generics="TData">
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import {
    ChevronLeft,
    ChevronRight,
    ChevronsLeft,
    ChevronsRight,
  } from '@lucide/svelte'
  import type { Table } from '@tanstack/table-core'

  let { table }: { table: Table<TData> } = $props()

  const selectedRowsCount = $derived(
    table.getFilteredSelectedRowModel().rows.length
  )
  const totalRowsCount = $derived(
    table.options.manualPagination
      ? table.getRowCount()
      : table.getFilteredRowModel().rows.length
  )
  const pageSize = $derived(table.getState().pagination.pageSize)
  const pageIndex = $derived(table.getState().pagination.pageIndex)
  const pageCount = $derived(table.getPageCount())

  // 后端默认走上限计数(1 万):没触顶时 rowCount 就是精确值,
  // 触顶时它只是**下界** —— 那就必须显示成「N+」,否则一个被截断的数
  // 看上去和精确总数一模一样。
  const capped = $derived(table.options.meta?.totalCapped === true)
  const approx = $derived(table.options.meta?.totalApprox)
  const plus = $derived(capped ? '+' : '')
  /** 触顶且拿到表级估算时,补一句量级 —— 「10000+ 行(全表约 20 万)」。 */
  const approxHint = $derived(
    capped && typeof approx === 'number'
      ? `(全表约 ${approx.toLocaleString('zh-CN')})`
      : ''
  )

  // 条/页选项对齐旧版 vxe pager(D7):10-200 五档。
  const PAGE_SIZES = [10, 20, 50, 100, 200]

  /** 数字页码序列(antd mini pager 观感):短表全列;长表首末页 + 当前页 ±1,间隙折叠为省略号。 */
  function pageSequence(current: number, count: number): (number | '…')[] {
    if (count <= 7) {
      return Array.from({ length: count }, (_, i) => i + 1)
    }
    const pages = new Set<number>([1, count, current + 1])
    if (current > 0) pages.add(current)
    if (current + 2 <= count) pages.add(current + 2)
    const sorted = [...pages].filter((p) => p >= 1 && p <= count).sort((a, b) => a - b)
    const result: (number | '…')[] = []
    let prev = 0
    for (const p of sorted) {
      if (p - prev > 1) result.push('…')
      result.push(p)
      prev = p
    }
    return result
  }

  const sequence = $derived(pageSequence(pageIndex, pageCount))
</script>

<div class="flex flex-wrap items-center justify-between gap-2 px-2">
  <div class="text-muted-foreground flex-1 text-sm">
    共 {totalRowsCount}{plus} 条{approxHint}
    {#if selectedRowsCount > 0}
      <span class="text-foreground font-medium">·已选 {selectedRowsCount}</span>
    {/if}
  </div>
  <div class="flex items-center gap-4">
    <div class="flex items-center gap-2">
      <p class="text-sm font-medium">条/页</p>
      <Select.Root
        type="single"
        value={String(pageSize)}
        onValueChange={(value) => table.setPageSize(Number(value))}
      >
        <Select.Trigger class="h-8 w-17.5">
          {pageSize}
        </Select.Trigger>
        <Select.Content side="top">
          {#each PAGE_SIZES as size}
            <Select.Item value={String(size)} label={String(size)}>
              {size}
            </Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    </div>
    <div class="flex items-center gap-1">
      {#each sequence as p, i (typeof p === 'number' ? `p${p}` : `e${i}`)}
        {#if p === '…'}
          <span class="text-muted-foreground px-1 text-sm">…</span>
        {:else}
          <Button
            variant={p - 1 === pageIndex ? 'default' : 'outline'}
            class="size-8 p-0 text-xs"
            aria-current={p - 1 === pageIndex ? 'page' : undefined}
            onclick={() => table.setPageIndex(p - 1)}
          >
            {p}
          </Button>
        {/if}
      {/each}
    </div>
    <div class="flex items-center space-x-2">
      <Button
        variant="outline"
        class="hidden size-8 p-0 lg:flex"
        onclick={() => table.setPageIndex(0)}
        disabled={!table.getCanPreviousPage()}
      >
        <span class="sr-only">转到第一页</span>
        <ChevronsLeft class="size-4" />
      </Button>
      <Button
        variant="outline"
        class="size-8 p-0"
        onclick={() => table.previousPage()}
        disabled={!table.getCanPreviousPage()}
      >
        <span class="sr-only">转到上一页</span>
        <ChevronLeft class="size-4" />
      </Button>
      <Button
        variant="outline"
        class="size-8 p-0"
        onclick={() => table.nextPage()}
        disabled={!table.getCanNextPage()}
      >
        <span class="sr-only">转到下一页</span>
        <ChevronRight class="size-4" />
      </Button>
      <Button
        variant="outline"
        class="hidden size-8 p-0 lg:flex"
        onclick={() => table.setPageIndex(table.getPageCount() - 1)}
        disabled={!table.getCanNextPage()}
      >
        <span class="sr-only">转到最后一页</span>
        <ChevronsRight class="size-4" />
      </Button>
    </div>
  </div>
</div>
