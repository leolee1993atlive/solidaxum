<script lang="ts">
  import { jobApi } from '$lib/api/job.js'
  import {
    jobStatusLabel,
    jobConcurrentLabel,
    DEFAULT_JOB_STATUS,
  } from '$lib/api/job.js'
  import type { SysJob } from '$lib/api/job.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { Plus, Pencil, Trash2, Play } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{ job_name?: string; job_group?: string }>({})

  const queryFields = [
    { key: 'job_name', label: '任务名称', placeholder: '请输入任务名称' },
    { key: 'job_group', label: '任务组名', placeholder: '请输入任务组名' },
  ]

  // SELECT_CLASS 与 role 页一致: 统一原生 select 外观, 匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // ---- dict label helpers ----
  // CrudTableShell 非操作列直接渲染 cell.getValue(), 故 misfire_policy/concurrent/status 用 accessorFn 文本映射,
  // 与 role 页 (status/dataScope -> 中文) 做法一致。
  function misfirePolicyLabel(v?: number): string {
    switch (v) {
      case 1:
        return '立即执行'
      case 2:
        return '执行一次'
      case 3:
        return '放弃执行'
      default:
        return '-'
    }
  }
  // ---- columns: accessorKey 一律 snake_case, 与实体字段对齐;actions 列走 cellRenderer ----
  const columns: ColumnDef<SysJob>[] = [
    { accessorKey: 'job_id', header: '任务编号' },
    { accessorKey: 'job_name', header: '任务名称' },
    { accessorKey: 'job_group', header: '任务组名' },
    { accessorKey: 'invoke_target', header: '调用目标' },
    { accessorKey: 'cron_expression', header: 'cron 表达式' },
    {
      accessorKey: 'misfire_policy',
      header: '执行策略',
      accessorFn: (row) => misfirePolicyLabel(row.misfire_policy),
    },
    {
      accessorKey: 'concurrent',
      header: '并发执行',
      accessorFn: (row) => jobConcurrentLabel(row.concurrent),
    },
    {
      accessorKey: 'status',
      header: '状态',
      accessorFn: (row) => jobStatusLabel(row.status),
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await jobApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<SysJob>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({
      job_name: filters.job_name,
      job_group: filters.job_group,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = { job_name: model.job_name, job_group: model.job_group }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- create / edit dialog ----
  // 表单使用独立的 model 字段 (snake_case, 与实体一致), 避免列表行与表单类型耦合。
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  const isEdit = $derived(editingId !== null)

  interface JobFormModel {
    job_name: string
    job_group: string
    invoke_target: string
    cron_expression: string
    job_params: string
    misfire_policy: number
    concurrent: number
    status: string
    remark: string
  }

  function emptyForm(): JobFormModel {
    return {
      job_name: '',
      job_group: 'DEFAULT',
      invoke_target: '',
      cron_expression: '',
      job_params: '',
      misfire_policy: 1,
      concurrent: 1,
      status: DEFAULT_JOB_STATUS,
      remark: '',
    }
  }

  let form = $state<JobFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: SysJob) {
    editingId = row.job_id
    // 列表行字段与详情一致, 直接用行数据预填 (避免额外网络往返)
    form = {
      job_name: row.job_name ?? '',
      job_group: row.job_group ?? 'DEFAULT',
      invoke_target: row.invoke_target ?? '',
      cron_expression: row.cron_expression ?? '',
      job_params: row.job_params ?? '',
      misfire_policy: row.misfire_policy ?? 1,
      concurrent: row.concurrent ?? 1,
      status: row.status ?? '0',
      remark: row.remark ?? '',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.job_name || !form.invoke_target || !form.cron_expression) {
      toast.error('任务名称、调用目标、cron 表达式不能为空')
      return
    }
    saving = true
    try {
      const payload: Partial<SysJob> = {
        job_name: form.job_name,
        job_group: form.job_group,
        invoke_target: form.invoke_target,
        cron_expression: form.cron_expression,
        job_params: form.job_params || undefined,
        misfire_policy: form.misfire_policy,
        concurrent: form.concurrent,
        status: form.status,
        remark: form.remark || undefined,
      }
      if (isEdit && editingId !== null) {
        payload.job_id = editingId
        await jobApi.update(payload)
        toast.success('修改成功')
      } else {
        await jobApi.create(payload)
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
  let deleteTarget = $state<SysJob | null>(null)

  function openDelete(row: SysJob) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await jobApi.delete(deleteTarget.job_id)
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

  // ---- run once confirm ----
  let runOpen = $state(false)
  let running = $state(false)
  let runTarget = $state<SysJob | null>(null)

  function openRun(row: SysJob) {
    runTarget = row
    runOpen = true
  }

  async function handleRun() {
    if (!runTarget) return
    running = true
    try {
      await jobApi.run(runTarget.job_id)
      toast.success('执行成功')
      runOpen = false
      runTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      running = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage="未找到定时任务数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    title="任务列表"
  >
    {#snippet toolbar()}
      <HasPermi perm="monitor:job:add">
        <Button size="sm" onclick={openCreate}
          ><Plus class="mr-1 h-4 w-4" />新增</Button
        >
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
      {@const r = row as SysJob}
      <div class="flex items-center gap-1">
        <HasPermi perm="monitor:job:edit">
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
        <HasPermi perm="monitor:job:changeStatus">
          <Button
            variant="ghost"
            size="icon"
            title="执行一次"
            aria-label="执行一次"
            onclick={() => openRun(r)}
          >
            <Play class="size-4" />
          </Button>
        </HasPermi>
        <HasPermi perm="monitor:job:remove">
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
      </div>
    {/snippet}
  </CrudTableShell>
</Main>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑任务' : '新增任务'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="job_name">任务名称</Label>
    <Input
      id="job_name"
      bind:value={form.job_name}
      placeholder="请输入任务名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="job_group">任务组名</Label>
    <select id="job_group" class={SELECT_CLASS} bind:value={form.job_group}>
      <option value="DEFAULT">DEFAULT</option>
      <option value="其他">其他</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="invoke_target">调用目标</Label>
    <Input
      id="invoke_target"
      bind:value={form.invoke_target}
      placeholder="如 task.demo.run()"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="cron_expression">cron 表达式</Label>
    <Input
      id="cron_expression"
      bind:value={form.cron_expression}
      placeholder="如 0/5 * * * * ?"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="job_params">任务参数</Label>
    <Input
      id="job_params"
      bind:value={form.job_params}
      placeholder="可选, 如 id=1"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="misfire_policy">执行策略</Label>
    <select
      id="misfire_policy"
      class={SELECT_CLASS}
      bind:value={form.misfire_policy}
    >
      <option value={1}>立即执行</option>
      <option value={2}>执行一次</option>
      <option value={3}>放弃执行</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="concurrent">并发执行</Label>
    <select id="concurrent" class={SELECT_CLASS} bind:value={form.concurrent}>
      <option value={0}>禁止</option>
      <option value={1}>允许</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" class={SELECT_CLASS} bind:value={form.status}>
      <option value="1">正常</option>
      <option value="0">暂停</option>
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
  desc={`确定要删除任务「${deleteTarget?.job_name ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={runOpen}
  onOpenChange={(v) => (runOpen = v)}
  title="确认执行"
  desc={`确定要立即执行任务「${runTarget?.job_name ?? ''}」一次吗?`}
  confirmText="执行"
  isLoading={running}
  handleConfirm={handleRun}
/>
