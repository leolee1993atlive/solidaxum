<script lang="ts">
  import { onMount } from 'svelte'
  import { postApi } from '$lib/api/system/post.js'
  import type {
    PostListVo,
    PostCreateBo,
    PostUpdateBo,
  } from '$lib/api/system/post.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import TreePanel from '$lib/components/TreePanel.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import type { TreeSelectNode } from '$lib/components/tree-select-types.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { Plus, Pencil, Trash2, Download } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{
    postCode?: string
    postCategory?: string
    postName?: string
    status?: string
    belongDeptId?: string
    deptId?: string
  }>({})

  // ---- 左侧部门树:点节点按 belongDeptId 过滤(后端会展开子树,见 post_handler.rs:33-42) ----
  let deptTree = $state<TreeSelectNode[]>([])

  onMount(async () => {
    try {
      const res = await postApi.deptTree()
      deptTree = res.data ?? []
    } catch {
      // apiClient 已 toast
    }
  })

  function handleDeptClick(id: string | undefined) {
    filters = { ...filters, belongDeptId: id }
    crudTable.resetPagination()
  }

  const queryFields = $derived([
    { key: 'postCode', label: '岗位编码', placeholder: '请输入岗位编码' },
    { key: 'postCategory', label: '类别编码', placeholder: '请输入类别编码' },
    { key: 'postName', label: '岗位名称', placeholder: '请输入岗位名称' },
    {
      key: 'deptId',
      label: '所属部门',
      type: 'treeSelect' as const,
      treeData: deptTree,
      placeholder: '请选择部门',
    },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_normal_disable',
    },
  ])

  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // 目标列对齐 Vue post/index.vue:135-152:岗位编号 v-if="false"(PO11 删)、
  // 类别编码(PO6 的列侧)、部门(PO4)、创建时间(PO5)。
  const columns: ColumnDef<PostListVo>[] = [
    { accessorKey: 'postCode', header: '岗位编码' },
    {
      accessorKey: 'postCategory',
      header: '类别编码',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { accessorKey: 'postName', header: '岗位名称' },
    {
      accessorKey: 'deptName',
      header: '部门',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { accessorKey: 'postSort', header: '排序' },
    { accessorKey: 'status', header: '状态', enableSorting: false },
    {
      accessorKey: 'createTime',
      header: '创建时间',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['status'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await postApi.list(
      {
        pageNum: params.pageNo,
        pageSize: params.pageSize,
        orderByColumn: params.sort,
        isAsc: params.order,
      },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<PostListVo>({
    columns,
    // 后端 post 排序白名单(post_service.rs:39-44):postId/postCode/postSort/createTime。
    // postName 不在白名单,原来那条是悬空的。
    sortableIds: new Set(['postCode', 'postSort', 'createTime']),
    fetchPage,
    // 🔴 selectable 必须配 getRowId(同 role 页)。
    getRowId: (row) => row.postId,
    getExtraParams: () => ({
      postCode: filters.postCode,
      postCategory: filters.postCategory,
      postName: filters.postName,
      status: filters.status,
      belongDeptId: filters.belongDeptId,
      deptId: filters.deptId,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      ...filters,
      postCode: model.postCode,
      postCategory: model.postCategory,
      postName: model.postName,
      status: model.status,
      deptId: model.deptId || undefined,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    // 重置只保留 belongDeptId(左侧树的选择不该被「重置」清掉)
    filters = { belongDeptId: filters.belongDeptId }
    crudTable.resetPagination()
  }

  // ---- PO9 批量删除 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  async function handleBatchDelete() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchDeleting = true
    try {
      await postApi.delete(ids.join(','))
      toast.success('批量删除成功')
      batchDeleteOpen = false
      // 🔴 reload 不动 rowSelection,已删的业务主键会留在选择集里(见 role 页同处注释)。
      crudTable.clearSelection()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchDeleting = false
    }
  }

  // ---- PO10 导出 ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      // 导出的筛选参数要带上 belongDeptId(左侧树的选择),后端 export_post 与 list 一样会展开子树。
      await postApi.export({
        postCode: filters.postCode,
        postName: filters.postName,
        postCategory: filters.postCategory,
        status: filters.status,
        belongDeptId: filters.belongDeptId,
        deptId: filters.deptId,
      })
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛裸 Error,不经 apiClient —— 不自己报就是「点了没反应」。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }

  // ---- create / edit dialog ----
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  const isEdit = $derived(editingId !== null)

  interface PostFormModel {
    postCode: string
    postName: string
    postCategory: string
    postSort: number
    deptId: string | null
    status: string
    remark: string
  }

  function emptyForm(): PostFormModel {
    return {
      postCode: '',
      postName: '',
      postCategory: '',
      postSort: 0,
      deptId: null,
      status: '0',
      remark: '',
    }
  }

  let form = $state<PostFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: PostListVo) {
    editingId = row.postId
    form = {
      postCode: row.postCode ?? '',
      postName: row.postName ?? '',
      postCategory: row.postCategory ?? '',
      postSort: row.postSort ?? 0,
      deptId: row.deptId ?? null,
      status: row.status ?? '0',
      remark: row.remark ?? '',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.postCode || !form.postName) {
      toast.error('岗位编码和岗位名称不能为空')
      return
    }
    if (form.deptId === null || form.deptId === undefined) {
      toast.error('归属部门不能为空')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: PostUpdateBo = {
          postId: editingId,
          postCode: form.postCode,
          postName: form.postName,
          postCategory: form.postCategory || undefined,
          postSort: form.postSort,
          deptId: form.deptId ?? undefined,
          status: form.status,
          remark: form.remark || undefined,
        }
        await postApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: PostCreateBo = {
          postCode: form.postCode,
          postName: form.postName,
          postCategory: form.postCategory || undefined,
          postSort: form.postSort,
          deptId: form.deptId ?? undefined,
          status: form.status,
          remark: form.remark || undefined,
        }
        await postApi.create(payload)
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

  // ---- delete confirm ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<PostListVo | null>(null)

  function openDelete(row: PostListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await postApi.delete(deleteTarget.postId)
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
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex gap-4">
    <TreePanel
      title="部门结构"
      placeholder="请输入部门名称"
      nodes={deptTree}
      selectedId={filters.belongDeptId}
      onNodeClick={handleDeptClick}
    />
    <div class="min-w-0 flex-1">
      <CrudTableShell
        table={crudTable.table}
        isLoading={crudTable.isLoading}
        error={crudTable.error}
        columnCount={columns.length}
        {customCellIds}
        selectable
        title="岗位列表"
        emptyMessage="未找到岗位数据。"
        globalFilter=""
        showSearch={false}
        showRefresh={false}
      >
        {#snippet toolbar({ selectedCount })}
          <HasPermi perm="system:post:export">
            <Button
              variant="outline"
              size="sm"
              onclick={handleExport}
              disabled={exporting}
            >
              <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
            </Button>
          </HasPermi>
          <HasPermi perm="system:post:add">
            <Button size="sm" onclick={openCreate}
              ><Plus class="mr-1 h-4 w-4" />新增</Button
            >
          </HasPermi>
          <HasPermi perm="system:post:remove">
            <Button
              variant="destructive"
              size="sm"
              disabled={selectedCount === 0}
              onclick={() => (batchDeleteOpen = true)}
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
          {@const r = row as PostListVo}
          {#if columnId === 'status'}
            <DictTag dictType="sys_normal_disable" value={r.status} />
          {:else}
            <div class="flex items-center gap-1">
              <HasPermi perm="system:post:edit">
                <Button
                  variant="ghost"
                  size="icon"
                  title="编辑"
                  aria-label="编辑"
                  onclick={() => openEdit(r)}
                >
                  <Pencil class="size-4" />
                </Button>
              </HasPermi>
              <HasPermi perm="system:post:remove">
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
    </div>
  </div>
</Main>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑岗位' : '新增岗位'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="postCode">岗位编码</Label>
    <Input
      id="postCode"
      bind:value={form.postCode}
      placeholder="请输入岗位编码"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="postName">岗位名称</Label>
    <Input
      id="postName"
      bind:value={form.postName}
      placeholder="请输入岗位名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="postCategory">类别编码</Label>
    <Input
      id="postCategory"
      bind:value={form.postCategory}
      placeholder="请输入类别编码"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="postSort">显示顺序</Label>
    <Input
      id="postSort"
      type="number"
      bind:value={form.postSort}
      placeholder="请输入显示顺序"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label>归属部门</Label>
    <TreeSelect
      treeData={deptTree}
      value={form.deptId ?? undefined}
      placeholder="请选择归属部门(必填)"
      onChange={(id) => {
        form.deptId = String(id)
      }}
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" bind:value={form.status} class={SELECT_CLASS}>
      <option value="0">正常</option>
      <option value="1">停用</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="remark">备注</Label>
    <Textarea
      id="remark"
      bind:value={form.remark}
      placeholder="请输入备注"
      rows={3}
    />
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除岗位「${deleteTarget?.postName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 个岗位吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>
