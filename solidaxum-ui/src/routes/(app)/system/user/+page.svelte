<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { userFormIssues, resetPwdIssues } from '$lib/api/system/user-rules.js'
  import { userApi, SUPER_ADMIN_USER_ID } from '$lib/api/system/user.js'
  import type {
    UserListVo,
    UserCreateBo,
    UserUpdateBo,
  } from '$lib/api/system/user.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import MultiSelect from '$lib/components/MultiSelect.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import StatusSwitchCell from '$lib/components/StatusSwitchCell.svelte'
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
  import { useAuth } from '$lib/context/auth.svelte.js'
  import {
    Plus,
    Pencil,
    Trash2,
    KeyRound,
    CircleCheck,
    Download,
    Upload,
    Unlock,
  } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'
  import UserImportDialog from './user-import-dialog.svelte'
  import UserViewDrawer from './user-view-drawer.svelte'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{
    userName?: string
    nickName?: string
    phoneNumber?: string
    status?: string
    'params[beginTime]'?: string
    'params[endTime]'?: string
    deptId?: string
  }>({})

  // ---- 左侧部门树(点节点按 deptId 过滤列表) ----
  let deptTree = $state<TreeSelectNode[]>([])

  onMount(async () => {
    try {
      const res = await userApi.deptTree()
      deptTree = res.data ?? []
    } catch {
      // apiClient 已 toast
    }
  })

  function handleDeptClick(id: string | undefined) {
    filters = { ...filters, deptId: id }
    crudTable.resetPagination()
  }

  const queryFields = [
    { key: 'userName', label: '用户名称', placeholder: '请输入用户名称' },
    { key: 'nickName', label: '用户昵称', placeholder: '请输入用户昵称' },
    { key: 'phoneNumber', label: '手机号码', placeholder: '请输入手机号码' },
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
      // UserPageQuery 用 #[serde(rename = "params[beginTime]")](user_handler.rs:84-87)
      beginKey: 'params[beginTime]',
      endKey: 'params[endTime]',
    },
  ]

  // SELECT_CLASS: 统一原生 select 外观,匹配 shadcn input。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // 目标列对齐 Vue user/index.vue:163-215:
  // 用户编号 visible:false(不做)· 性别 Vue 列表无此列(不做,只留弹窗里的)
  const columns: ColumnDef<UserListVo>[] = [
    { accessorKey: 'userName', header: '用户名称' },
    { accessorKey: 'nickName', header: '用户昵称' },
    {
      accessorKey: 'deptName',
      header: '部门',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'phoneNumber',
      header: '手机号码',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { accessorKey: 'status', header: '状态', enableSorting: false },
    {
      accessorKey: 'createTime',
      header: '创建时间',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  // userName 列可点(详情抽屉),故一并进 customCellIds。
  const customCellIds = new Set(['userName', 'status'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await userApi.page(
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

  const crudTable = useCrudTable<UserListVo>({
    columns,
    // 后端 user 排序白名单(user_service.rs:130-134):userId/userName/nickName/createTime。
    // 编号列已删,userId 随之去掉。
    sortableIds: new Set(['userName', 'nickName', 'createTime']),
    fetchPage,
    // 🔴 selectable 必须配 getRowId(同 role/post 页)。
    getRowId: (row) => row.userId,
    getExtraParams: () => ({
      userName: filters.userName,
      nickName: filters.nickName,
      phoneNumber: filters.phoneNumber,
      status: filters.status,
      'params[beginTime]': filters['params[beginTime]'],
      'params[endTime]': filters['params[endTime]'],
      deptId: filters.deptId,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      ...filters,
      userName: model.userName,
      nickName: model.nickName,
      phoneNumber: model.phoneNumber,
      status: model.status,
      'params[beginTime]': model['params[beginTime]'],
      'params[endTime]': model['params[endTime]'],
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    // 重置保留 deptId(左侧部门树的选择)
    filters = { deptId: filters.deptId }
    crudTable.resetPagination()
  }

  // ---- create / edit dialog ----
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  const isEdit = $derived(editingId !== null)

  interface UserFormModel {
    userName: string
    password: string
    nickName: string
    phoneNumber: string
    email: string
    gender: string
    status: string
    deptId: string | undefined
    remark: string
    roleIds: string[]
    postIds: string[]
  }

  function emptyForm(): UserFormModel {
    return {
      userName: '',
      password: '',
      nickName: '',
      phoneNumber: '',
      email: '',
      gender: '0',
      status: '0',
      deptId: undefined,
      remark: '',
      roleIds: [],
      postIds: [],
    }
  }

  let form = $state<UserFormModel>(emptyForm())

  // 角色/岗位可选项 —— 后端 GET /system/user/(新增) 与 /system/user/{id}(编辑) 都会返回。
  let roleOptions = $state<
    { label: string; value: string; disabled?: boolean }[]
  >([])
  let postOptions = $state<
    { label: string; value: string; disabled?: boolean }[]
  >([])

  const auth = useAuth()
  // 对齐 Vue:编辑自己时隐藏归属部门/角色/岗位(防止把自己踢出部门或摘掉自己的角色)。
  const editingSelf = $derived(
    editingId !== null && editingId === String(auth.user?.userId ?? '')
  )

  async function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
    try {
      const res = await userApi.getAddInfo()
      roleOptions = (res.data?.roles ?? []).map((r) => ({
        label: r.roleName,
        value: r.roleId,
        disabled: r.status === '1',
      }))
      postOptions = (res.data?.posts ?? []).map((p) => ({
        label: p.postName,
        value: p.postId,
        disabled: p.status === '1',
      }))
    } catch {
      // apiClient 已 toast
    }
  }

  async function openEdit(row: UserListVo) {
    editingId = row.userId
    dialogOpen = true
    try {
      // 必须走详情:列表行没有 roleIds/postIds。
      const res = await userApi.getById(row.userId)
      const u = res.data?.user
      roleOptions = (res.data?.roles ?? []).map((r) => ({
        label: r.roleName,
        value: r.roleId,
        disabled: r.status === '1',
      }))
      postOptions = (res.data?.posts ?? []).map((p) => ({
        label: p.postName,
        value: p.postId,
        disabled: p.status === '1',
      }))
      form = {
        userName: u?.userName ?? row.userName ?? '',
        password: '',
        nickName: u?.nickName ?? row.nickName ?? '',
        phoneNumber: u?.phoneNumber ?? row.phoneNumber ?? '',
        email: u?.email ?? row.email ?? '',
        gender: u?.gender ?? row.gender ?? '0',
        status: u?.status ?? row.status ?? '0',
        deptId: u?.deptId ?? row.deptId,
        remark: u?.remark ?? row.remark ?? '',
        roleIds: res.data?.roleIds ?? [],
        postIds: res.data?.postIds ?? [],
      }
    } catch {
      // apiClient 已 toast
    }
  }

  async function handleConfirm() {
    const issues = userFormIssues(form, { isEdit, editingSelf })
    if (issues.length > 0) {
      toast.error(issues[0])
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: UserUpdateBo = {
          userId: editingId,
          userName: form.userName,
          nickName: form.nickName,
          phoneNumber: form.phoneNumber || undefined,
          email: form.email || undefined,
          gender: form.gender || undefined,
          status: form.status || undefined,
          deptId: form.deptId,
          remark: form.remark || undefined,
          roleIds: form.roleIds,
          postIds: form.postIds,
        }
        await userApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: UserCreateBo = {
          userName: form.userName,
          password: form.password,
          nickName: form.nickName,
          phoneNumber: form.phoneNumber || undefined,
          email: form.email || undefined,
          gender: form.gender || undefined,
          status: form.status || undefined,
          deptId: form.deptId,
          remark: form.remark || undefined,
          roleIds: form.roleIds,
          postIds: form.postIds,
        }
        await userApi.create(payload)
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
  let deleteTarget = $state<UserListVo | null>(null)

  function openDelete(row: UserListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await userApi.delete(deleteTarget.userId)
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

  // ---- 用户名列点击 → 详情抽屉(对齐 Vue el-link + handleViewDetail) ----
  let viewOpen = $state(false)
  let viewUserId = $state<string | undefined>(undefined)

  function openView(row: UserListVo) {
    viewUserId = row.userId
    viewOpen = true
  }

  // ---- 重置密码(Vue 用 ElMessageBox.prompt,本仓用 ConfirmDialog + 一个输入框) ----
  let resetPwdOpen = $state(false)
  let resetPwdSaving = $state(false)
  let resetPwdTarget = $state<UserListVo | null>(null)
  let newPassword = $state('')

  function openResetPwd(row: UserListVo) {
    resetPwdTarget = row
    newPassword = ''
    resetPwdOpen = true
  }

  async function handleResetPwd() {
    if (!resetPwdTarget) return
    const issues = resetPwdIssues(newPassword)
    if (issues.length > 0) {
      toast.error(issues[0])
      return
    }
    resetPwdSaving = true
    try {
      await userApi.resetPwd(resetPwdTarget.userId, newPassword)
      toast.success('修改成功，新密码是：' + newPassword)
      resetPwdOpen = false
      resetPwdTarget = null
    } catch {
      // apiClient 已 toast
    } finally {
      resetPwdSaving = false
    }
  }

  // ---- 批量删除 / 解锁 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  /**
   * 后端两条硬拒绝(user_handler.rs:338-341 + check_user_allowed):
   * ① 不能删当前登录用户 ② 不能删超管。整批里有一个就整批 400,故前端先剔掉。
   */
  const deletableIds = $derived(
    crudTable.selectedIds.filter(
      (id) =>
        id !== SUPER_ADMIN_USER_ID && id !== String(auth.user?.userId ?? '')
    )
  )

  async function handleBatchDelete() {
    if (deletableIds.length === 0) return
    batchDeleting = true
    try {
      await userApi.delete(deletableIds.join(','))
      toast.success('批量删除成功')
      batchDeleteOpen = false
      // 🔴 reload 不动 rowSelection,已删的业务主键会留在选择集里(见 role 页同处注释)。
      // 本页还多一层:解锁的 disabled 门是 selectedCount !== 1,读的是同一张脏表。
      crudTable.clearSelection()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchDeleting = false
    }
  }

  let unlocking = $state(false)
  let unlockOpen = $state(false)

  /** 对齐 Vue:解锁只处理选中的第一条(后端端点是单个 user_id 的 path 参数)。 */
  const unlockTargetId = $derived(crudTable.selectedIds[0])

  /** 对齐 Vue handleUnlock(index.vue:666-673):先 modal.confirm 再调端点。 */
  async function handleUnlock() {
    const id = unlockTargetId
    if (!id) return
    unlocking = true
    try {
      await userApi.unlock(id)
      toast.success(`用户编号"${id}"解锁成功`)
      unlockOpen = false
    } catch {
      // apiClient 已 toast
    } finally {
      unlocking = false
    }
  }

  // ---- 导出 / 下载模板 ----
  let exporting = $state(false)
  let importOpen = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await userApi.export({ ...filters })
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛裸 Error,不经 apiClient —— 不自己报就是「点了没反应」。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }

  async function handleDownloadTemplate() {
    try {
      await userApi.importTemplate()
    } catch (e) {
      // downloadBlob 同上,裸 Error。
      toast.error(e instanceof Error ? e.message : '模板下载失败')
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex gap-4">
    <TreePanel
      title="部门结构"
      placeholder="请输入部门名称"
      nodes={deptTree}
      selectedId={filters.deptId}
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
        title="用户列表"
        emptyMessage="未找到用户数据。"
        globalFilter=""
        showSearch={false}
        showRefresh={false}
      >
        {#snippet toolbar({ selectedCount })}
          <Button variant="outline" size="sm" onclick={handleDownloadTemplate}>
            <Download class="mr-1 h-4 w-4" />下载模板
          </Button>
          <HasPermi perm="system:user:import">
            <Button
              variant="outline"
              size="sm"
              onclick={() => (importOpen = true)}
            >
              <Upload class="mr-1 h-4 w-4" />导入
            </Button>
          </HasPermi>
          <HasPermi perm="system:user:export">
            <Button
              variant="outline"
              size="sm"
              onclick={handleExport}
              disabled={exporting}
            >
              <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
            </Button>
          </HasPermi>
          <HasPermi perm="system:user:add">
            <Button size="sm" onclick={openCreate}>
              <Plus class="mr-1 h-4 w-4" />新增
            </Button>
          </HasPermi>
          <HasPermi perm="system:user:remove">
            <Button
              variant="destructive"
              size="sm"
              disabled={selectedCount === 0 || deletableIds.length === 0}
              onclick={() => (batchDeleteOpen = true)}
            >
              <Trash2 class="mr-1 h-4 w-4" />删除
            </Button>
          </HasPermi>
          <HasPermi perm="system:user:edit">
            <Button
              variant="outline"
              size="sm"
              disabled={selectedCount !== 1 || unlocking}
              onclick={() => (unlockOpen = true)}
            >
              <Unlock class="mr-1 h-4 w-4" />{unlocking ? '解锁中…' : '解锁'}
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
          {@const r = row as UserListVo}
          {#if columnId === 'userName'}
            <button
              type="button"
              class="text-primary-emphasis hover:underline"
              onclick={() => openView(r)}>{r.userName}</button
            >
          {:else if columnId === 'status'}
            <HasPermi perm="system:user:edit">
              <StatusSwitchCell
                value={r.status}
                checkedValue="0"
                uncheckedValue="1"
                label={`${r.status === '0' ? '停用' : '启用'}用户 ${r.userName}`}
                confirmText={`确认要「${r.status === '0' ? '停用' : '启用'}」用户「${r.userName}」吗?`}
                successMessage="操作成功"
                onConfirm={(next) => userApi.changeStatus(r.userId, next)}
                onDone={() => crudTable.reload()}
              />
            </HasPermi>
          {:else}
            {@const builtin = r.userId === SUPER_ADMIN_USER_ID}
            <div class="flex items-center gap-1">
              {#if !builtin}
                <HasPermi perm="system:user:edit">
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
                <HasPermi perm="system:user:remove">
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
                <HasPermi perm="system:user:resetPwd">
                  <Button
                    variant="ghost"
                    size="icon"
                    title="重置密码"
                    aria-label="重置密码"
                    onclick={() => openResetPwd(r)}
                  >
                    <KeyRound class="size-4" />
                  </Button>
                </HasPermi>
                <HasPermi perm="system:user:query">
                  <Button
                    variant="ghost"
                    size="icon"
                    title="分配角色"
                    aria-label="分配角色"
                    onclick={() => goto(`/system/user-auth/role/${r.userId}`)}
                  >
                    <CircleCheck class="size-4" />
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

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑用户' : '新增用户'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="userName">用户名</Label>
    <Input
      id="userName"
      bind:value={form.userName}
      placeholder="请输入用户名"
      disabled={isEdit}
    />
  </div>
  {#if !isEdit}
    <div class="flex flex-col gap-3">
      <Label for="password">密码</Label>
      <Input
        id="password"
        type="password"
        bind:value={form.password}
        placeholder="请输入密码(5-20位)"
      />
    </div>
  {/if}
  <div class="flex flex-col gap-3">
    <Label for="nickName">昵称</Label>
    <Input id="nickName" bind:value={form.nickName} placeholder="请输入昵称" />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="phoneNumber">手机号</Label>
    <Input
      id="phoneNumber"
      bind:value={form.phoneNumber}
      placeholder="请输入手机号"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="email">邮箱</Label>
    <Input id="email" bind:value={form.email} placeholder="请输入邮箱" />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="gender">性别</Label>
    <select id="gender" class={SELECT_CLASS} bind:value={form.gender}>
      <option value="0">男</option>
      <option value="1">女</option>
      <option value="2">未知</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" class={SELECT_CLASS} bind:value={form.status}>
      <option value="0">正常</option>
      <option value="1">停用</option>
    </select>
  </div>
  {#if !editingSelf}
    <div class="flex flex-col gap-3">
      <Label>归属部门</Label>
      <TreeSelect
        treeData={deptTree}
        value={form.deptId}
        placeholder="请选择归属部门"
        onChange={(id) => {
          form.deptId = String(id)
        }}
      />
    </div>
    <div class="flex flex-col gap-3">
      <Label>岗位</Label>
      <MultiSelect
        options={postOptions}
        values={form.postIds}
        placeholder="请选择岗位"
        onChange={(next) => (form.postIds = next)}
      />
    </div>
    <div class="flex flex-col gap-3">
      <Label>角色</Label>
      <MultiSelect
        options={roleOptions}
        values={form.roleIds}
        placeholder="请选择角色"
        onChange={(next) => (form.roleIds = next)}
      />
    </div>
  {/if}
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
  desc={`确定要删除用户「${deleteTarget?.userName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={resetPwdOpen}
  onOpenChange={(o) => (resetPwdOpen = o)}
  title={`重置密码 —— ${resetPwdTarget?.userName ?? ''}`}
  desc={`请输入新密码(5-20位),不能包含 < > " ' \\ | 等字符。`}
  confirmText="重置密码"
  handleConfirm={handleResetPwd}
  isLoading={resetPwdSaving}
>
  <div class="flex flex-col gap-3">
    <Label for="newPassword">新密码</Label>
    <Input
      id="newPassword"
      type="password"
      bind:value={newPassword}
      placeholder="请输入新密码(5-20位)"
    />
  </div>
</ConfirmDialog>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认删除"
  desc={`确定要删除选中的 ${deletableIds.length} 个用户吗?删除后不可恢复。${
    crudTable.selectedIds.length !== deletableIds.length
      ? '（已自动跳过超级管理员与当前登录用户）'
      : ''
  }`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>

<ConfirmDialog
  open={unlockOpen}
  onOpenChange={(v) => (unlockOpen = v)}
  title="确认解锁"
  desc={`是否确认解锁用户编号为"${unlockTargetId ?? ''}"的数据项?`}
  confirmText="解锁"
  isLoading={unlocking}
  handleConfirm={handleUnlock}
/>

<UserImportDialog
  bind:open={importOpen}
  onImported={() => crudTable.reload()}
/>

<UserViewDrawer bind:open={viewOpen} userId={viewUserId} />
