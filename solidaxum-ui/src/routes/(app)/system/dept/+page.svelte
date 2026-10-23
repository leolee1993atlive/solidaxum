<script lang="ts">
  import { onMount } from 'svelte'
  import { deptApi } from '$lib/api/system/dept.js'
  import type {
    DeptListVo,
    DeptCreateBo,
    DeptUpdateBo,
  } from '$lib/api/system/dept.js'
  import { deptFormIssues } from '$lib/api/system/user-rules.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import { TreeTableShell, buildTree } from '$lib/components/crud/index.js'
  import { filterTreeNodes } from '$lib/components/crud/tree-filter.js'
  import { toTreeSelectNodes } from '$lib/components/tree-select-utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Plus, Pencil, Trash2 } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- tree state ----
  let tree = $state<DeptListVo[]>([])
  let loading = $state(false)

  // SELECT_CLASS 与 role 页一致:统一原生 select 外观,匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  async function load() {
    loading = true
    try {
      const resp = await deptApi.list()
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

  interface DeptFormModel {
    deptName: string
    deptCategory: string
    parentId: string
    orderNum: number
    leader: string | undefined
    phone: string
    email: string
    status: string
  }

  function emptyForm(parentId = '0'): DeptFormModel {
    return {
      deptName: '',
      deptCategory: '',
      parentId,
      orderNum: 0,
      leader: undefined,
      phone: '',
      email: '',
      status: '0',
    }
  }

  let form = $state<DeptFormModel>(emptyForm())

  // 负责人候选。对齐 Vue —— **编辑态取「正在编辑的这个部门」的用户**
  // (`handleUpdate` → `getDeptAllUser(row.deptId)`,dept/index.vue:334),
  // 不是上级部门。取错了会让现任负责人不在候选里(select 渲染成空白、看着像没设),
  // 而能选中的全是不属于本部门的人。
  // 新增态该部门还不存在,只能退而取上级部门的用户(Vue 的 handleAdd 干脆不拉)。
  let deptUsers = $state<{ label: string; value: string }[]>([])

  async function loadDeptUsers(deptId: string) {
    if (!deptId || deptId === '0') {
      deptUsers = []
      return
    }
    try {
      const res = await deptApi.listUserByDept(deptId)
      deptUsers = (res.data ?? []).map((u) => ({
        label: u.nickName ? `${u.userName}(${u.nickName})` : u.userName,
        value: u.userId,
      }))
    } catch {
      // apiClient 已 toast
      deptUsers = []
    }
  }

  function openCreate(parentId = '0') {
    editingId = null
    form = emptyForm(parentId)
    // 新增:部门还不存在,候选取上级部门的用户。
    loadDeptUsers(form.parentId)
    dialogOpen = true
  }

  function openEdit(row: DeptListVo) {
    editingId = row.deptId
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)
    form = {
      deptName: row.deptName ?? '',
      deptCategory: row.deptCategory ?? '',
      parentId: row.parentId ?? '0',
      orderNum: row.orderNum ?? 0,
      leader: row.leader,
      phone: row.phone ?? '',
      email: row.email ?? '',
      status: row.status ?? '0',
    }
    // 编辑:候选是本部门的用户(对齐 Vue handleUpdate),与上级部门无关。
    loadDeptUsers(row.deptId)
    dialogOpen = true
  }

  async function handleConfirm() {
    const issues = deptFormIssues(form)
    if (issues.length > 0) {
      toast.error(issues[0])
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: DeptUpdateBo = {
          deptId: editingId,
          parentId: form.parentId,
          deptName: form.deptName,
          deptCategory: form.deptCategory || undefined,
          orderNum: form.orderNum,
          leader: form.leader,
          phone: form.phone || undefined,
          email: form.email || undefined,
          status: form.status,
        }
        await deptApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: DeptCreateBo = {
          parentId: form.parentId,
          deptName: form.deptName,
          deptCategory: form.deptCategory || undefined,
          orderNum: form.orderNum,
          leader: form.leader,
          phone: form.phone || undefined,
          email: form.email || undefined,
          status: form.status,
        }
        await deptApi.create(payload)
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
  let deleteTarget = $state<DeptListVo | null>(null)

  function openDelete(row: DeptListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await deptApi.delete(deleteTarget.deptId)
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

  // status: 0 正常 / 1 停用 (值来源 sys_normal_disable)

  const columns: ColumnDef<DeptListVo>[] = [
    { id: 'deptName', accessorKey: 'deptName', header: '部门名称' },
    {
      id: 'deptCategory',
      accessorKey: 'deptCategory',
      header: '类别编码',
      accessorFn: (r) => r.deptCategory ?? '-',
    },
    { id: 'orderNum', accessorKey: 'orderNum', header: '排序' },
    {
      id: 'status',
      accessorKey: 'status',
      header: '状态',
      enableSorting: false,
    },
    {
      id: 'createTime',
      accessorKey: 'createTime',
      header: '创建时间',
      accessorFn: (r) => r.createTime ?? '-',
    },
    { id: 'actions', header: '操作' },
  ]

  const customCellIds = new Set(['status'])

  // Vue 有部门名称 / 类别编码 / 状态三项(index.vue:13-40)。
  // 本页是 TreeTableShell 全量树,不走分页请求 —— 前端过滤,不加 query 参数。
  let filters = $state<{
    deptName?: string
    deptCategory?: string
    status?: string
  }>({})

  const queryFields = [
    { key: 'deptName', label: '部门名称', placeholder: '请输入部门名称' },
    { key: 'deptCategory', label: '类别编码', placeholder: '请输入类别编码' },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_normal_disable',
    },
  ]

  function handleSearch(model: Record<string, string>) {
    filters = {
      deptName: model.deptName,
      deptCategory: model.deptCategory,
      status: model.status,
    }
  }
  function handleReset() {
    filters = {}
  }

  const fullTree = $derived(
    buildTree(tree as unknown as Record<string, unknown>[], {
      idKey: 'deptId',
      parentKey: 'parentId',
      rootValue: '0',
    }) as unknown as DeptListVo[]
  )

  const treeData = $derived.by(() => {
    const name = (filters.deptName ?? '').trim()
    const category = (filters.deptCategory ?? '').trim()
    const status = filters.status ?? ''
    return filterTreeNodes(
      fullTree as unknown as Record<string, unknown>[],
      (n) =>
        (name === '' || String(n.deptName ?? '').includes(name)) &&
        (category === '' || String(n.deptCategory ?? '').includes(category)) &&
        (status === '' || n.status === status)
    ) as unknown as DeptListVo[]
  })

  // ⚠️ 上级菜单可选项必须用未过滤的全量树 —— 否则一筛选,上级部门可选项就跟着少了。
  const parentTreeData = $derived(
    toTreeSelectNodes(fullTree as unknown as Record<string, unknown>[], {
      idKey: 'deptId',
      labelKey: 'deptName',
    })
  )

  const currentDeptId = $derived(editingId ?? undefined)
</script>

<div class="space-y-4 p-4 md:p-6">
  <TreeTableShell
    data={treeData}
    {columns}
    {customCellIds}
    isLoading={loading}
    error={null}
    treeColumnId="deptName"
    showSearch={false}
    onRefresh={load}
    emptyMessage="暂无部门数据。"
    title="部门列表"
  >
    {#snippet toolbar()}
      <HasPermi perm="system:dept:add">
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
    {#snippet cellRenderer({ row, columnId })}
      {@const node = row as DeptListVo}
      {#if columnId === 'status'}
        <DictTag dictType="sys_normal_disable" value={node.status} />
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="system:dept:add">
            <Button
              variant="ghost"
              size="icon"
              title="新增子部门"
              aria-label="新增子部门"
              onclick={() => openCreate(node.deptId)}
            >
              <Plus class="size-4" />
            </Button>
          </HasPermi>
          <HasPermi perm="system:dept:edit">
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
          <HasPermi perm="system:dept:remove">
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
      {/if}
    {/snippet}
  </TreeTableShell>
</div>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑部门' : '新增部门'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="deptName">部门名称</Label>
    <Input
      id="deptName"
      bind:value={form.deptName}
      placeholder="请输入部门名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="deptCategory">类别编码</Label>
    <Input
      id="deptCategory"
      bind:value={form.deptCategory}
      placeholder="请输入类别编码"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label>上级部门</Label>
    <TreeSelect
      treeData={parentTreeData}
      value={form.parentId}
      excludeIds={currentDeptId !== undefined ? [currentDeptId] : []}
      rootOption={{ id: '0', label: '顶级部门' }}
      onChange={(id) => {
        form.parentId = String(id)
        // 仅新增态:候选来自上级部门,换了上级就得重拉、原选中作废。
        // 编辑态候选来自本部门,与上级无关 —— 跟着清会把现任负责人白白抹掉。
        if (editingId === null) {
          form.leader = undefined
          loadDeptUsers(form.parentId)
        }
      }}
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
  <div class="flex flex-col gap-3">
    <Label for="leader">负责人</Label>
    <select id="leader" class={SELECT_CLASS} bind:value={form.leader}>
      <option value={undefined}>请选择负责人</option>
      {#each deptUsers as u (u.value)}
        <option value={u.value}>{u.label}</option>
      {/each}
    </select>
    {#if form.parentId && form.parentId !== '0' && deptUsers.length === 0}
      <p class="text-muted-foreground text-xs">该部门下暂无用户</p>
    {/if}
  </div>
  <div class="flex flex-col gap-3">
    <Label for="phone">联系电话</Label>
    <Input id="phone" bind:value={form.phone} placeholder="请输入联系电话" />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="email">邮箱</Label>
    <Input id="email" bind:value={form.email} placeholder="请输入邮箱" />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" class={SELECT_CLASS} bind:value={form.status}>
      <option value="0">正常</option>
      <option value="1">停用</option>
    </select>
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除部门「${deleteTarget?.deptName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>
