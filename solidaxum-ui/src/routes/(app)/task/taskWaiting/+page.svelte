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
  import { CheckCircle2 } from '@lucide/svelte'

  // 筛选面四项(申请人多选 + 任务名称 + 流程定义名称/编码),对齐 Vue。
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

  // 列序与标题逐字对齐 taskWaiting.vue(已实测)。业务编码绑 businessCode
  // (原 applyCode 不存在,恒显示 '-')、标题「任务名称」非「当前节点」。
  const columns: ColumnDef<FlowTaskVo>[] = [
    { accessorKey: 'businessCode', header: '业务编码', accessorFn: (r) => r.businessCode ?? '-' },
    { accessorKey: 'businessTitle', header: '业务标题', accessorFn: (r) => r.businessTitle ?? '-' },
    { accessorKey: 'flowName', header: '流程定义名称', accessorFn: (r) => r.flowName ?? '-' },
    { accessorKey: 'flowCode', header: '流程定义编码', accessorFn: (r) => r.flowCode ?? '-' },
    { accessorKey: 'categoryName', header: '流程分类', accessorFn: (r) => r.categoryName ?? '-' },
    { accessorKey: 'nodeName', header: '任务名称', accessorFn: (r) => r.nodeName ?? '-' },
    { accessorKey: 'createByName', header: '申请人', accessorFn: (r) => r.createByName ?? '-' },
    { accessorKey: 'assigneeNames', header: '办理人', accessorFn: (r) => r.assigneeNames ?? '-' },
    // 流程状态: 走字典 tag(wf_business_status)。
    { accessorKey: 'flowStatus', header: '流程状态', enableSorting: false },
    { accessorKey: 'createTime', header: '创建时间', accessorFn: (r) => r.createTime ?? '-' },
    // 操作列(办理入口)。
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['flowStatus'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const r = await myTaskApi.taskWaiting({ pageNum: params.pageNo, pageSize: params.pageSize }, params.extra ?? {})
    return fromListResult(r)
  }

  const crudTable = useCrudTable<FlowTaskVo>({
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
    title="我的待办"
    emptyMessage="暂无待办任务。"
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
          type: 'approval',
          taskId: r.id,
          from: '/task/taskWaiting',
        })}
        <Button
          variant="ghost"
          size="sm"
          title={url ? '办理' : '该任务未配置业务表单路径,无法办理'}
          disabled={!url}
          onclick={() => url && goto(url)}>
          <CheckCircle2 class="mr-1 h-4 w-4" />办理
        </Button>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>
