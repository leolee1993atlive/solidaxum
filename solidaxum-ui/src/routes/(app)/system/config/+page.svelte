<script lang="ts">
  import { configApi } from '$lib/api/system/config.js'
  import type {
    ConfigListVo,
    ConfigCreateBo,
    ConfigUpdateBo,
  } from '$lib/api/system/config.js'
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
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { fetchDict } from '$lib/hooks/use-dict.js'
  import { Plus, Pencil, Trash2, Download, RefreshCw } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{ configName?: string; configKey?: string }>({})

  const queryFields = [
    { key: 'configName', label: '参数名称', placeholder: '请输入参数名称' },
    { key: 'configKey', label: '参数键名', placeholder: '请输入参数键名' },
  ]

  // CFG1:Vue 用左侧竖排 el-tabs 按 configType 筛全表(index.vue:67-71)。
  // 本仓无竖排 tab 惯例,用一排按钮;值域 ''=全部 / 'Y'=系统内置 / 'N'=自定义。
  const CONFIG_TYPE_TABS = [
    { value: '', label: '全部' },
    { value: 'Y', label: '系统内置' },
    { value: 'N', label: '自定义配置' },
  ]
  let activeType = $state('')

  function switchType(v: string) {
    if (v === activeType) return
    activeType = v
    crudTable.resetPagination()
  }

  // SELECT_CLASS 与 user/role 页一致:统一原生 select 外观,匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // ---- 行内即时保存 (CFG2) ----
  // 注意:此列对所有人可见(只读);仅 system:config:edit 用户可编辑。
  // 不用 HasPermi(会整块隐藏,无权限用户连值都看不到),守卫下沉为 disabled。
  const auth = useAuth()
  const canEditConfig = $derived(auth.hasPermission('system:config:edit'))

  // CFG3:系统内置改 radio + sys_yes_no 字典。
  let yesNoOptions = $state<{ label: string; value: string }[]>([])
  $effect(() => {
    fetchDict('sys_yes_no').then((list) => {
      yesNoOptions = list.map((d) => ({
        label: d.dictLabel,
        value: d.dictValue,
      }))
    })
  })

  async function saveConfigValue(row: ConfigListVo, next: string) {
    if (next === (row.configValue ?? '')) return // 未改动不发请求
    try {
      await configApi.updateByKey(row.configKey, next)
      toast.success('保存成功')
      crudTable.reload()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '保存失败')
      crudTable.reload() // 失败也刷新,把输入框拉回真值
    }
  }

  // ---- columns: 普通列用 accessorFn 返回格式化文本;actions 列走 cellRenderer ----
  // CFG8:删掉「参数主键」列(Vue 列表无此列)。同一把尺子也适用于「系统内置」——
  // Vue 表只有 参数名称/参数键名/参数键值/备注/操作(index.vue:76-93),
  // configType 只作为表单里的 radio 存在(CFG3),列表不展示。
  const columns: ColumnDef<ConfigListVo>[] = [
    { accessorKey: 'configName', header: '参数名称' },
    { accessorKey: 'configKey', header: '参数键名' },
    { accessorKey: 'configValue', header: '参数键值', enableSorting: false },
    {
      accessorKey: 'remark',
      header: '备注',
      cell: (info) => (info.getValue() as string | undefined) || '-',
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
    const resp = await configApi.list(
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

  const customCellIds = new Set(['configValue'])

  const crudTable = useCrudTable<ConfigListVo>({
    columns,
    // 后端 config 排序白名单(config_service.rs:173):configId/configName/configKey/createTime。
    sortableIds: new Set(['configName', 'configKey']),
    fetchPage,
    getExtraParams: () => ({
      configName: filters.configName,
      configKey: filters.configKey,
      configType: activeType || undefined,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = { configName: model.configName, configKey: model.configKey }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- create / edit dialog ----
  // 表单使用独立的 model 字段,避免列表行与表单类型耦合。
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  const isEdit = $derived(editingId !== null)

  interface ConfigFormModel {
    configName: string
    configKey: string
    configValue: string
    configType: string
    remark: string
  }

  function emptyForm(): ConfigFormModel {
    return {
      configName: '',
      configKey: '',
      configValue: '',
      configType: 'N',
      remark: '',
    }
  }

  let form = $state<ConfigFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: ConfigListVo) {
    editingId = row.configId
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)
    form = {
      configName: row.configName ?? '',
      configKey: row.configKey ?? '',
      configValue: row.configValue ?? '',
      configType: row.configType ?? 'N',
      remark: row.remark ?? '',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.configName || !form.configKey) {
      toast.error('参数名称和参数键名不能为空')
      return
    }
    // CFG7:Vue rules 要求 configValue 必填(index.vue:199)。
    if (!form.configValue) {
      toast.error('参数键值不能为空')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: ConfigUpdateBo = {
          configId: editingId,
          configName: form.configName,
          configKey: form.configKey,
          configValue: form.configValue,
          configType: form.configType || undefined,
          remark: form.remark || undefined,
        }
        await configApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: ConfigCreateBo = {
          configName: form.configName,
          configKey: form.configKey,
          configValue: form.configValue,
          configType: form.configType || undefined,
          remark: form.remark || undefined,
        }
        await configApi.create(payload)
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
  let deleteTarget = $state<ConfigListVo | null>(null)

  function openDelete(row: ConfigListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await configApi.delete(deleteTarget.configId)
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

  // ---- CFG5 导出 ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await configApi.export({
        configName: filters.configName,
        configKey: filters.configKey,
        configType: activeType || undefined,
      })
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛的是裸 Error(不经 apiClient),全局没人 toast —— 必须自己报。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }

  // ---- CFG4 刷新缓存 ----
  let refreshing = $state(false)

  async function handleRefreshCache() {
    refreshing = true
    try {
      await configApi.refreshCache()
      toast.success('刷新成功')
    } catch {
      // apiClient 已 toast
    } finally {
      refreshing = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage="未找到参数数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    title="参数列表"
    {customCellIds}
  >
    {#snippet toolbar()}
      <HasPermi perm="system:config:export">
        <Button
          variant="outline"
          size="sm"
          onclick={handleExport}
          disabled={exporting}
        >
          <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
        </Button>
      </HasPermi>
      <HasPermi perm="system:config:remove">
        <Button
          variant="outline"
          size="sm"
          onclick={handleRefreshCache}
          disabled={refreshing}
        >
          <RefreshCw class="mr-1 h-4 w-4" />{refreshing ? '刷新中…' : '刷新'}
        </Button>
      </HasPermi>
      <HasPermi perm="system:config:add">
        <Button size="sm" onclick={openCreate}
          ><Plus class="mr-1 h-4 w-4" />新增</Button
        >
      </HasPermi>
    {/snippet}
    {#snippet queryForm()}
      <div class="flex gap-1">
        {#each CONFIG_TYPE_TABS as t (t.value)}
          <Button
            size="sm"
            variant={activeType === t.value ? 'default' : 'outline'}
            onclick={() => switchType(t.value)}>{t.label}</Button
          >
        {/each}
      </div>
      <QueryForm
        fields={queryFields}
        onSearch={handleSearch}
        onReset={handleReset}
      />
    {/snippet}
    {#snippet cellRenderer({ row, columnId })}
      {@const r = row as ConfigListVo}
      {#if columnId === 'configValue'}
        <Input
          class="h-8"
          value={r.configValue ?? ''}
          disabled={!canEditConfig}
          onblur={(e) =>
            saveConfigValue(r, (e.currentTarget as HTMLInputElement).value)}
          onkeydown={(e) => {
            if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur()
          }}
        />
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="system:config:edit">
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
          <HasPermi perm="system:config:remove">
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
      {/if}
    {/snippet}
  </CrudTableShell>
</Main>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑参数' : '新增参数'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="configName">参数名称</Label>
    <Input
      id="configName"
      bind:value={form.configName}
      placeholder="请输入参数名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="configKey">参数键名</Label>
    <Input
      id="configKey"
      bind:value={form.configKey}
      placeholder="请输入参数键名"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="configValue">参数键值</Label>
    <Input
      id="configValue"
      bind:value={form.configValue}
      placeholder="请输入参数键值"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label>系统内置</Label>
    <div class="flex items-center gap-4">
      {#each yesNoOptions as opt (opt.value)}
        <label class="flex items-center gap-2 text-sm">
          <input
            type="radio"
            name="configType"
            value={opt.value}
            bind:group={form.configType}
          />
          {opt.label}
        </label>
      {/each}
    </div>
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
  desc={`确定要删除参数「${deleteTarget?.configName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>
