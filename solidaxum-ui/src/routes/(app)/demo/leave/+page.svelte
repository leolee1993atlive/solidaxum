<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'
  import { goto } from '$app/navigation'

  import { leaveApi } from '$lib/api/workflow/leave.js'
  import type { LeaveVo } from '$lib/api/workflow/leave.js'
  import { leaveTypeLabel, canEditLeave, canCancelLeave } from '$lib/api/workflow/leave-options.js'
  import { instanceApi } from '$lib/api/workflow/instance.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Plus, Pencil, Eye, Trash2, Undo2, Download } from '@lucide/svelte'
  import DictTag from '$lib/components/DictTag.svelte'

  let filters = $state<{ startLeaveDays?: string; endLeaveDays?: string }>({})

  // 后端 LeavePageQuery 已有 start_leave_days / end_leave_days
  // (leave_handler.rs:33-34),前端接上即生效。
  // Vue 列表没有 leaveType 文本筛选,故不提供该筛选。
  const queryFields = [
    { key: 'startLeaveDays', label: '请假天数(起)', placeholder: '最少天数' },
    { key: 'endLeaveDays', label: '请假天数(至)', placeholder: '最多天数' },
  ]

  // 不做「编号 id」「申请编号 applyCode」「创建时间 createTime」三列(Vue 无);
  // 补「请假原因 remark」列(Vue index.vue:85 有)。
  // 列对齐 Vue index.vue:66-90。
  const columns: ColumnDef<LeaveVo>[] = [
    { accessorKey: 'leaveType', header: '请假类型', accessorFn: (row) => leaveTypeLabel(row.leaveType) },
    { accessorKey: 'startDate', header: '开始时间', accessorFn: (row) => row.startDate ?? '-' },
    { accessorKey: 'endDate', header: '结束时间', accessorFn: (row) => row.endDate ?? '-' },
    { accessorKey: 'leaveDays', header: '请假天数' },
    { accessorKey: 'remark', header: '请假原因', accessorFn: (row) => row.remark ?? '-' },
    { accessorKey: 'status', header: '流程状态', enableSorting: false },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await leaveApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<LeaveVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({
      startLeaveDays: filters.startLeaveDays,
      endLeaveDays: filters.endLeaveDays,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      startLeaveDays: model.startLeaveDays,
      endLeaveDays: model.endLeaveDays,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  function gotoEdit(mode: 'add' | 'update' | 'view', row?: LeaveVo) {
    const base = row ? `/workflow/leaveEdit/${row.id}` : '/workflow/leaveEdit'
    goto(`${base}?mode=${mode}`)
  }

  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<LeaveVo | null>(null)

  function openDelete(row: LeaveVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await leaveApi.delete(String(deleteTarget.id))
      toast.success('删除成功')
      deleteOpen = false
      deleteTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      deleting = false
    }
  }

  // ---- 撤销 ----
  let cancelOpen = $state(false)
  let cancelling = $state(false)
  let cancelTarget = $state<LeaveVo | null>(null)

  function openCancel(row: LeaveVo) {
    cancelTarget = row
    cancelOpen = true
  }

  async function handleCancel() {
    if (!cancelTarget) return
    cancelling = true
    try {
      // 传的是 businessId(请假单 id),不是 instanceId —— 对齐 Vue
      // handleCancelProcessApply(index.vue:290-300)。封装已是 PUT。
      await instanceApi.cancelProcessApply(String(cancelTarget.id), '申请人撤销流程！')
      toast.success('撤销成功')
      cancelOpen = false
      cancelTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      cancelling = false
    }
  }

  // ---- 导出 ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await leaveApi.export({ ...filters })
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛裸 Error,全局没人 toast —— 不自己报就是「点了没反应」。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }
</script>

<Main class="flex min-h-0 flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage="未找到请假申请数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    fit
    title="申请列表"
    customCellIds={new Set(['status'])}>
    {#snippet toolbar()}
      <HasPermi perm="workflow:leave:export">
        <Button variant="outline" size="sm" onclick={handleExport} disabled={exporting}>
          <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
        </Button>
      </HasPermi>
      <HasPermi perm="workflow:leave:add">
        <Button size="sm" onclick={() => gotoEdit('add')}><Plus class="mr-1 h-4 w-4" />新增</Button>
      </HasPermi>
    {/snippet}
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as LeaveVo}
      {#if columnId === 'status'}
        <DictTag dictType="wf_business_status" value={r.status} />
      {:else}
        <div class="flex items-center gap-1">
          {#if canEditLeave(r.status)}
            <HasPermi perm="workflow:leave:edit">
              <Button variant="ghost" size="icon" title="修改" aria-label="修改"
                onclick={() => gotoEdit('update', r)}>
                <Pencil class="size-4" />
              </Button>
            </HasPermi>
            <HasPermi perm="workflow:leave:remove">
              <Button variant="ghost" size="icon" class="text-destructive hover:text-destructive"
                title="删除" aria-label="删除" onclick={() => openDelete(r)}>
                <Trash2 class="size-4" />
              </Button>
            </HasPermi>
          {/if}
          <!-- 查看:恒显示,对齐 Vue index.vue:124-126 -->
          <Button variant="ghost" size="icon" title="查看" aria-label="查看"
            onclick={() => gotoEdit('view', r)}>
            <Eye class="size-4" />
          </Button>
          {#if canCancelLeave(r.status)}
            <!-- 守卫用 workflow:instance:cancel(后端 instance_handler.rs:294-297),
                 不是 workflow:leave:* —— 撤销打的是流程实例的端点。 -->
            <HasPermi perm="workflow:instance:cancel">
              <Button variant="ghost" size="icon" title="撤销" aria-label="撤销"
                onclick={() => openCancel(r)}>
                <Undo2 class="size-4" />
              </Button>
            </HasPermi>
          {/if}
        </div>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除请假申请「${deleteTarget?.applyCode ?? String(deleteTarget?.id ?? '')}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete} />

<ConfirmDialog
  open={cancelOpen}
  onOpenChange={(v) => (cancelOpen = v)}
  title="确认撤销"
  desc="是否确认撤销当前单据?撤销后流程终止,单据回到可编辑状态。"
  confirmText="撤销"
  isLoading={cancelling}
  handleConfirm={handleCancel} />
