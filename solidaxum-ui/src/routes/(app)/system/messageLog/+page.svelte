<script lang="ts">
  import {
    messageLogApi,
    type MessageLogVO,
  } from '$lib/api/system/messageLog.js'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import type { QueryField } from '$lib/components/query-form-types.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import type { ColumnDef } from '@tanstack/table-core'

  const CHANNEL_OPTIONS = [
    { label: 'SSE', value: 'SSE' },
    { label: '钉钉', value: 'DINGTALK' },
    { label: '企业微信', value: 'WECHAT' },
    { label: '飞书', value: 'FEISHU' },
    { label: '短信', value: 'SMS' },
    { label: 'APP', value: 'APP' },
  ]
  const STATUS_OPTIONS = [
    { label: '成功', value: 'SUCCESS' },
    { label: '失败', value: 'FAILED' },
    { label: '跳过', value: 'SKIPPED' },
  ]

  function labelOf(options: { label: string; value: string }[], value: string) {
    return options.find((o) => o.value === value)?.label ?? value
  }

  const queryFields: QueryField[] = [
    {
      key: 'channel',
      label: '渠道',
      type: 'select',
      options: CHANNEL_OPTIONS,
    },
    {
      key: 'status',
      label: '状态',
      type: 'select',
      options: STATUS_OPTIONS,
    },
    {
      key: 'bizType',
      label: '业务类型',
      placeholder: '如 task_reminder',
    },
    {
      key: 'createTime',
      label: '发送时间',
      type: 'dateRange',
      beginKey: 'begin',
      endKey: 'end',
    },
  ]

  interface Filters {
    channel?: string
    status?: string
    bizType?: string
    begin?: string
    end?: string
  }

  let filters = $state<Filters>({})

  const columns: ColumnDef<MessageLogVO>[] = [
    { accessorKey: 'createTime', header: '发送时间' },
    { accessorKey: 'channel', header: '渠道' },
    { accessorKey: 'bizType', header: '业务类型' },
    { accessorKey: 'bizId', header: '业务ID' },
    { accessorKey: 'status', header: '状态' },
    { accessorKey: 'recipientUserIds', header: '收件人' },
    { accessorKey: 'content', header: '内容' },
    { accessorKey: 'errorMessage', header: '失败原因' },
  ]

  const customCellIds = new Set(['channel', 'status', 'content'])

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await messageLogApi.list({
      pageNum: params.pageNo,
      pageSize: params.pageSize,
      ...filters,
    })
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<MessageLogVO>({
    columns,
    sortableIds: new Set(),
    fetchPage,
    getRowId: (row) => row.logId,
    getExtraParams: () => ({ ...filters }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      channel: model.channel || undefined,
      status: model.status || undefined,
      bizType: model.bizType?.trim() || undefined,
      begin: model.begin || undefined,
      end: model.end || undefined,
    }
    crudTable.resetPagination()
  }

  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  function statusClass(status: string) {
    if (status === 'SUCCESS') return 'text-foreground font-medium'
    if (status === 'FAILED') return 'text-destructive font-medium'
    return 'text-muted-foreground'
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex flex-col gap-1">
    <div class="text-lg font-semibold tracking-tight">消息发送历史</div>
    <p class="text-muted-foreground text-sm">
      全局发送审计流水（SSE / 三 IM / 短信 / APP）；只读，不可重发。
    </p>
  </div>

  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    {customCellIds}
    title="发送记录"
    emptyMessage="暂无发送记录"
    globalFilter=""
    showSearch={false}
    showRefresh
    onRefresh={() => crudTable.reload()}
  >
    {#snippet queryForm()}
      <QueryForm
        fields={queryFields}
        onSearch={handleSearch}
        onReset={handleReset}
      />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as MessageLogVO}
      {#if columnId === 'channel'}
        <span>{labelOf(CHANNEL_OPTIONS, r.channel)}</span>
      {:else if columnId === 'status'}
        <span class={statusClass(r.status)}>
          {labelOf(STATUS_OPTIONS, r.status)}
        </span>
      {:else if columnId === 'content'}
        <span class="line-clamp-2 max-w-[360px] whitespace-pre-wrap">
          {r.content || '—'}
        </span>
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>
