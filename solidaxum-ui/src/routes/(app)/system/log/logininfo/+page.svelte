<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'

  import { loginInfoApi } from '$lib/api/monitor/logininfo.js'
  import type { LoginInfo } from '$lib/api/monitor/logininfo.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
        import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Trash2, Eraser, LockOpen, Download } from '@lucide/svelte'

  // ---- 多字段筛选 (RuoYi 风格),顺序对齐 Vue index.vue:14-40 ----
  let filters = $state<{
    ipaddr?: string
    userName?: string
    status?: string
    'params[beginTime]'?: string
    'params[endTime]'?: string
  }>({})

  const queryFields = [
    { key: 'ipaddr', label: '登录地址', placeholder: '请输入登录地址' },
    { key: 'userName', label: '用户名称', placeholder: '请输入用户名称' },
    // 后端 LoginInfoQuery.status 一直支持(model.rs:69 + repo filter)。
    // 标签用「状态」—— Vue 的 el-form-item label 就是「状态」(index.vue:25),
    // 「登录状态」是它的 placeholder。
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_common_status',
    },
    // 后端 #[serde(rename = "params[beginTime]")](model.rs:71-74)。
    // ⚠️ 必须补足时分秒 —— 不补的话 logininfo/repo.rs:158 解析失败,
    //    整条筛选被静默跳过。
    {
      key: 'loginTime',
      label: '登录时间',
      type: 'dateRange' as const,
      beginKey: 'params[beginTime]',
      endKey: 'params[endTime]',
    },
  ]

  // ---- columns: status / deviceType 走 customCellIds + DictTag。
  //      列序逐字对齐 Vue index.vue:105-143;Vue 该页表格无「操作」列 ----
  const customCellIds = new Set(['status', 'deviceType'])
  const columns: ColumnDef<LoginInfo>[] = [
    { accessorKey: 'infoId', header: '访问编号' },
    { accessorKey: 'userName', header: '用户名称' },
    // 后端 LoginInfo.client_key 一直返回(model.rs:10),真实库 47 行中 30 行有值。
    {
      accessorKey: 'clientKey',
      header: '客户端',
      accessorFn: (row) => row.clientKey ?? '-',
    },
    // sys_device_type 字典,真实库 4 行(pc/android/ios/xcx)。
    { accessorKey: 'deviceType', header: '设备类型' },
    { accessorKey: 'ipaddr', header: '地址' },
    {
      accessorKey: 'loginLocation',
      header: '登录地点',
      accessorFn: (row) => row.loginLocation ?? '-',
    },
    // Vue os 在 browser 前。
    { accessorKey: 'os', header: '操作系统' },
    { accessorKey: 'browser', header: '浏览器' },
    { accessorKey: 'status', header: '登录状态' },
    {
      accessorKey: 'msg',
      header: '描述',
      accessorFn: (row) => row.msg ?? '-',
    },
    { accessorKey: 'loginTime', header: '访问时间' },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await loginInfoApi.list(
      {
        pageNum: params.pageNo,
        pageSize: params.pageSize,
        // 排序必须真发出去:后端白名单见 logininfo/service.rs:36-42。
        orderByColumn: params.sort,
        isAsc: params.order,
      },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<LoginInfo>({
    columns,
    // 对齐 Vue 的两个可排序列(index.vue 的 sortable="custom"):
    // 用户名称 userName / 访问时间 loginTime。两键都在后端白名单里。
    sortableIds: new Set(['userName', 'loginTime']),
    fetchPage,
    // 🔴 selectable 必须配 getRowId —— enableRowSelection 由 Boolean(getRowId) 决定。
    // 缺了它勾选框永远勾不上,「解锁」按钮(disabled={singleSelected})恒禁用。
    getRowId: (row) => String(row.infoId),
    getExtraParams: () => ({ ...filters }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      ipaddr: model.ipaddr,
      userName: model.userName,
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

  // ---- 批量删除(Vue 顶部「删除」按钮依赖行勾选,index.vue:57-66) ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  async function handleBatchDelete() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchDeleting = true
    try {
      await loginInfoApi.delete(ids.join(','))
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

  // ---- clean all confirm ----
  let cleanOpen = $state(false)
  let cleaning = $state(false)

  function openClean() {
    cleanOpen = true
  }

  async function handleClean() {
    cleaning = true
    try {
      await loginInfoApi.clean()
      toast.success('清空成功')
      cleanOpen = false
      crudTable.resetPagination()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      cleaning = false
    }
  }

  const selectedRows = $derived(crudTable.table.getSelectedRowModel().rows.map((r) => r.original))
  const singleSelected = $derived(selectedRows.length !== 1)
  let unlockOpen = $state(false)
  async function doUnlock() {
    const row = selectedRows[0]
    if (!row) return
    await loginInfoApi.unlock(String(row.userName))
    toast.success(`已解锁账号【${row.userName}】`)
    unlockOpen = false
    crudTable.table.resetRowSelection()
  }

  // ---- 导出 ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await loginInfoApi.export({ ...filters })
      toast.success('导出成功')
    } catch (e) {
      // ⚠️ exportBlob 绕开 apiClient 自己 fetch,抛裸 Error —— 不自己 toast 就是静默。
      console.error(e)
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }
</script>


<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    title="登录日志"
    emptyMessage="未找到登录日志数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    selectable
    customCellIds={customCellIds}>
    {#snippet toolbar({ selectedCount })}
      <HasPermi perm="monitor:logininfo:remove">
        <Button variant="destructive" size="sm" onclick={openClean}>
          <Eraser class="mr-1 h-4 w-4" />清空
        </Button>
      </HasPermi>
      <HasPermi perm="monitor:logininfo:export">
        <Button variant="outline" size="sm" disabled={exporting} onclick={handleExport}>
          <Download class="mr-1 h-4 w-4" />导出
        </Button>
      </HasPermi>
      <HasPermi perm="monitor:logininfo:unlock">
        <Button size="sm" variant="secondary" disabled={singleSelected} onclick={() => (unlockOpen = true)}>
          <LockOpen class="mr-1 h-4 w-4" />解锁
        </Button>
      </HasPermi>
      <HasPermi perm="monitor:logininfo:remove">
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
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as LoginInfo}
      {#if columnId === 'deviceType'}
        <DictTag dictType="sys_device_type" value={r.deviceType ?? ''} />
      {:else}
        <DictTag dictType="sys_common_status" value={r.status} />
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 条登录日志吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete} />

<ConfirmDialog
  open={cleanOpen}
  onOpenChange={(v) => (cleanOpen = v)}
  title="确认清空"
  desc="确定要清空所有登录日志吗?清空后不可恢复。"
  confirmText="清空"
  destructive
  isLoading={cleaning}
  handleConfirm={handleClean} />

<ConfirmDialog
  open={unlockOpen}
  onOpenChange={(v) => (unlockOpen = v)}
  title="确认解锁"
  desc={`是否确认解锁账号【${selectedRows[0]?.userName ?? ''}】？`}
  handleConfirm={doUnlock} />
