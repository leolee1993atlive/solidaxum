<script lang="ts">
  import { goto } from '$app/navigation'
  // 数据范围值域是后端语义,单独成模块并由 data-scope.test.ts 钉住(5/6 曾被写反)。
  import { DATA_SCOPE_OPTIONS } from '$lib/api/system/data-scope.js'
  import { menuApi } from '$lib/api/system/menu.js'
  import { roleApi, SUPER_ADMIN_ROLE_ID } from '$lib/api/system/role.js'
  import type {
    RoleListVo,
    RoleCreateBo,
    RoleUpdateBo,
  } from '$lib/api/system/role.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import PermissionTree from '$lib/components/PermissionTree.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import StatusSwitchCell from '$lib/components/StatusSwitchCell.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import {
    buildPermissionTree,
    fromTreeSelectNodes,
    submitIds,
    type PermissionNode,
  } from '$lib/components/permission-tree.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import {
    Plus,
    Pencil,
    Trash2,
    CircleCheck,
    Users,
    Download,
  } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{
    roleName?: string
    roleKey?: string
    status?: string
    'params[beginTime]'?: string
    'params[endTime]'?: string
  }>({})

  const queryFields = [
    { key: 'roleName', label: '角色名称', placeholder: '请输入角色名称' },
    { key: 'roleKey', label: '权限字符', placeholder: '请输入权限字符' },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_normal_disable',
    },
    {
      key: 'createTime',
      label: '创建时间',
      type: 'dateRange' as const,
      // ⚠️ 后端 RolePageQuery 用 #[serde(rename = "params[beginTime]")](role_handler.rs:38-41),
      // 不是裸 beginTime。照 operlog 页(+page.svelte:45-46)的写法。
      beginKey: 'params[beginTime]',
      endKey: 'params[endTime]',
    },
  ]

  // SELECT_CLASS: 统一原生 select 外观,匹配 shadcn input。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // 目标列对齐 Vue role/index.vue:93-116:
  // 角色编号 v-if="false"(R11 删)· 数据范围/备注 Vue 无此列(R11 删)· 创建时间补上(R7)
  const columns: ColumnDef<RoleListVo>[] = [
    { accessorKey: 'roleName', header: '角色名称' },
    { accessorKey: 'roleKey', header: '权限字符' },
    { accessorKey: 'roleSort', header: '显示顺序' },
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
    const resp = await roleApi.page(
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

  const crudTable = useCrudTable<RoleListVo>({
    columns,
    // 后端 role 排序白名单(role_service.rs:224-229):roleId/roleName/roleSort/createTime。
    // roleKey 不在白名单里,原来那条是悬空的,一并去掉。
    sortableIds: new Set(['roleName', 'roleSort', 'createTime']),
    fetchPage,
    // 🔴 selectable 必须配 getRowId,否则 enableRowSelection = Boolean(getRowId) = false,
    // 勾选框照常渲染但 rowSelection 恒空 → 批量按钮永远 disabled。
    getRowId: (row) => row.roleId,
    getExtraParams: () => ({
      roleName: filters.roleName,
      roleKey: filters.roleKey,
      status: filters.status,
      'params[beginTime]': filters['params[beginTime]'],
      'params[endTime]': filters['params[endTime]'],
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      roleName: model.roleName,
      roleKey: model.roleKey,
      status: model.status,
      'params[beginTime]': model['params[beginTime]'],
      'params[endTime]': model['params[endTime]'],
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- R8 批量删除 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  /** 超管角色不可删(后端 check_role_allowed 会 400),先在前端剔掉。 */
  const deletableIds = $derived(
    crudTable.selectedIds.filter((id) => id !== SUPER_ADMIN_ROLE_ID)
  )

  async function handleBatchDelete() {
    if (deletableIds.length === 0) return
    batchDeleting = true
    try {
      await roleApi.delete(deletableIds.join(','))
      toast.success('批量删除成功')
      batchDeleteOpen = false
      // 🔴 reload 不动 rowSelection,已删的业务主键会留在选择集里 ——
      // 批量按钮保持可点、计数是旧的,再点一次就是对已删 id 重发 DELETE。
      crudTable.clearSelection()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchDeleting = false
    }
  }

  // ---- R9 导出 ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await roleApi.export({ ...filters })
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛裸 Error,不经 apiClient,全局没人 toast —— 不自己报就是「点了没反应」。
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

  interface RoleFormModel {
    roleName: string
    roleKey: string
    roleSort: number
    dataScope: string
    status: string
    remark: string
  }

  // dataScope 不在基础表单里出现(对齐 Vue —— 它属于「分配权限」弹窗的数据权限 Tab),
  // 但仍随 update 原值回传,避免编辑基础信息时把已配置的数据范围重置。
  function emptyForm(): RoleFormModel {
    return {
      roleName: '',
      roleKey: '',
      roleSort: 0,
      dataScope: '1',
      status: '0',
      remark: '',
    }
  }

  let form = $state<RoleFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: RoleListVo) {
    editingId = row.roleId
    form = {
      roleName: row.roleName ?? '',
      roleKey: row.roleKey ?? '',
      roleSort: row.roleSort ?? 0,
      dataScope: row.dataScope ?? '1',
      status: row.status ?? '0',
      remark: row.remark ?? '',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.roleName || !form.roleKey) {
      toast.error('角色名称和权限字符不能为空')
      return
    }
    // R12:Vue rules 三项必填(index.vue:798-802),roleSort 也在内。
    if (
      form.roleSort === undefined ||
      form.roleSort === null ||
      Number.isNaN(form.roleSort)
    ) {
      toast.error('角色顺序不能为空')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: RoleUpdateBo = {
          roleId: editingId,
          roleName: form.roleName,
          roleKey: form.roleKey,
          roleSort: form.roleSort,
          dataScope: form.dataScope || undefined,
          status: form.status,
          remark: form.remark || undefined,
        }
        await roleApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: RoleCreateBo = {
          roleName: form.roleName,
          roleKey: form.roleKey,
          roleSort: form.roleSort,
          dataScope: form.dataScope || undefined,
          status: form.status,
          remark: form.remark || undefined,
        }
        await roleApi.create(payload)
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

  // ---- 分配权限弹窗(菜单权限 / 数据权限两个 Tab) ----
  let permOpen = $state(false)
  let permSaving = $state(false)
  let permRole = $state<RoleListVo | null>(null)
  let permTab = $state('menu')

  let menuNodes = $state<PermissionNode[]>([])
  let menuChecked = $state(new Set<string>())
  let menuLinked = $state(true)

  let deptNodes = $state<PermissionNode[]>([])
  let deptChecked = $state(new Set<string>())
  let deptLinked = $state(true)
  let dataScope = $state('1')

  async function openPermission(row: RoleListVo) {
    permRole = row
    permTab = 'menu'
    dataScope = row.dataScope ?? '1'
    menuLinked = row.menuCheckStrictly ?? true
    deptLinked = row.deptCheckStrictly ?? true
    menuNodes = []
    deptNodes = []
    menuChecked = new Set()
    deptChecked = new Set()
    permOpen = true
    try {
      const [menuRes, deptRes] = await Promise.all([
        menuApi.roleMenuTreeselect(String(row.roleId)),
        roleApi.deptTree(row.roleId),
      ])
      menuNodes = buildPermissionTree(menuRes.data?.menus ?? [])
      // 回填不做级联:后端返回的 checkedKeys 已含半选父节点,级联会把未授权的兄弟节点也勾上。
      menuChecked = new Set(menuRes.data?.checkedKeys ?? [])
      deptNodes = fromTreeSelectNodes(deptRes.data?.depts ?? [])
      deptChecked = new Set(deptRes.data?.checkedKeys ?? [])
    } catch {
      // apiClient 已 toast
    }
  }

  async function handlePermissionConfirm() {
    if (!permRole) return
    permSaving = true
    try {
      await roleApi.updatePermission({
        roleId: permRole.roleId,
        menuIds: submitIds(menuNodes, menuChecked, { linked: menuLinked }),
        // 仅「自定义数据权限」才提交部门集合,对齐 Vue submitDataScope。
        deptIds:
          dataScope === '2'
            ? submitIds(deptNodes, deptChecked, { linked: deptLinked })
            : [],
        dataScope,
        menuCheckStrictly: menuLinked,
        deptCheckStrictly: deptLinked,
      })
      toast.success('分配权限成功')
      permOpen = false
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      permSaving = false
    }
  }

  // ---- delete confirm ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<RoleListVo | null>(null)

  function openDelete(row: RoleListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await roleApi.delete(deleteTarget.roleId)
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
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    {customCellIds}
    selectable
    title="角色列表"
    emptyMessage="未找到角色数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
  >
    {#snippet toolbar({ selectedCount })}
      <HasPermi perm="system:role:export">
        <Button
          variant="outline"
          size="sm"
          onclick={handleExport}
          disabled={exporting}
        >
          <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
        </Button>
      </HasPermi>
      <HasPermi perm="system:role:add">
        <Button size="sm" onclick={openCreate}>
          <Plus class="mr-1 h-4 w-4" />新增
        </Button>
      </HasPermi>
      <HasPermi perm="system:role:remove">
        <Button
          variant="destructive"
          size="sm"
          disabled={selectedCount === 0 || deletableIds.length === 0}
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
      {@const r = row as RoleListVo}
      {#if columnId === 'status'}
        {@const builtin = r.roleId === SUPER_ADMIN_ROLE_ID}
        <HasPermi perm="system:role:edit">
          <StatusSwitchCell
            value={r.status}
            checkedValue="0"
            uncheckedValue="1"
            disabled={builtin}
            label={`${r.status === '0' ? '停用' : '启用'}角色 ${r.roleName}`}
            confirmText={`确认要「${r.status === '0' ? '停用' : '启用'}」角色「${r.roleName}」吗?`}
            successMessage="操作成功"
            onConfirm={(next) => roleApi.changeStatus(r.roleId, next)}
            onDone={() => crudTable.reload()}
          />
        </HasPermi>
      {:else}
        {@const builtin = r.roleId === SUPER_ADMIN_ROLE_ID}
        <div class="flex items-center gap-1">
          {#if !builtin}
            <HasPermi perm="system:role:edit">
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
            <HasPermi perm="system:role:remove">
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
            <HasPermi perm="system:role:edit">
              <Button
                variant="ghost"
                size="icon"
                title="分配权限"
                aria-label="分配权限"
                onclick={() => openPermission(r)}
              >
                <CircleCheck class="size-4" />
              </Button>
            </HasPermi>
            <HasPermi perm="system:role:edit">
              <Button
                variant="ghost"
                size="icon"
                title="分配用户"
                aria-label="分配用户"
                onclick={() => goto(`/system/role-auth/user/${r.roleId}`)}
              >
                <Users class="size-4" />
              </Button>
            </HasPermi>
          {/if}
        </div>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑角色' : '新增角色'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="roleName">角色名称</Label>
    <Input
      id="roleName"
      bind:value={form.roleName}
      placeholder="请输入角色名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="roleKey">权限字符</Label>
    <Input
      id="roleKey"
      bind:value={form.roleKey}
      placeholder="请输入权限字符"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="roleSort">显示顺序</Label>
    <Input
      id="roleSort"
      type="number"
      bind:value={form.roleSort}
      placeholder="请输入显示顺序"
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
  desc={`确定要删除角色「${deleteTarget?.roleName ?? ''}」吗?`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认删除"
  desc={`确定要删除选中的 ${deletableIds.length} 个角色吗?删除后不可恢复。${
    crudTable.selectedIds.length !== deletableIds.length
      ? '（已自动跳过超级管理员角色）'
      : ''
  }`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>

<FormDrawer
  open={permOpen}
  onOpenChange={(o) => (permOpen = o)}
  title={`分配权限 —— ${permRole?.roleName ?? ''}`}
  loading={permSaving}
  class="sm:max-w-[760px]"
  onConfirm={handlePermissionConfirm}
>
  <Tabs.Root bind:value={permTab}>
    <Tabs.List>
      <Tabs.Trigger value="menu">菜单权限</Tabs.Trigger>
      <Tabs.Trigger value="data">数据权限</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="menu" class="pt-3">
      <PermissionTree
        nodes={menuNodes}
        checked={menuChecked}
        linked={menuLinked}
        onChange={(next) => (menuChecked = next)}
        onLinkedChange={(next) => (menuLinked = next)}
      />
    </Tabs.Content>
    <Tabs.Content value="data" class="space-y-3 pt-3">
      <div class="flex flex-col gap-3">
        <Label for="permDataScope">权限范围</Label>
        <select id="permDataScope" bind:value={dataScope} class={SELECT_CLASS}>
          {#each DATA_SCOPE_OPTIONS as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>
      {#if dataScope === '2'}
        <PermissionTree
          nodes={deptNodes}
          checked={deptChecked}
          linked={deptLinked}
          showButtonColumn={false}
          onChange={(next) => (deptChecked = next)}
          onLinkedChange={(next) => (deptLinked = next)}
        />
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</FormDrawer>
