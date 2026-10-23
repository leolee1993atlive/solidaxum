<script lang="ts">
  import type { ColumnDef } from '@tanstack/table-core'

  import { goto } from '$app/navigation'
  import { myTaskApi } from '$lib/api/workflow/mytask.js'
  import type { FlowHisTaskVo } from '$lib/api/workflow/mytask.js'
  import { buildFormUrl } from '$lib/api/workflow/flow-router.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Eye } from '@lucide/svelte'

  // 筛选面四项(与 taskWaiting 同形)。
  let filters = $state<{
    createByIds?: string
    nodeName?: string
    flowName?: string
    flowCode?: string
  }>({})

  const queryFields = [
    { key: 'createByIds', label: '申请人', type: 'userSelect' as const },
    { key: 'nodeName', label: '任务名称', placeholder: '请输入任务名称' },
    { key: 'flowName', label: '流程定义名称', placeholder: '请输入流程定义名称' },
    { key: 'flowCode', label: '流程定义编码', placeholder: '请输入流程定义编码' },
  ]

  // 列序与标题逐字对齐 taskFinish.vue(已实测):含任务状态列,业务编号绑
  // businessCode,标题「任务名称」。
  const columns: ColumnDef<FlowHisTaskVo>[] = [
    { accessorKey: 'businessCode', header: '业务编码', accessorFn: (r) => r.businessCode ?? '-' },
    { accessorKey: 'businessTitle', header: '业务标题', accessorFn: (r) => r.businessTitle ?? '-' },
    { accessorKey: 'flowName', header: '流程定义名称', accessorFn: (r) => r.flowName ?? '-' },
    { accessorKey: 'flowCode', header: '流程定义编码', accessorFn: (r) => r.flowCode ?? '-' },
    { accessorKey: 'categoryName', header: '流程分类', accessorFn: (r) => r.categoryName ?? '-' },
    { accessorKey: 'version', header: '版本号', accessorFn: (r) => r.version ?? '-' },
    { accessorKey: 'nodeName', header: '任务名称', accessorFn: (r) => r.nodeName ?? '-' },
    { accessorKey: 'createByName', header: '申请人', accessorFn: (r) => r.createByName ?? '-' },
    // 已办页办理人绑 approverName —— 不是 assigneeNames。
    { accessorKey: 'approverName', header: '办理人', accessorFn: (r) => r.approverName ?? '-' },
    // ⚠️ Vue 顺序:流程状态在前、任务状态在后,别写反。
    { accessorKey: 'flowStatus', header: '流程状态', enableSorting: false },
    // 任务状态,wf_task_status 字典(后端 flow_task_status 字段)。
    { accessorKey: 'flowTaskStatus', header: '任务状态', enableSorting: false },
    { accessorKey: 'createTime', header: '创建时间', accessorFn: (r) => r.createTime ?? '-' },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['flowStatus', 'flowTaskStatus'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const r = await myTaskApi.taskFinish({ pageNum: params.pageNo, pageSize: params.pageSize }, params.extra ?? {})
    return fromListResult(r)
  }

  const crudTable = useCrudTable<FlowHisTaskVo>({
    columns,
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({
      createByIds: filters.createByIds,
      nodeName: filters.nodeName,
      flowName: filters.flowName,
      flowCode: filters.flowCode,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      createByIds: model.createByIds,
      nodeName: model.nodeName,
      flowName: model.flowName,
      flowCode: model.flowCode,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    {customCellIds}
    title="我的已办"
    emptyMessage="暂无已办任务。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}>
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as FlowHisTaskVo}
      {#if columnId === 'flowTaskStatus'}
        <DictTag dictType="wf_task_status" value={r.flowTaskStatus} />
      {:else if columnId === 'flowStatus'}
        <DictTag dictType="wf_business_status" value={r.flowStatus} />
      {:else}
        {@const url = buildFormUrl({
          formPath: r.formPath,
          businessId: r.businessId,
          type: 'view',
          from: '/task/taskFinish',
        })}
        <Button
          variant="ghost"
          size="sm"
          title={url ? '查看' : '该任务未配置业务表单路径'}
          disabled={!url}
          onclick={() => url && goto(url)}>
          <Eye class="mr-1 h-4 w-4" />查看
        </Button>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>
