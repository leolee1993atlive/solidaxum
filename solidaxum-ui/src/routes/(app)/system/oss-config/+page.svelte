<script lang="ts">
  import { ossConfigApi } from '$lib/api/system/oss-config.js'
  import type {
    OssConfigVo,
    OssConfigCreateBo,
    OssConfigUpdateBo,
  } from '$lib/api/system/oss-config.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import StatusSwitchCell from '$lib/components/StatusSwitchCell.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { DICT_TAG_CLASS } from '$lib/components/dict-tag.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { Plus, Pencil, Trash2 } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 筛选(桶名称 + 是否默认 + configKey) ----
  let filters = $state<{
    configKey?: string
    bucketName?: string
    status?: string
  }>({})

  // 「是否默认」的字典是 sys_yes_no(Y/N),不是 sys_normal_disable(0/1) —— 接错字典值域全错。
  const queryFields = [
    { key: 'configKey', label: '配置key', placeholder: '请输入配置key' },
    { key: 'bucketName', label: '桶名称', placeholder: '请输入桶名称' },
    {
      key: 'status',
      label: '是否默认',
      type: 'select' as const,
      dictType: 'sys_yes_no',
    },
  ]

  // SELECT_CLASS 与 user/role 页一致:统一原生 select 外观,匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // accessPolicy 的彩色 tag 配色 —— 无对应字典(Vue 硬编码 el-tag,config.vue:101-105),
  // 直接用 DICT_TAG_CLASS 常量,不另起一套颜色(与流程实例页的激活/挂起 tag 一致)。
  // 值域是 '0'/'1'/'2'(private/public/custom),不是字面字符串。
  const ACCESS_POLICY_OPTIONS = [
    { label: 'private', value: '0' },
    { label: 'public', value: '1' },
    { label: 'custom', value: '2' },
  ]
  const ACCESS_POLICY_CLASS: Record<string, string> = {
    '0': DICT_TAG_CLASS.warning, // private → warning(橙)
    '1': DICT_TAG_CLASS.success, // public → success(绿)
    '2': DICT_TAG_CLASS.info, // custom → info(灰)
  }
  const ACCESS_POLICY_LABEL: Record<string, string> = {
    '0': 'private',
    '1': 'public',
    '2': 'custom',
  }

  // ---- columns ----
  // 不做「主键」(ossConfigId) 列 —— Vue 用 v-if="columns[0].visible" 默认隐藏(config.vue:86)。
  // 含「前缀 prefix」「域 region」「是否HTTPS isHttps」三列(Vue 都有)。
  // accessPolicy 走 cellRenderer 渲染彩色 tag + 列表有该列。
  const customCellIds = new Set(['status', 'accessPolicy', 'isHttps'])
  const columns: ColumnDef<OssConfigVo>[] = [
    { accessorKey: 'configKey', header: '配置key' },
    { accessorKey: 'endpoint', header: '访问站点' },
    {
      accessorKey: 'domainUrl',
      header: '自定义域名',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { accessorKey: 'bucketName', header: '桶名称' },
    {
      accessorKey: 'prefix',
      header: '前缀',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'region',
      header: '域',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { accessorKey: 'accessPolicy', header: '桶权限类型', enableSorting: false },
    { accessorKey: 'isHttps', header: '是否HTTPS', enableSorting: false },
    { accessorKey: 'status', header: '是否默认', enableSorting: false },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await ossConfigApi.list(
      {
        pageNum: params.pageNo,
        pageSize: params.pageSize,
        // 排序必须真发出去:后端 build_order_by(page.rs:87)按驼峰键白名单映射到列名
        // (oss_service.rs:215-219 收 ossConfigId/configKey/createTime),不发就是点了没反应。
        orderByColumn: params.sort,
        isAsc: params.order,
      },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<OssConfigVo>({
    columns,
    // 「主键」列不做,ossConfigId 不是可见列 —— 留在 sortableIds 里是死配置。
    sortableIds: new Set(['configKey']),
    fetchPage,
    getExtraParams: () => ({
      configKey: filters.configKey,
      bucketName: filters.bucketName,
      status: filters.status,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      configKey: model.configKey,
      bucketName: model.bucketName,
      status: model.status,
    }
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

  interface OssConfigFormModel {
    configKey: string
    accessKey: string
    secretKey: string
    bucketName: string
    prefix: string
    endpoint: string
    domainUrl: string
    isHttps: string
    region: string
    accessPolicy: string
    remark: string
  }

  function emptyForm(): OssConfigFormModel {
    return {
      configKey: '',
      accessKey: '',
      secretKey: '',
      bucketName: '',
      prefix: '',
      endpoint: '',
      domainUrl: '',
      isHttps: 'N',
      region: '',
      accessPolicy: '1',
      remark: '',
    }
  }

  let form = $state<OssConfigFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: OssConfigVo) {
    editingId = row.ossConfigId
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)。
    // secretKey 在 GET 时返回明文,直接预填。
    form = {
      configKey: row.configKey ?? '',
      accessKey: row.accessKey ?? '',
      secretKey: row.secretKey ?? '',
      bucketName: row.bucketName ?? '',
      prefix: row.prefix ?? '',
      endpoint: row.endpoint ?? '',
      domainUrl: row.domainUrl ?? '',
      isHttps: row.isHttps ?? 'N',
      region: row.region ?? '',
      accessPolicy: row.accessPolicy ?? '',
      remark: row.remark ?? '',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    // accessKey / secretKey / accessPolicy 必填(后端有 #[validate] 兜底,本条只是体验降级)。
    if (!form.configKey || !form.bucketName || !form.endpoint) {
      toast.error('配置key、桶名称、域名(endpoint)不能为空')
      return
    }
    if (!form.accessKey || !form.secretKey || form.accessPolicy === '') {
      toast.error('accessKey、secretKey、权限策略不能为空')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: OssConfigUpdateBo = {
          ossConfigId: editingId,
          configKey: form.configKey,
          accessKey: form.accessKey,
          secretKey: form.secretKey,
          bucketName: form.bucketName,
          prefix: form.prefix || undefined,
          endpoint: form.endpoint,
          domainUrl: form.domainUrl || undefined,
          isHttps: form.isHttps || undefined,
          region: form.region || undefined,
          accessPolicy: form.accessPolicy || undefined,
          remark: form.remark || undefined,
        }
        await ossConfigApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: OssConfigCreateBo = {
          configKey: form.configKey,
          accessKey: form.accessKey,
          secretKey: form.secretKey,
          bucketName: form.bucketName,
          prefix: form.prefix || undefined,
          endpoint: form.endpoint,
          domainUrl: form.domainUrl || undefined,
          isHttps: form.isHttps || undefined,
          region: form.region || undefined,
          accessPolicy: form.accessPolicy || undefined,
          remark: form.remark || undefined,
        }
        await ossConfigApi.create(payload)
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
  let deleteTarget = $state<OssConfigVo | null>(null)

  function openDelete(row: OssConfigVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await ossConfigApi.delete(deleteTarget.ossConfigId)
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
    title="配置列表"
    emptyMessage="未找到对象存储配置数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
  >
    {#snippet toolbar()}
      <HasPermi perm="system:ossConfig:add">
        <Button size="sm" onclick={openCreate}>
          <Plus class="mr-1 h-4 w-4" />新增
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
      {@const r = row as OssConfigVo}
      {#if columnId === 'accessPolicy'}
        {@const v = r.accessPolicy ?? ''}
        {@const cls = ACCESS_POLICY_CLASS[v]}
        {@const label = ACCESS_POLICY_LABEL[v]}
        {#if label}
          <Badge variant="outline" class={cls}>{label}</Badge>
        {:else}
          <span class="text-muted-foreground">-</span>
        {/if}
      {:else if columnId === 'isHttps'}
        <DictTag dictType="sys_yes_no" value={r.isHttps ?? ''} />
      {:else if columnId === 'status'}
        <HasPermi perm="system:ossConfig:edit">
          <StatusSwitchCell
            value={r.status}
            checkedValue="Y"
            uncheckedValue="N"
            label={`是否将 ${r.configKey} 设为默认桶`}
            confirmText={`确认要「${r.status === 'Y' ? '取消默认' : '设为默认'}」配置「${r.configKey}」吗?`}
            successMessage="操作成功"
            onConfirm={(next) => ossConfigApi.changeStatus(r.ossConfigId, next)}
            onDone={() => crudTable.reload()}
          />
        </HasPermi>
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="system:ossConfig:edit">
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
          <HasPermi perm="system:ossConfig:remove">
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
  title={isEdit ? '编辑对象存储配置' : '新增对象存储配置'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="configKey">配置key</Label>
    <Input
      id="configKey"
      bind:value={form.configKey}
      placeholder="请输入配置key"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="accessKey">accessKey</Label>
    <Input
      id="accessKey"
      bind:value={form.accessKey}
      placeholder="请输入accessKey"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="secretKey">secretKey</Label>
    <Input
      id="secretKey"
      type="password"
      bind:value={form.secretKey}
      placeholder="请输入secretKey"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="bucketName">桶名称</Label>
    <Input
      id="bucketName"
      bind:value={form.bucketName}
      placeholder="请输入桶名称"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="prefix">前缀</Label>
    <Input
      id="prefix"
      bind:value={form.prefix}
      placeholder="请输入前缀(可选)"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="endpoint">域名(endpoint)</Label>
    <Input
      id="endpoint"
      bind:value={form.endpoint}
      placeholder="请输入endpoint"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="domainUrl">自定义域名</Label>
    <Input
      id="domainUrl"
      bind:value={form.domainUrl}
      placeholder="请输入自定义域名(可选)"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="isHttps">是否HTTPS</Label>
    <select id="isHttps" class={SELECT_CLASS} bind:value={form.isHttps}>
      <option value="Y">是</option>
      <option value="N">否</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="region">区域</Label>
    <Input
      id="region"
      bind:value={form.region}
      placeholder="请输入区域(可选)"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="accessPolicy">权限策略</Label>
    <select
      id="accessPolicy"
      class={SELECT_CLASS}
      bind:value={form.accessPolicy}
    >
      <option value="">请选择</option>
      {#each ACCESS_POLICY_OPTIONS as opt (opt.value)}
        <option value={opt.value}>{opt.label}</option>
      {/each}
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
  desc={`确定要删除对象存储配置「${deleteTarget?.configKey ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>
