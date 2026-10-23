<script lang="ts">
  import { operLogApi, prettyJson } from '$lib/api/monitor/operlog.js'
  import type { OperLog } from '$lib/api/monitor/operlog.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import { Trash2, Eraser, Eye, Download } from '@lucide/svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{
    operIp?: string
    title?: string
    operName?: string
    clientKey?: string
    deviceType?: string
    browser?: string
    os?: string
    businessType?: string
    status?: string
    'params[beginTime]'?: string
    'params[endTime]'?: string
  }>({})

  // 顺序逐字对齐 Vue index.vue:14-62。
  // operIp/clientKey/deviceType/browser/os 五项后端均支持
  // (OperLogQuery model.rs:91-96 + repo.rs:198-220 全是 contains 模糊匹配)。
  const queryFields = [
    { key: 'operIp', label: '操作地址', placeholder: '请输入操作地址' },
    { key: 'title', label: '系统模块', placeholder: '请输入系统模块' },
    { key: 'operName', label: '操作人员', placeholder: '请输入操作人员' },
    { key: 'clientKey', label: '客户端', placeholder: '请输入客户端' },
    {
      key: 'deviceType',
      label: '设备类型',
      type: 'select' as const,
      dictType: 'sys_device_type',
    },
    { key: 'browser', label: '浏览器', placeholder: '请输入浏览器' },
    { key: 'os', label: '操作系统', placeholder: '请输入操作系统' },
    {
      key: 'businessType',
      label: '类型',
      type: 'select' as const,
      dictType: 'sys_oper_type',
    },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_common_status',
    },
    {
      key: 'operTime',
      label: '操作时间',
      type: 'dateRange' as const,
      // 后端 #[serde(rename = "params[beginTime]")](model.rs:97-100),不是裸 beginTime。
      beginKey: 'params[beginTime]',
      endKey: 'params[endTime]',
    },
  ]

  // ---- columns: businessType / status / deviceType 走 customCellIds + DictTag;
  //      actions 列走 cellRenderer。列序逐字对齐 Vue index.vue:123-193 ----
  const customCellIds = new Set(['businessType', 'status', 'deviceType'])
  const columns: ColumnDef<OperLog>[] = [
    { accessorKey: 'operId', header: '日志编号' },
    { accessorKey: 'title', header: '系统模块' },
    { accessorKey: 'businessType', header: '操作类型' },
    { accessorKey: 'operName', header: '操作人员' },
    // ⚠️ 「部门」列:后端 oper_log.rs 中间件把 dept_name 写死成 None,
    //    真实库 sys_oper_log 里 dept_name 非空行数为 0 —— 本列当前恒显示 '-'。
    //    列按 parity 加上(中间件一补就有数据),缺口属后端待办。
    {
      accessorKey: 'deptName',
      header: '部门',
      accessorFn: (row) => row.deptName ?? '-',
    },
    {
      accessorKey: 'clientKey',
      header: '客户端',
      accessorFn: (row) => row.clientKey ?? '-',
    },
    { accessorKey: 'deviceType', header: '设备类型' },
    {
      accessorKey: 'browser',
      header: '浏览器',
      accessorFn: (row) => row.browser ?? '-',
    },
    { accessorKey: 'os', header: '操作系统', accessorFn: (row) => row.os ?? '-' },
    { accessorKey: 'operIp', header: '操作地址' },
    { accessorKey: 'status', header: '操作状态' },
    { accessorKey: 'operTime', header: '操作日期' },
    {
      accessorKey: 'costTime',
      header: '消耗时间',
      accessorFn: (row) => `${row.costTime}毫秒`,
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await operLogApi.list(
      {
        pageNum: params.pageNo,
        pageSize: params.pageSize,
        // 排序必须真发出去:后端 build_order_by(core/vo/page.rs:87)按驼峰键
        // 白名单映射列名(operlog/service.rs:39-49),不发就是点了没反应。
        orderByColumn: params.sort,
        isAsc: params.order,
      },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<OperLog>({
    columns,
    // 对齐 Vue 的三个可排序列(index.vue 的 sortable="custom"):
    // 操作人员 operName / 操作日期 operTime / 消耗时间 costTime。
    // 三个键都在后端白名单里(operlog/service.rs:39-49)。
    sortableIds: new Set(['operName', 'operTime', 'costTime']),
    fetchPage,
    getRowId: (row) => String(row.operId),
    getExtraParams: () => ({ ...filters }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      operIp: model.operIp,
      title: model.title,
      operName: model.operName,
      clientKey: model.clientKey,
      deviceType: model.deviceType,
      browser: model.browser,
      os: model.os,
      businessType: model.businessType,
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

  // ---- detail dialog ----
  let detail = $state<OperLog | null>(null)

  // ---- batch delete confirm ----
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
      await operLogApi.delete(ids.join(','))
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
      await operLogApi.clean()
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

  // ---- 导出 ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await operLogApi.export({ ...filters })
      toast.success('导出成功')
    } catch (e) {
      // ⚠️ exportBlob 绕开 apiClient 自己 fetch,抛的是裸 Error
      //    —— 全局 toast 管不到,这里不自己 toast 就是完全静默。
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
    title="操作日志"
    emptyMessage="未找到操作日志数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    selectable
    customCellIds={customCellIds}
  >
    {#snippet toolbar({ selectedCount })}
      <HasPermi perm="monitor:operlog:remove">
        <Button variant="destructive" size="sm" onclick={openClean}>
          <Eraser class="mr-1 h-4 w-4" />清空
        </Button>
      </HasPermi>
      <HasPermi perm="monitor:operlog:export">
        <Button variant="outline" size="sm" disabled={exporting} onclick={handleExport}>
          <Download class="mr-1 h-4 w-4" />导出
        </Button>
      </HasPermi>
      <HasPermi perm="monitor:operlog:remove">
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
      {#if columnId === 'businessType'}
        {@const r = row as OperLog}
        <DictTag dictType="sys_oper_type" value={r.businessType} />
      {:else if columnId === 'status'}
        {@const r = row as OperLog}
        <DictTag dictType="sys_common_status" value={r.status} />
      {:else if columnId === 'deviceType'}
        {@const r = row as OperLog}
        <DictTag dictType="sys_device_type" value={r.deviceType ?? ''} />
      {:else}
        {@const r = row as OperLog}
        <div class="flex items-center gap-1">
          <!-- 不做逐行「删除」—— Vue 行操作只有「详情」,删除仅在顶部批量。
               详情按钮补 monitor:operlog:query 守卫(Vue index.vue:184 有,
               真实库 sys_menu 已在册)。本页详情不调后端(list 已随行返回全部字段),
               故这是纯 parity 守卫,不是 403 风险。 -->
          <HasPermi perm="monitor:operlog:query">
            <Button
              variant="ghost"
              size="icon"
              title="详情"
              aria-label="详情"
              onclick={() => (detail = r)}
            >
              <Eye class="size-4" />
            </Button>
          </HasPermi>
        </div>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<ConfirmDialog
  open={cleanOpen}
  onOpenChange={(v) => (cleanOpen = v)}
  title="确认清空"
  desc="确定要清空所有操作日志吗?清空后不可恢复。"
  confirmText="清空"
  destructive
  isLoading={cleaning}
  handleConfirm={handleClean}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 条日志吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>

<Dialog.Root open={detail !== null} onOpenChange={(v) => { if (!v) detail = null }}>
  <Dialog.Content class="max-h-[85vh] overflow-y-auto sm:max-w-[720px]">
    <Dialog.Header><Dialog.Title>操作日志详情</Dialog.Title></Dialog.Header>
    {#if detail}
      <dl class="grid grid-cols-[110px_1fr] gap-x-4 gap-y-2 text-sm">
        <dt class="text-muted-foreground">操作模块</dt>
        <dd>{detail.title}（<DictTag dictType="sys_oper_type" value={detail.businessType} />）</dd>
        <dt class="text-muted-foreground">登录信息</dt>
        <dd>{detail.operName} / {detail.operIp}{detail.operLocation ? ` / ${detail.operLocation}` : ''}</dd>
        <dt class="text-muted-foreground">请求地址</dt>
        <dd class="break-all">{detail.operUrl ?? '-'}</dd>
        <dt class="text-muted-foreground">请求方式</dt>
        <dd>{detail.requestMethod}</dd>
        <dt class="text-muted-foreground">操作方法</dt>
        <dd class="break-all">{detail.method ?? '-'}</dd>
        <dt class="text-muted-foreground">操作状态</dt>
        <dd>{detail.status === 0 ? '正常' : '失败'}</dd>
        <dt class="text-muted-foreground">操作时间</dt>
        <dd>{detail.operTime ?? '-'}</dd>
        <dt class="text-muted-foreground">消耗时间</dt>
        <dd>{detail.costTime} ms</dd>
        <dt class="text-muted-foreground">请求参数</dt>
        <dd><pre class="bg-muted max-h-48 overflow-auto rounded p-2 text-xs">{prettyJson(detail.operParam)}</pre></dd>
        <dt class="text-muted-foreground">返回参数</dt>
        <dd><pre class="bg-muted max-h-48 overflow-auto rounded p-2 text-xs">{prettyJson(detail.jsonResult)}</pre></dd>
        {#if detail.status !== 0 && detail.errorMsg}
          <dt class="text-destructive">异常信息</dt>
          <dd><pre class="text-destructive max-h-48 overflow-auto rounded p-2 text-xs">{detail.errorMsg}</pre></dd>
        {/if}
      </dl>
    {/if}
  </Dialog.Content>
</Dialog.Root>
