<script lang="ts">
  import { onMount } from 'svelte'
  import { categoryApi } from '$lib/api/workflow/category.js'
  import type {
    CategoryVo,
    CategoryCreateBo,
    CategoryEditBo,
  } from '$lib/api/workflow/category.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import { TreeTableShell, buildTree } from '$lib/components/crud/index.js'
  import { toTreeSelectNodes } from '$lib/components/tree-select-utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Plus, Pencil, Trash2 } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- tree state ----
  let tree = $state<CategoryVo[]>([])
  let loading = $state(false)

  // ---- CAT1 筛选(服务端过滤) ----
  let filterCategoryName = $state<string | undefined>(undefined)

  const queryFields = [
    { key: 'categoryName', label: '分类名称', placeholder: '请输入分类名称' },
  ]

  function handleSearch(model: Record<string, string>) {
    filterCategoryName = model.categoryName || undefined
    load()
  }
  function handleReset() {
    filterCategoryName = undefined
    load()
  }

  async function load() {
    loading = true
    try {
      const resp = await categoryApi.list(filterCategoryName)
      tree = resp.data ?? []
    } catch {
      // apiClient 已经 toast 错误,这里不重复提示
    } finally {
      loading = false
    }
  }

  onMount(() => {
    load()
  })

  // ---- create / edit dialog ----
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  const isEdit = $derived(editingId !== null)

  interface CategoryFormModel {
    categoryName: string
    parentId: string
    orderNum: number
  }

  function emptyForm(parentId = '0'): CategoryFormModel {
    return {
      categoryName: '',
      parentId,
      orderNum: 0,
    }
  }

  let form = $state<CategoryFormModel>(emptyForm())

  function openCreate(parentId = '0') {
    editingId = null
    form = emptyForm(parentId)
    dialogOpen = true
  }

  function openEdit(row: CategoryVo) {
    editingId = row.categoryId
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)
    form = {
      categoryName: row.categoryName ?? '',
      parentId: row.parentId ?? '0',
      orderNum: row.orderNum ?? 0,
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    // CAT4:对齐 Vue rules(index.vue:166-170) —— parentId 与 categoryName 均必填
    if (!form.categoryName) {
      toast.error('请输入分类名称')
      return
    }
    if (!form.parentId) {
      toast.error('请选择上级分类')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: CategoryEditBo = {
          categoryId: editingId,
          parentId: form.parentId,
          categoryName: form.categoryName,
          orderNum: form.orderNum,
        }
        await categoryApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: CategoryCreateBo = {
          parentId: form.parentId,
          categoryName: form.categoryName,
          orderNum: form.orderNum,
        }
        await categoryApi.create(payload)
        toast.success('新增成功')
      }
      dialogOpen = false
      load()
    } catch {
      // apiClient 已 toast
    } finally {
      saving = false
    }
  }

  // ---- delete confirm ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<CategoryVo | null>(null)

  function openDelete(row: CategoryVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await categoryApi.delete(deleteTarget.categoryId)
      toast.success('删除成功')
      deleteOpen = false
      deleteTarget = null
      load()
    } catch {
      // apiClient 已 toast
    } finally {
      deleting = false
    }
  }

  const columns: ColumnDef<CategoryVo>[] = [
    { id: 'categoryName', accessorKey: 'categoryName', header: '分类名称' },
    // CAT5:label「排序」→「显示排序」(对齐 Vue)
    { id: 'orderNum', accessorKey: 'orderNum', header: '显示排序' },
    {
      id: 'createTime',
      accessorKey: 'createTime',
      header: '创建时间',
      accessorFn: (r) => r.createTime ?? '-',
    },
    { id: 'actions', header: '操作' },
  ]

  const treeData = $derived(
    buildTree(tree as unknown as Record<string, unknown>[], {
      idKey: 'categoryId',
      parentKey: 'parentId',
      rootValue: '0',
    }) as unknown as CategoryVo[]
  )

  // CAT2:上级分类改 tree-select —— 复用页面已有的树数据,避免再拉一遍 categoryTree。
  const parentTreeData = $derived(
    toTreeSelectNodes(treeData as unknown as Record<string, unknown>[], {
      idKey: 'categoryId',
      labelKey: 'categoryName',
    })
  )

  // 编辑时排除自身及子孙,否则能把分类挂到自己底下形成环。
  const currentCategoryId = $derived(editingId ?? undefined)
</script>

<div class="space-y-4 p-4 md:p-6">
  <TreeTableShell
    data={treeData}
    {columns}
    isLoading={loading}
    error={null}
    treeColumnId="categoryName"
    showSearch={false}
    onRefresh={load}
    emptyMessage="暂无分类数据。"
    title="分类列表"
  >
    {#snippet toolbar()}
      <HasPermi perm="workflow:category:add">
        <Button size="sm" onclick={() => openCreate('0')}>
          <Plus class="mr-1 h-4 w-4" />新增
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
    {#snippet cellRenderer({ row })}
      {@const node = row as CategoryVo}
      <div class="flex items-center gap-1">
        <HasPermi perm="workflow:category:add">
          <Button
            variant="ghost"
            size="icon"
            title="新增子分类"
            aria-label="新增子分类"
            onclick={() => openCreate(node.categoryId)}
          >
            <Plus class="size-4" />
          </Button>
        </HasPermi>
        <HasPermi perm="workflow:category:edit">
          <Button
            variant="ghost"
            size="icon"
            title="编辑"
            aria-label="编辑"
            onclick={() => openEdit(node)}
          >
            <Pencil class="size-4" />
          </Button>
        </HasPermi>
        <HasPermi perm="workflow:category:remove">
          <Button
            variant="ghost"
            size="icon"
            class="text-destructive hover:text-destructive"
            title="删除"
            aria-label="删除"
            onclick={() => openDelete(node)}
          >
            <Trash2 class="size-4" />
          </Button>
        </HasPermi>
      </div>
    {/snippet}
  </TreeTableShell>
</div>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '修改流程分类' : '添加流程分类'}
  loading={saving}
  class="sm:max-w-[520px]"
  onConfirm={handleConfirm}
>
  <div class="space-y-4">
    <!-- CAT2:上级分类从裸 number 输入改为 tree-select -->
    <div class="flex flex-col gap-3">
      <Label>上级分类</Label>
      <TreeSelect
        treeData={parentTreeData}
        value={form.parentId}
        excludeIds={currentCategoryId !== undefined ? [currentCategoryId] : []}
        rootOption={{ id: '0', label: '顶级分类' }}
        onChange={(id) => {
          form.parentId = String(id)
        }}
      />
    </div>
    <div class="flex flex-col gap-3">
      <Label for="categoryName">分类名称</Label>
      <Input
        id="categoryName"
        bind:value={form.categoryName}
        placeholder="请输入分类名称"
      />
    </div>
    <div class="flex flex-col gap-3">
      <Label for="orderNum">显示排序</Label>
      <Input
        id="orderNum"
        type="number"
        bind:value={form.orderNum}
        placeholder="请输入显示排序"
      />
    </div>
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除分类「${deleteTarget?.categoryName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>
