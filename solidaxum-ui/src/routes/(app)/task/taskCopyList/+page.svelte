<script lang="ts">
  import type { ColumnDef } from '@tanstack/table-core'

  import { goto } from '$app/navigation'
  import { myTaskApi } from '$lib/api/workflow/mytask.js'
  import type { FlowTaskVo } from '$lib/api/workflow/mytask.js'
  import { buildFormUrl } from '$lib/api/workflow/flow-router.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Eye } from '@lucide/svelte'

  // 抄送页 Vue 无「申请人」(taskCopyList.vue:11-19 只有三框),不加 userSelect。
  let filters = $state<{ nodeName?: string; flowName?: string; flowCode?: string }>({})

  const queryFields = [
    { key: 'nodeName', label: '任务名称', placeholder: '请输入任务名称' },
    { key: 'flowName', label: '流程定义名称', placeholder: '请输入流程定义名称' },
    { key: 'flowCode', label: '流程定义编码', placeholder: '请输入流程定义编码' },
  ]

  // 列序与标题逐字对齐 taskCopyList.vue(已实测):业务编号绑 businessCode,
  // 不做多余的「创建时间」列(Vue 抄送页无)。
  // 抄送页无「申请人」「办理人」两列(Vue 如此)。
  const columns: ColumnDef<FlowTaskVo>[] = [
    { accessorKey: 'businessCode', header: '业务编码', accessorFn: (r) => r.businessCode ?? '-' },
    { accessorKey: 'businessTitle', header: '业务标题', accessorFn: (r) => r.businessTitle ?? '-' },
    { accessorKey: 'flowName', header: '流程定义名称', accessorFn: (r) => r.flowName ?? '-' },
    { accessorKey: 'flowCode', header: '流程定义编码', accessorFn: (r) => r.flowCode ?? '-' },
    { accessorKey: 'categoryName', header: '流程分类', accessorFn: (r) => r.categoryName ?? '-' },
    { accessorKey: 'version', header: '版本号', accessorFn: (r) => r.version ?? '-' },
    { accessorKey: 'nodeName', header: '任务名称', accessorFn: (r) => r.nodeName ?? '-' },
    { accessorKey: 'flowStatus', header: '流程状态', enableSorting: false },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['flowStatus'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const r = await myTaskApi.taskCopyList({ pageNum: params.pageNo, pageSize: params.pageSize }, params.extra ?? {})
    return fromListResult(r)
  }

  const crudTable = useCrudTable<FlowTaskVo>({
    columns,
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({
      nodeName: filters.nodeName,
      flowName: filters.flowName,
      flowCode: filters.flowCode,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
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
    title="我的抄送"
    {customCellIds}
    emptyMessage="暂无抄送。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}>
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as FlowTaskVo}
      {#if columnId === 'flowStatus'}
        <DictTag dictType="wf_business_status" value={r.flowStatus} />
      {:else}
        {@const url = buildFormUrl({
          formPath: r.formPath,
          businessId: r.businessId,
          type: 'view',
          from: '/task/taskCopyList',
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
