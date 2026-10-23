<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'

  import { onlineApi, slicePage } from '$lib/api/monitor/online.js'
  import type { UserOnline } from '$lib/api/monitor/online.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
        import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { LogOut } from '@lucide/svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{ userName?: string; ipaddr?: string }>({})

  const queryFields = [
    { key: 'userName', label: '用户名称', placeholder: '请输入用户名称' },
    { key: 'ipaddr', label: 'IP地址', placeholder: '请输入IP地址' },
  ]

  // ---- columns: deviceType 走 customCellIds + DictTag;actions 列走 cellRenderer。
  //      列序逐字对齐 Vue index.vue:51-85 ----
  const customCellIds = new Set(['deviceType'])
  const columns: ColumnDef<UserOnline>[] = [
    { accessorKey: 'tokenId', header: '会话编号' },
    { accessorKey: 'userName', header: '登录名称' },
    // 后端 UserOnline.client_key 一直返回(online/model.rs:15)。
    {
      accessorKey: 'clientKey',
      header: '客户端',
      accessorFn: (row) => row.clientKey ?? '-',
    },
    // sys_device_type 字典 tag。
    { accessorKey: 'deviceType', header: '设备类型' },
    // 不做「用户昵称」列(Vue 无此列)。
    {
      accessorKey: 'deptName',
      header: '所属部门',
      accessorFn: (row) => row.deptName ?? '-',
    },
    { accessorKey: 'ipaddr', header: '主机' },
    {
      accessorKey: 'loginLocation',
      header: '登录地点',
      accessorFn: (row) => row.loginLocation ?? '-',
    },
    // Vue os 在 browser 前。
    { accessorKey: 'os', header: '操作系统' },
    { accessorKey: 'browser', header: '浏览器' },
    { accessorKey: 'loginTime', header: '登录时间' },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    // 后端不分页、恒返回全量,故此处前端切片(与 Vue 同策略)。
    const resp = await onlineApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    const all = resp.rows ?? []
    return {
      rows: slicePage(all, params.pageNo, params.pageSize),
      totalRowCount: all.length,
    }
  }

  const crudTable = useCrudTable<UserOnline>({
    columns,
    // 本页后端不分页也不排序(OnlineUserQuery 只有 userName/ipaddr,
    // online/handler.rs:14-19),排序无从转发 —— 保持空集,表头不长排序控件。
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({ userName: filters.userName, ipaddr: filters.ipaddr }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = { userName: model.userName, ipaddr: model.ipaddr }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- force logout confirm ----
  let forceOpen = $state(false)
  let forcing = $state(false)
  let forceTarget = $state<UserOnline | null>(null)

  function openForce(row: UserOnline) {
    forceTarget = row
    forceOpen = true
  }

  async function handleForce() {
    if (!forceTarget) return
    forcing = true
    try {
      await onlineApi.forceLogout(forceTarget.tokenId)
      toast.success('强退成功')
      forceOpen = false
      forceTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      forcing = false
    }
  }
</script>


<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    title="在线用户"
    emptyMessage="未找到在线用户数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    customCellIds={customCellIds}>
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as UserOnline}
      {#if columnId === 'deviceType'}
        <DictTag dictType="sys_device_type" value={r.deviceType ?? ''} />
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="monitor:online:forceLogout">
            <Button
              variant="ghost"
              size="icon"
              class="text-destructive hover:text-destructive"
              title="强退"
              aria-label="强退"
            onclick={() => openForce(r)}>
            <LogOut class="size-4" />
          </Button>
        </HasPermi>
      </div>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<ConfirmDialog
  open={forceOpen}
  onOpenChange={(v) => (forceOpen = v)}
  title="确认强退"
  desc="确定强退该用户?"
  confirmText="强退"
  destructive
  isLoading={forcing}
  handleConfirm={handleForce} />
