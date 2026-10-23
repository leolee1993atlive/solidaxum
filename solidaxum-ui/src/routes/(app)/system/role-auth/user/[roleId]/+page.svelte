<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'

  import { roleApi } from '$lib/api/system/role.js'
  import type { UserListVo } from '$lib/api/system/user.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import SelectUserDialog from './select-user-dialog.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Plus, CircleX, X } from '@lucide/svelte'

  const roleId = page.params.roleId!

  // 「选择用户」弹窗开关 —— T8 在此文件末尾接入 SelectUserDialog 组件。
  let selectUserOpen = $state(false)

  // ---- 筛选(Vue authUser.vue 只有用户名称/手机号码两项) ----
  let filters = $state<{ userName?: string; phoneNumber?: string }>({})

  const queryFields = [
    { key: 'userName', label: '用户名称', placeholder: '请输入用户名称' },
    { key: 'phoneNumber', label: '手机号码', placeholder: '请输入手机号码' },
  ]

  // status 列走 customCellIds + DictTag(sys_normal_disable),不再硬编码文本映射。
  const columns: ColumnDef<UserListVo>[] = [
    { accessorKey: 'userName', header: '用户名称' },
    { accessorKey: 'nickName', header: '用户昵称' },
    { accessorKey: 'email', header: '邮箱', cell: (info) => (info.getValue() as string | undefined) || '-' },
    { accessorKey: 'phoneNumber', header: '手机', cell: (info) => (info.getValue() as string | undefined) || '-' },
    { accessorKey: 'status', header: '状态', enableSorting: false },
    { accessorKey: 'createTime', header: '创建时间', cell: (info) => (info.getValue() as string | undefined) || '-' },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  const customCellIds = new Set(['status'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await roleApi.allocatedList(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      { roleId, ...(params.extra ?? {}) }
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<UserListVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    // 业务主键做 row id —— manualPagination 下不给会串页。
    getRowId: (row) => String(row.userId),
    getExtraParams: () => ({
      userName: filters.userName,
      phoneNumber: filters.phoneNumber,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = { userName: model.userName, phoneNumber: model.phoneNumber }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 取消单个授权 ----
  let cancelOpen = $state(false)
  let canceling = $state(false)
  let cancelTarget = $state<UserListVo | null>(null)

  function openCancel(row: UserListVo) {
    cancelTarget = row
    cancelOpen = true
  }

  async function handleCancel() {
    if (!cancelTarget) return
    canceling = true
    try {
      await roleApi.cancelAuthUser(roleId, cancelTarget.userId as string)
      toast.success('取消授权成功')
      cancelOpen = false
      cancelTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      canceling = false
    }
  }

  // ---- 批量取消授权 ----
  let batchCancelOpen = $state(false)
  let batchCanceling = $state(false)

  async function handleBatchCancel() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchCanceling = true
    try {
      await roleApi.cancelAuthUsers(roleId, ids)
      toast.success('批量取消授权成功')
      batchCancelOpen = false
      crudTable.clearSelection()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchCanceling = false
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
    emptyMessage="该角色暂无已授权用户。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    selectable
    title="已授权用户">
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet toolbar({ selectedCount })}
      <Button variant="outline" size="sm" onclick={() => goto('/system/role')}>
        <X class="mr-1 h-4 w-4" />关闭
      </Button>
      <HasPermi perm="system:role:add">
        <Button size="sm" onclick={() => (selectUserOpen = true)}>
          <Plus class="mr-1 h-4 w-4" />添加
        </Button>
      </HasPermi>
      <HasPermi perm="system:role:remove">
        <Button
          variant="destructive"
          size="sm"
          disabled={selectedCount === 0}
          onclick={() => (batchCancelOpen = true)}>
          <CircleX class="mr-1 h-4 w-4" />取消
        </Button>
      </HasPermi>
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as UserListVo}
      {#if columnId === 'status'}
        <DictTag dictType="sys_normal_disable" value={r.status} />
      {:else}
        <HasPermi perm="system:role:remove">
          <Button
            variant="ghost"
            size="icon"
            title="取消授权"
            aria-label="取消授权"
            onclick={() => openCancel(r)}>
            <CircleX class="size-4" />
          </Button>
        </HasPermi>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<ConfirmDialog
  open={cancelOpen}
  onOpenChange={(v) => (cancelOpen = v)}
  title="确认取消授权"
  desc={`确定要取消用户「${cancelTarget?.userName ?? ''}」的本角色授权吗?`}
  confirmText="取消授权"
  destructive
  isLoading={canceling}
  handleConfirm={handleCancel} />

<ConfirmDialog
  open={batchCancelOpen}
  onOpenChange={(v) => (batchCancelOpen = v)}
  title="确认批量取消授权"
  desc={`确定要取消选中的 ${crudTable.selectedIds.length} 个用户的本角色授权吗?`}
  confirmText="批量取消"
  destructive
  isLoading={batchCanceling}
  handleConfirm={handleBatchCancel} />

<SelectUserDialog
  open={selectUserOpen}
  roleId={String(roleId)}
  onOpenChange={(v) => (selectUserOpen = v)}
  onGranted={() => crudTable.reload()} />
