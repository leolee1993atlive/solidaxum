<script lang="ts">
  import {
    buildViewSpel,
    isSubmittableSpel,
  } from '$lib/api/workflow/spel-expression.js'
  import { spelApi } from '$lib/api/workflow/spel.js'
  import type { SpelVo } from '$lib/api/workflow/spel.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { Plus, Pencil, Trash2 } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 筛选 ----
  let filters = $state<{
    componentName?: string
    methodName?: string
    status?: string
  }>({})

  const queryFields = [
    { key: 'componentName', label: '组件名称', placeholder: '请输入组件名称' },
    { key: 'methodName', label: '方法名称', placeholder: '请输入方法名称' },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_normal_disable',
    },
  ]

  // ---- 列 ----
  // 不做「编号」「创建时间」列(Vue 无);label 改回 Vue 的「SPEL表达式」。
  // 含「状态」列(Vue el-tag 正常/停用双态)。
  const columns: ColumnDef<SpelVo>[] = [
    {
      accessorKey: 'componentName',
      header: '组件名称',
      accessorFn: (row) => row.componentName ?? '-',
    },
    {
      accessorKey: 'methodName',
      header: '方法名称',
      accessorFn: (row) => row.methodName ?? '-',
    },
    {
      accessorKey: 'methodParams',
      header: '方法参数',
      accessorFn: (row) => row.methodParams ?? '-',
    },
    {
      accessorKey: 'viewSpel',
      header: 'SPEL表达式',
      accessorFn: (row) => row.viewSpel ?? '-',
    },
    { accessorKey: 'status', header: '状态', enableSorting: false },
    {
      accessorKey: 'remark',
      header: '备注',
      accessorFn: (row) => row.remark ?? '-',
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['status'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await spelApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<SpelVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    getRowId: (row) => String(row.id),
    getExtraParams: () => ({
      componentName: filters.componentName,
      methodName: filters.methodName,
      status: filters.status,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      componentName: model.componentName,
      methodName: model.methodName,
      status: model.status,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 单行删除 ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<SpelVo | null>(null)

  function openDelete(row: SpelVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await spelApi.delete([String(deleteTarget.id)])
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

  // ---- 批量删除 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  function openBatchDelete() {
    batchDeleteOpen = true
  }

  async function handleBatchDelete() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchDeleting = true
    try {
      await spelApi.delete(ids)
      toast.success('批量删除成功')
      batchDeleteOpen = false
      crudTable.clearSelection()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchDeleting = false
    }
  }

  // ---- 新增/编辑弹窗 ----
  type SpelForm = {
    componentName?: string
    methodName?: string
    methodParams?: string
    remark?: string
    status?: string
  }

  let dialogOpen = $state(false)
  let isEdit = $state(false)
  let editingId = $state<string | null>(null)
  let saving = $state(false)
  let form = $state<SpelForm>({})

  // 实时预览:三个字段任一变化即重算,对齐 Vue 的 watch(index.vue:384)。
  const viewSpel = $derived(buildViewSpel(form))

  function resetForm() {
    form = {
      componentName: '',
      methodName: '',
      methodParams: '',
      remark: '',
      status: '0',
    }
  }

  function openCreate() {
    isEdit = false
    editingId = null
    resetForm()
    dialogOpen = true
  }

  function openEdit(row: SpelVo) {
    isEdit = true
    editingId = String(row.id)
    form = {
      componentName: row.componentName ?? '',
      methodName: row.methodName ?? '',
      methodParams: row.methodParams ?? '',
      remark: row.remark ?? '',
      status: row.status ?? '0',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    // viewSpel 由预览派生、不让用户直接编辑(Vue 亦然:该输入框 readonly)。
    // 提交时用预览值而非 form.viewSpel,否则编辑既有记录会把旧值原样回传。
    if (!isSubmittableSpel(viewSpel)) {
      toast.error('请先填写组件名称与方法名称')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        await spelApi.update({ ...form, viewSpel, id: editingId })
        toast.success('修改成功')
      } else {
        await spelApi.create({ ...form, viewSpel })
        toast.success('新增成功')
      }
      dialogOpen = false
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      saving = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage="未找到流程表达式数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    selectable
    title="表达式列表"
    {customCellIds}
  >
    {#snippet toolbar({ selectedCount })}
      <HasPermi perm="workflow:spel:add">
        <Button size="sm" onclick={openCreate}>
          <Plus class="mr-1 h-4 w-4" />新增
        </Button>
      </HasPermi>
      <HasPermi perm="workflow:spel:remove">
        <Button
          variant="destructive"
          size="sm"
          disabled={selectedCount === 0}
          onclick={openBatchDelete}
        >
          <Trash2 class="mr-1 h-4 w-4" />删除
        </Button>
      </HasPermi>
    {/snippet}
    {#snippet queryForm()}
      <QueryForm
        fields={queryFields}
        onSearch={handleSearch}
        onReset={handleReset}
      />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as SpelVo}
      {#if columnId === 'status'}
        <DictTag dictType="sys_normal_disable" value={r.status} />
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="workflow:spel:edit">
            <Button
              variant="ghost"
              size="icon"
              title="修改"
              aria-label="修改"
              onclick={() => openEdit(r)}
            >
              <Pencil class="size-4" />
            </Button>
          </HasPermi>
          <HasPermi perm="workflow:spel:remove">
            <Button
              variant="ghost"
              size="icon"
              class="text-destructive hover:text-destructive"
              title="删除"
              aria-label="删除"
              onclick={() => openDelete(r)}
            >
              <Trash2 class="size-4" />
            </Button>
          </HasPermi>
        </div>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '修改表达式' : '添加表达式'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="componentName">
      组件名称
      <span class="text-muted-foreground text-xs"
        >（注册到 Spring 容器中的组件名，如 spelRuleComponent）</span
      >
    </Label>
    <Input
      id="componentName"
      bind:value={form.componentName}
      placeholder="请输入组件名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="methodName">
      方法名称
      <span class="text-muted-foreground text-xs"
        >（组件中的方法名称，如 selectDeptLeaderById）</span
      >
    </Label>
    <Input
      id="methodName"
      bind:value={form.methodName}
      placeholder="请输入方法名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="methodParams">
      方法参数
      <span class="text-muted-foreground text-xs"
        >（多个参数用英文逗号分隔）</span
      >
    </Label>
    <Input
      id="methodParams"
      bind:value={form.methodParams}
      placeholder="如：deptId,userId"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label>SPEL表达式（自动生成）</Label>
    <Input value={viewSpel} readonly class="font-mono" />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="spelStatus">状态</Label>
    <!-- 表单面保持原生 select,与 oss-config / user 等页一致;渲染面已用 DictTag 字典化 -->
    <select
      id="spelStatus"
      class="border-input flex h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm"
      bind:value={form.status}
    >
      <option value="0">正常</option>
      <option value="1">停用</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="spelRemark">备注</Label>
    <Textarea
      id="spelRemark"
      bind:value={form.remark}
      rows={3}
      placeholder="请输入备注"
    />
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除流程表达式「${deleteTarget?.componentName ?? ''}.${deleteTarget?.methodName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 条流程表达式吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>
