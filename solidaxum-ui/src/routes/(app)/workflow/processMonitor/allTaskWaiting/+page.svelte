<script lang="ts">
  import { goto } from '$app/navigation'
  import type { ListResult } from '$lib/api/types.js'
  import { buildFormUrl } from '$lib/api/workflow/flow-router.js'
  import type { FlowHisTaskVo } from '$lib/api/workflow/mytask.js'
  import { taskApi } from '$lib/api/workflow/task.js'
  import type { FlowTaskVo } from '$lib/api/workflow/task.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import MessageType from '$lib/components/process/MessageType.svelte'
  import ProcessMeddle from '$lib/components/process/ProcessMeddle.svelte'
  import UserPicker from '$lib/components/process/UserPicker.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { Eye } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 待办/已办双 Tab(形态照搬 processInstance 页) ----
  // 'wait' 待办任务 / 'finish' 已办任务。label 逐字对齐 allTaskWaiting.vue:63-64。
  type Tab = 'wait' | 'finish'
  let activeTab = $state<Tab>('wait')

  function switchTab(tab: Tab) {
    if (activeTab === tab) return
    activeTab = tab
    // 切 Tab 必须重置分页 + 清选择(否则从第 3 页切过去请求不存在的页码)。
    crudTable.clearSelection()
    crudTable.resetPagination()
    crudTable.reload()
  }

  // ---- 筛选面 ----
  // Vue allTaskWaiting.vue:11-21 筛选:申请人(多选) · 任务名称 nodeName · 流程定义名称 flowName。
  // 不做前端多做的「流程编码」筛选框(Vue 筛选区无此项;注意「流程定义编码」列保留)。
  let filters = $state<{
    createByIds?: string
    nodeName?: string
    flowName?: string
  }>({})

  const queryFields = [
    { key: 'createByIds', label: '申请人', type: 'userSelect' as const },
    { key: 'nodeName', label: '任务名称', placeholder: '请输入任务名称' },
    {
      key: 'flowName',
      label: '流程定义名称',
      placeholder: '请输入流程定义名称',
    },
  ]

  // ---- 列 ----
  // 两 Tab VO 不同(待办 FlowTaskVo / 已办 FlowHisTaskVo),表格用 FlowTaskVo 泛型
  // (两 VO 都带 [k:string]:unknown 索引签名,已办独有字段 flowTaskStatus/approverName 经索引签名可达)。
  // 「任务状态」列只在已办 Tab 出现,对齐 Vue v-if="tab === 'finish'"(allTaskWaiting.vue:125)。
  // useCrudTable 在 $derived 里读 options.columns,所以下面把 columns 以 getter 传进去即可随 Tab 变。
  const baseColumns: ColumnDef<FlowTaskVo>[] = [
    // 业务编码/流程分类/版本号/办理人(待办用 assigneeNames)均对齐 Vue。
    {
      accessorKey: 'businessCode',
      header: '业务编码',
      accessorFn: (r) => r.businessCode ?? '-',
    },
    {
      accessorKey: 'businessTitle',
      header: '业务标题',
      accessorFn: (r) => r.businessTitle ?? '-',
    },
    {
      accessorKey: 'flowName',
      header: '流程定义名称',
      accessorFn: (r) => r.flowName ?? '-',
    },
    {
      accessorKey: 'flowCode',
      header: '流程定义编码',
      accessorFn: (r) => r.flowCode ?? '-',
    },
    {
      accessorKey: 'categoryName',
      header: '流程分类',
      accessorFn: (r) => r.categoryName ?? '-',
    },
    {
      accessorKey: 'version',
      header: '版本号',
      accessorFn: (r) => (r.version ? `v${r.version}.0` : '-'),
    },
    {
      accessorKey: 'nodeName',
      header: '任务名称',
      accessorFn: (r) => r.nodeName ?? '-',
    },
    {
      accessorKey: 'createByName',
      header: '申请人',
      accessorFn: (r) => r.createByName ?? '-',
    },
    // 办理人:待办 assigneeNames / 已办 approverName —— 优先 approverName(已办),回退 assigneeNames。
    {
      accessorKey: 'assigneeNames',
      header: '办理人',
      accessorFn: (r) =>
        (r as FlowHisTaskVo).approverName ?? r.assigneeNames ?? '-',
    },
    { accessorKey: 'flowStatus', header: '流程状态', enableSorting: false },
  ]

  // 任务状态列 —— 仅已办 Tab(FlowHisTaskVo 独有 flowTaskStatus)。
  const taskStatusColumn: ColumnDef<FlowTaskVo> = {
    accessorKey: 'flowTaskStatus',
    header: '任务状态',
    enableSorting: false,
    accessorFn: (r) => (r as FlowHisTaskVo).flowTaskStatus ?? '-',
  }

  const tailColumns: ColumnDef<FlowTaskVo>[] = [
    {
      accessorKey: 'createTime',
      header: '创建时间',
      accessorFn: (r) => r.createTime ?? '-',
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const columns = $derived<ColumnDef<FlowTaskVo>[]>([
    ...baseColumns,
    ...(activeTab === 'finish' ? [taskStatusColumn] : []),
    ...tailColumns,
  ])

  const customCellIds = new Set(['flowStatus', 'flowTaskStatus'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    // 已办走「全部已办」pageByAllTaskFinish(非「我的已办」pageByTaskFinish)。
    const resp =
      activeTab === 'wait'
        ? await taskApi.pageByAllTaskWait(
            { pageNum: params.pageNo, pageSize: params.pageSize },
            params.extra ?? {}
          )
        : await taskApi.pageByAllTaskFinish(
            { pageNum: params.pageNo, pageSize: params.pageSize },
            params.extra ?? {}
          )
    return fromListResult(resp as unknown as ListResult<FlowTaskVo>)
  }

  const crudTable = useCrudTable<FlowTaskVo>({
    // getter:列随 Tab 变(任务状态列只在已办出现)。
    get columns() {
      return columns
    },
    sortableIds: new Set(),
    fetchPage,
    // 🔴 必须提供 —— useCrudTable 的 enableRowSelection 由它决定(Boolean(getRowId))。
    // 不传则勾选框渲染出来但永远勾不上,两个批量按钮恒 disabled。
    getRowId: (row) => String(row.id),
    getExtraParams: () => ({
      createByIds: filters.createByIds,
      nodeName: filters.nodeName,
      flowName: filters.flowName,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      createByIds: model.createByIds,
      nodeName: model.nodeName,
      flowName: model.flowName,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 查看(跳业务表单,用 buildFormUrl) ----
  function viewUrl(r: FlowTaskVo): string | null {
    return buildFormUrl({
      formPath: r.formPath as string | null | undefined,
      businessId: r.businessId,
      type: 'view',
      from: '/workflow/processMonitor/allTaskWaiting',
    })
  }
  function handleView(r: FlowTaskVo) {
    const url = viewUrl(r)
    if (url) goto(url)
    else toast.error('该任务未配置业务表单路径')
  }

  // ---- 修改办理人(仅待办 Tab) ----
  // 流程对齐 Vue allTaskWaiting.vue:290-295:选人 → 确认弹窗 → updateAssignee。
  let assigneeOpen = $state(false)
  let assigneeConfirmOpen = $state(false)
  let assigneeUserIds = $state<string[]>([])
  let assigning = $state(false)

  function openAssignee() {
    if (crudTable.selectedIds.length === 0) return
    assigneeUserIds = []
    assigneeOpen = true
  }

  function confirmAssigneePick() {
    if (assigneeUserIds.length === 0) {
      toast.error('请选择一个用户')
      return
    }
    assigneeOpen = false
    assigneeConfirmOpen = true
  }

  async function handleAssignee() {
    assigning = true
    try {
      await taskApi.updateAssignee(assigneeUserIds[0], crudTable.selectedIds)
      toast.success(`已修改 ${crudTable.selectedIds.length} 条任务的办理人`)
      assigneeConfirmOpen = false
      crudTable.clearSelection()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      assigning = false
    }
  }

  // ---- 催办(仅待办 Tab) + MessageType ----
  let urgeOpen = $state(false)
  let urging = $state(false)

  function openUrge() {
    if (crudTable.selectedIds.length === 0) return
    urgeOpen = true
  }

  async function handleUrge(payload: {
    messageType: string[]
    message: string
  }) {
    // ⚠️ messageType 后端不消费(见 MessageType.svelte 顶部注释);消息内容进 message。
    // urgeMany 是前端循环逐条发,返回 {ok,fail};部分失败如实报告。
    urging = true
    try {
      // urgeMany 自己吞掉逐条失败(且已 silent,不逐条弹 toast),不会抛 —— 故无 catch。
      const { ok, fail } = await taskApi.urgeMany(
        crudTable.selectedIds,
        payload.message
      )
      if (fail === 0) toast.success(`催办成功 ${ok} 条`)
      else toast.error(`催办完成:成功 ${ok} 条 / 失败 ${fail} 条`)
      urgeOpen = false
      crudTable.clearSelection()
      crudTable.reload()
    } finally {
      urging = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage={activeTab === 'wait'
      ? '未找到待办任务。'
      : '未找到已办任务。'}
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    selectable
    title="任务列表"
    {customCellIds}
  >
    {#snippet queryForm()}
      <!-- 双 Tab -->
      <div class="mb-3 flex gap-1 border-b">
        {#each [{ v: 'wait', label: '待办任务' }, { v: 'finish', label: '已办任务' }] as t (t.v)}
          <button
            type="button"
            class="border-b-2 px-4 py-2 text-sm transition-colors {activeTab ===
            t.v
              ? 'border-primary text-primary-emphasis font-medium'
              : 'text-muted-foreground hover:text-foreground border-transparent'}"
            onclick={() => switchTab(t.v as Tab)}
          >
            {t.label}
          </button>
        {/each}
      </div>
      <QueryForm
        fields={queryFields}
        onSearch={handleSearch}
        onReset={handleReset}
      />
    {/snippet}
    {#snippet toolbar({ selectedCount })}
      <!-- 批量按钮仅待办 Tab 出现(已办不能改办理人也不能催办) -->
      {#if activeTab === 'wait'}
        <HasPermi perm="workflow:task:edit">
          <Button
            variant="outline"
            size="sm"
            disabled={selectedCount === 0}
            onclick={openAssignee}
          >
            修改
          </Button>
        </HasPermi>
        <!-- 后端 urge_task(task_handler.rs:319)与改派同权限,守卫不能只加一半 -->
        <HasPermi perm="workflow:task:edit">
          <Button
            variant="outline"
            size="sm"
            disabled={selectedCount === 0}
            onclick={openUrge}
          >
            催办
          </Button>
        </HasPermi>
      {/if}
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as FlowTaskVo}
      {#if columnId === 'flowStatus'}
        <DictTag dictType="wf_business_status" value={r.flowStatus} />
      {:else if columnId === 'flowTaskStatus'}
        <!-- 待办行无 flowTaskStatus(显示 '-'),已办行显示任务状态 tag -->
        {@const ts = (r as FlowHisTaskVo).flowTaskStatus}
        {#if ts}
          <DictTag dictType="wf_task_status" value={ts} />
        {:else}
          -
        {/if}
      {:else}
        <div class="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            title={viewUrl(r) ? '查看' : '该任务未配置业务表单路径'}
            disabled={!viewUrl(r)}
            onclick={() => handleView(r)}
          >
            <Eye class="mr-1 h-4 w-4" />查看
          </Button>
          <!-- 流程干预(ProcessMeddle 自带改派/转办/加签/终止四项,仅待办有意义) -->
          {#if activeTab === 'wait'}
            <ProcessMeddle taskId={r.id} onDone={() => crudTable.reload()} />
          {/if}
        </div>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<!-- 选人(单选) → 再确认 -->
<Dialog.Root bind:open={assigneeOpen}>
  <Dialog.Content class="sm:max-w-[520px]">
    <Dialog.Header>
      <Dialog.Title>选择办理人</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-2 px-2 pb-2">
      <Label>新办理人(单选)</Label>
      <UserPicker
        value={assigneeUserIds}
        multiple={false}
        onChange={(next) => (assigneeUserIds = next)}
      />
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (assigneeOpen = false)}
        >取消</Button
      >
      <Button onclick={confirmAssigneePick}>确定</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<ConfirmDialog
  open={assigneeConfirmOpen}
  onOpenChange={(v) => (assigneeConfirmOpen = v)}
  title="确认修改办理人"
  desc={`是否确认将选中的 ${crudTable.selectedIds.length} 条任务的办理人修改为新用户?`}
  confirmText="确认"
  isLoading={assigning}
  handleConfirm={handleAssignee}
/>

<!-- 催办消息弹窗 -->
<MessageType
  bind:open={urgeOpen}
  title="催办"
  loading={urging}
  onConfirm={handleUrge}
  onCancel={() => (urgeOpen = false)}
/>
