<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'

  import { roleApi } from '$lib/api/system/role.js'
  import type { UserListVo } from '$lib/api/system/user.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import DictTag from '$lib/components/DictTag.svelte'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Button } from '$lib/components/ui/button/index.js'

  let {
    open = false,
    roleId,
    onOpenChange,
    onGranted,
  }: {
    open?: boolean
    roleId: string
    onOpenChange: (v: boolean) => void
    /** 授权成功后通知父页刷新已授权列表 */
    onGranted: () => void
  } = $props()

  let filters = $state<{ userName?: string; phoneNumber?: string }>({})
  let saving = $state(false)

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
  ]

  const customCellIds = new Set(['status'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await roleApi.unallocatedList(
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
    getRowId: (row) => String(row.userId),
    getExtraParams: () => ({
      userName: filters.userName,
      phoneNumber: filters.phoneNumber,
    }),
    // 与父页同路由,不加 scope 会互相覆盖 sessionStorage 的分页缓存。
    cacheScope: 'select-user',
  })

  // 弹窗每次打开都重拉未授权列表(对齐 Vue selectUser.vue 的 show() → getList())。
  // 组件被父页无条件挂载,不重拉的话上次已授权的人还留在候选里,再次确定会重复授权。
  $effect(() => {
    if (open) crudTable.reload()
  })

  function handleSearch(model: Record<string, string>) {
    filters = { userName: model.userName, phoneNumber: model.phoneNumber }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  async function handleConfirm() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) {
      toast.error('请选择要分配的用户')
      return
    }
    saving = true
    try {
      await roleApi.selectAuthUsers(roleId, ids)
      toast.success('分配成功')
      crudTable.clearSelection()
      onOpenChange(false)
      onGranted()
    } catch {
      // apiClient 已 toast
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root {open} onOpenChange={(v) => onOpenChange(v)}>
  <Dialog.Content class="sm:max-w-[800px]">
    <Dialog.Header>
      <Dialog.Title>选择用户</Dialog.Title>
    </Dialog.Header>
    <div class="py-2">
      <CrudTableShell
        table={crudTable.table}
        isLoading={crudTable.isLoading}
        error={crudTable.error}
        columnCount={columns.length}
        {customCellIds}
        emptyMessage="没有可分配的用户。"
        globalFilter=""
        showSearch={false}
        showRefresh={false}
        selectable
        title="待选用户">
        {#snippet queryForm()}
          <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
        {/snippet}
        {#snippet cellRenderer({ row, columnId })}
          {@const r = row as UserListVo}
          {#if columnId === 'status'}
            <DictTag dictType="sys_normal_disable" value={r.status} />
          {:else}
            <span></span>
          {/if}
        {/snippet}
      </CrudTableShell>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => onOpenChange(false)} disabled={saving}>取消</Button>
      <Button onclick={handleConfirm} disabled={saving}>{saving ? '提交中...' : '确定'}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
