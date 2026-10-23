<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'

  import { jobLogApi } from '$lib/api/monitor/joblog.js'
  import type { SysJobLog } from '$lib/api/monitor/joblog.js'
  import { jobLogStatusLabel } from '$lib/api/monitor/joblog.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
        import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Eraser } from '@lucide/svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{ job_name?: string }>({})

  const queryFields = [
    { key: 'job_name', label: '任务名称', placeholder: '请输入任务名称' },
  ]

  // ---- columns: accessorKey 一律 snake_case, 与实体字段对齐。 ----
  const columns: ColumnDef<SysJobLog>[] = [
    { accessorKey: 'job_log_id', header: '日志编号' },
    { accessorKey: 'job_name', header: '任务名称' },
    { accessorKey: 'job_group', header: '任务组名' },
    { accessorKey: 'invoke_target', header: '调用目标' },
    {
      accessorKey: 'job_message',
      header: '日志信息',
      accessorFn: (row) => row.job_message ?? '-',
    },
    {
      accessorKey: 'status',
      header: '状态',
      accessorFn: (row) => jobLogStatusLabel(row.status),
    },
    {
      accessorKey: 'start_time',
      header: '开始时间',
      accessorFn: (row) => row.start_time ?? '-',
    },
    {
      accessorKey: 'stop_time',
      header: '停止时间',
      accessorFn: (row) => row.stop_time ?? '-',
    },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await jobLogApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<SysJobLog>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({ job_name: filters.job_name }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = { job_name: model.job_name }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- clean all confirm ----
  let cleanOpen = $state(false)
  let cleaning = $state(false)

  function openClean() {
    cleanOpen = true
  }

  async function handleClean() {
    cleaning = true
    try {
      await jobLogApi.clean()
      toast.success('清空成功')
      cleanOpen = false
      crudTable.resetPagination()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      cleaning = false
    }
  }
</script>


<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    title="日志列表"
    emptyMessage="未找到任务日志数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}>
    {#snippet toolbar()}
      <HasPermi perm="monitor:job:remove">
        <Button variant="destructive" size="sm" onclick={openClean}>
          <Eraser class="mr-1 h-4 w-4" />清空
        </Button>
      </HasPermi>
    {/snippet}
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer()}{/snippet}
  </CrudTableShell>
</Main>

<ConfirmDialog
  open={cleanOpen}
  onOpenChange={(v) => (cleanOpen = v)}
  title="确认清空"
  desc="确定要清空所有任务日志吗?清空后不可恢复。"
  confirmText="清空"
  destructive
  isLoading={cleaning}
  handleConfirm={handleClean} />
