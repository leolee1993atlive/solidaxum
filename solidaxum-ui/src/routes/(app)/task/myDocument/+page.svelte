<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'
  import { goto } from '$app/navigation'
  import { Eye, Pencil, Trash2, Undo2 } from '@lucide/svelte'

  import { myTaskApi } from '$lib/api/workflow/mytask.js'
  import type { FlowTaskVo } from '$lib/api/workflow/mytask.js'
  import { instanceApi } from '$lib/api/workflow/instance.js'
  import { categoryApi } from '$lib/api/workflow/category.js'
  import type { CategoryTreeVo } from '$lib/api/workflow/category.js'
  import { buildFormUrl } from '$lib/api/workflow/flow-router.js'
  import TreePanel from '$lib/components/TreePanel.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import { DICT_TAG_CLASS } from '$lib/components/dict-tag.js'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'

  // ---- 左侧流程分类树(接法同 processDefinition / processInstance 页) ----
  let categoryTree = $state<CategoryTreeVo[]>([])
  /** 选中的分类 id;undefined = 全部(对齐 Vue handleNodeClick 里 id==='0' 归空) */
  let selectedCategory = $state<string | undefined>(undefined)

  async function loadCategoryTree() {
    categoryTree = (await categoryApi.categoryTree()).data ?? []
  }

  function handleCategoryClick(id: string | undefined) {
    // 根节点(id '0')等同「全部」
    selectedCategory = id === '0' ? undefined : id
    crudTable.reload()
  }

  // ---- 筛选字段 flowCode ----
  // Vue myDocument.vue:29 筛选只有「流程定义编码 flowCode」。pageByCurrent 走 InstancePageQuery,
  // 后端 flow_code 与 flow_name 都收,但对齐 Vue 按 flowCode。
  let filters = $state<{ flowCode?: string }>({})
  const queryFields = [
    { key: 'flowCode', label: '流程定义编码', placeholder: '请输入流程定义编码' },
  ]

  // ---- 列(对齐 Vue myDocument.vue:67-87) ----
  // 流程分类/版本号/流程状态/激活挂起状态列均对齐 Vue;「当前节点 nodeName」与
  // 「业务编号」两列 Vue 无,不做(「业务编号」原前端多做且绑 applyCode 恒 '-')。
  const columns: ColumnDef<FlowTaskVo>[] = [
    { accessorKey: 'flowName', header: '流程定义名称', accessorFn: (r) => r.flowName ?? '-' },
    { accessorKey: 'flowCode', header: '流程定义编码', accessorFn: (r) => r.flowCode ?? '-' },
    { accessorKey: 'categoryName', header: '流程分类', accessorFn: (r) => r.categoryName ?? '-' },
    { accessorKey: 'version', header: '版本号', accessorFn: (r) => (r.version ? `v${r.version}.0` : '-') },
    // 激活/挂起(无字典,用 DICT_TAG_CLASS 配色 + Badge,与 processInstance 页一致)
    { accessorKey: 'isSuspended', header: '状态', enableSorting: false },
    { accessorKey: 'flowStatus', header: '流程状态', enableSorting: false },
    { accessorKey: 'createTime', header: '启动时间', accessorFn: (r) => r.createTime ?? '-' },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['isSuspended', 'flowStatus'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const r = await myTaskApi.myDocument(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(r)
  }

  const crudTable = useCrudTable<FlowTaskVo>({
    columns,
    sortableIds: new Set(),
    fetchPage,
    // id 口径见 instance.ts —— 实例 id 是后端 i64,本仓按 string 承载
    getRowId: (row) => String(row.id),
    getExtraParams: () => ({
      category: selectedCategory,
      flowCode: filters.flowCode,
    }),
  })

  $effect(() => {
    void loadCategoryTree()
  })

  function handleSearch(model: Record<string, string>) {
    filters = { flowCode: model.flowCode }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 行操作 + 状态守卫 ----
  // 守卫条件逐字对齐 Vue myDocument.vue:93-131:
  //   编辑/删除: flowStatus ∈ {draft, cancel, back}
  //   查看:      无条件
  //   撤销:      flowStatus === 'waiting'
  const EDITABLE = new Set(['draft', 'cancel', 'back'])

  function isEditable(r: FlowTaskVo) {
    return !!r.flowStatus && EDITABLE.has(r.flowStatus)
  }
  function isCancelable(r: FlowTaskVo) {
    return r.flowStatus === 'waiting'
  }

  function viewUrl(r: FlowTaskVo): string | null {
    return buildFormUrl({
      formPath: r.formPath as string | null | undefined,
      businessId: r.businessId,
      type: 'view',
      from: '/task/myDocument',
    })
  }
  function editUrl(r: FlowTaskVo): string | null {
    return buildFormUrl({
      formPath: r.formPath as string | null | undefined,
      businessId: r.businessId,
      type: 'update',
      from: '/task/myDocument',
    })
  }

  // 编辑
  function handleEdit(r: FlowTaskVo) {
    const url = editUrl(r)
    if (url) goto(url)
    else toast.error('该实例未配置业务表单路径')
  }
  // 查看
  function handleView(r: FlowTaskVo) {
    const url = viewUrl(r)
    if (url) goto(url)
    else toast.error('该实例未配置业务表单路径')
  }

  // 删除(Vue handleDelete:modal.confirm → deleteByInstanceIds)
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<FlowTaskVo | null>(null)

  function openDelete(r: FlowTaskVo) {
    deleteTarget = r
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await instanceApi.deleteByInstanceIds([String(deleteTarget.id)])
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

  // 撤销(Vue handleCancelProcessApply:modal.confirm → cancelProcessApply(businessId, message))
  // message 对齐 Vue: '申请人撤销流程！'。
  let cancelOpen = $state(false)
  let canceling = $state(false)
  let cancelTarget = $state<FlowTaskVo | null>(null)

  function openCancel(r: FlowTaskVo) {
    cancelTarget = r
    cancelOpen = true
  }

  async function handleCancel() {
    if (!cancelTarget) return
    canceling = true
    try {
      await instanceApi.cancelProcessApply(
        String(cancelTarget.businessId),
        '申请人撤销流程！'
      )
      toast.success('撤销成功')
      cancelOpen = false
      cancelTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      canceling = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex gap-4">
    <TreePanel
      title="流程分类"
      placeholder="请输入流程分类名"
      nodes={categoryTree}
      selectedId={selectedCategory}
      onNodeClick={handleCategoryClick} />
    <div class="min-w-0 flex-1">
      <CrudTableShell
        table={crudTable.table}
        isLoading={crudTable.isLoading}
        error={crudTable.error}
        columnCount={columns.length}
        title="我发起的"
        emptyMessage="暂无发起的流程。"
        globalFilter=""
        showSearch={false}
        showRefresh={false}
        customCellIds={customCellIds}>
        {#snippet queryForm()}
          <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
        {/snippet}
        {#snippet cellRenderer({ row, columnId })}
          {@const r = row as FlowTaskVo}
          {#if columnId === 'isSuspended'}
            <Badge
              variant="outline"
              class={r.isSuspended ? DICT_TAG_CLASS.danger : DICT_TAG_CLASS.success}>
              {r.isSuspended ? '挂起' : '激活'}
            </Badge>
          {:else if columnId === 'flowStatus'}
            <DictTag dictType="wf_business_status" value={r.flowStatus} />
          {:else}
            <div class="flex flex-wrap items-center gap-1">
              <!-- 编辑: draft/cancel/back -->
              {#if isEditable(r)}
                <Button
                  variant="ghost"
                  size="sm"
                  title={editUrl(r) ? '编辑' : '该实例未配置业务表单路径'}
                  disabled={!editUrl(r)}
                  onclick={() => handleEdit(r)}>
                  <Pencil class="mr-1 h-4 w-4" />编辑
                </Button>
              {/if}
              <!-- 删除: draft/cancel/back(Vue 同条件,非无条件) -->
              {#if isEditable(r)}
                <HasPermi perm="workflow:instance:remove">
                  <Button
                    variant="ghost"
                    size="sm"
                    class="text-destructive hover:text-destructive"
                    title="删除"
                    onclick={() => openDelete(r)}>
                    <Trash2 class="mr-1 h-4 w-4" />删除
                  </Button>
                </HasPermi>
              {/if}
              <!-- 查看: 无条件 -->
              <Button
                variant="ghost"
                size="sm"
                title={viewUrl(r) ? '查看' : '该实例未配置业务表单路径'}
                disabled={!viewUrl(r)}
                onclick={() => handleView(r)}>
                <Eye class="mr-1 h-4 w-4" />查看
              </Button>
              <!-- 撤销: waiting。后端 cancel_process_apply(instance_handler.rs:296)要 workflow:instance:cancel -->
              {#if isCancelable(r)}
                <HasPermi perm="workflow:instance:cancel">
                  <Button
                    variant="ghost"
                    size="sm"
                    title="撤销"
                    onclick={() => openCancel(r)}>
                    <Undo2 class="mr-1 h-4 w-4" />撤销
                  </Button>
                </HasPermi>
              {/if}
            </div>
          {/if}
        {/snippet}
      </CrudTableShell>
    </div>
  </div>
</Main>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除流程「${deleteTarget?.flowName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete} />

<ConfirmDialog
  open={cancelOpen}
  onOpenChange={(v) => (cancelOpen = v)}
  title="确认撤销"
  desc={`确定要撤销当前单据「${cancelTarget?.flowName ?? ''}」吗?`}
  confirmText="撤销"
  isLoading={canceling}
  handleConfirm={handleCancel} />
