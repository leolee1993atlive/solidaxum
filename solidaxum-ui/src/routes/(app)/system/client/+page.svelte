<script lang="ts">
  import {
    parseRuleText,
    toRuleText,
    resolveRuleList,
  } from '$lib/api/system/client-rules.js'
  import { clientApi } from '$lib/api/system/client.js'
  import type {
    ClientListVo,
    ClientCreateBo,
    ClientUpdateBo,
  } from '$lib/api/system/client.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import MultiSelect from '$lib/components/MultiSelect.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import StatusSwitchCell from '$lib/components/StatusSwitchCell.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
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
  import { fetchDict } from '$lib/hooks/use-dict.js'
  import { Plus, Pencil, Trash2, Download } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 筛选 (CLI6/CLI7: 补 clientSecret 文本 + status 字典下拉;clientKey 原有) ----
  let filters = $state<{
    clientKey?: string
    clientSecret?: string
    status?: string
  }>({})

  const queryFields = [
    { key: 'clientKey', label: '客户端key', placeholder: '请输入客户端key' },
    {
      key: 'clientSecret',
      label: '客户端密钥',
      placeholder: '请输入客户端密钥',
    },
    {
      key: 'status',
      label: '状态',
      type: 'select' as const,
      dictType: 'sys_normal_disable',
    },
  ]

  // 授权类型选项走字典 sys_grant_type(password/sms/email/xcx/social,ry_vue.sql 实测)。
  // CLI1:旧实现是硬编码 password/refresh_token 单选 + 自由文本逗号手录,refresh_token 根本
  // 不在 sys_grant_type 字典里,且 grantType 是真被执行的字段(auth_handler.rs:59 校验登录)。
  let grantTypeOptions = $state<{ label: string; value: string }[]>([])
  $effect(() => {
    fetchDict('sys_grant_type').then((list) => {
      grantTypeOptions = list.map((d) => ({
        label: d.dictLabel,
        value: d.dictValue,
      }))
    })
  })

  // CLI10:设备类型走字典 sys_device_type(pc/android/ios/xcx,ry_vue.sql 实测),旧实现是自由文本。
  let deviceTypeOptions = $state<{ label: string; value: string }[]>([])
  $effect(() => {
    fetchDict('sys_device_type').then((list) => {
      deviceTypeOptions = list.map((d) => ({
        label: d.dictLabel,
        value: d.dictValue,
      }))
    })
  })

  // SELECT_CLASS 与 user/role 页一致:统一原生 select 外观,匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // ---- columns: 普通列用 accessorFn 返回格式化文本;actions 列走 cellRenderer ----
  // status 列改走 cellRenderer 渲染 StatusSwitchCell(即时切换启用/停用,唤醒 changeStatus 死代码)。
  // accessPath / ipWhitelist 走 cellRenderer 渲染 tag 列表(空态显示「全部路径」/「全部IP」)。
  const customCellIds = new Set(['status', 'accessPath', 'ipWhitelist'])
  const columns: ColumnDef<ClientListVo>[] = [
    { accessorKey: 'id', header: 'id' },
    { accessorKey: 'clientId', header: '客户端编号' },
    { accessorKey: 'clientKey', header: '客户端key' },
    { accessorKey: 'grantType', header: '授权类型' },
    { accessorKey: 'accessPath', header: '允许访问路径', enableSorting: false },
    { accessorKey: 'ipWhitelist', header: 'IP白名单', enableSorting: false },
    { accessorKey: 'activeTimeout', header: '活跃超时(秒)' },
    { accessorKey: 'timeout', header: '固定超时(秒)' },
    { accessorKey: 'status', header: '状态', enableSorting: false },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    sort?: string
    order?: 'asc' | 'desc'
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await clientApi.list(
      {
        pageNum: params.pageNo,
        pageSize: params.pageSize,
        // 排序必须真发出去:后端 build_order_by(page.rs:87)按驼峰键白名单映射到列名
        // (client_service.rs:65-70 收 id/clientId/clientKey/createTime),不发就是点了没反应。
        orderByColumn: params.sort,
        isAsc: params.order,
      },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<ClientListVo>({
    columns,
    sortableIds: new Set(['id', 'clientKey']),
    fetchPage,
    // CLI12:getRowId 必须配,否则 useCrudTable 的 enableRowSelection=false,勾选框不工作。
    // 用 id(主键),翻页后 rowSelection 以 id 为键,不会串页。
    getRowId: (row) => String(row.id),
    getExtraParams: () => ({
      clientKey: filters.clientKey,
      clientSecret: filters.clientSecret,
      status: filters.status,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      clientKey: model.clientKey,
      clientSecret: model.clientSecret,
      status: model.status,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- CLI8 导出(form-urlencoded body,见 clientApi.export) ----
  let exporting = $state(false)

  async function handleExport() {
    exporting = true
    try {
      await clientApi.export({
        clientKey: filters.clientKey,
        clientSecret: filters.clientSecret,
        status: filters.status,
      })
      toast.success('导出成功')
    } catch (e) {
      // clientApi.export 自己 fetch(不走 apiClient),抛的是裸 Error —— 全局没有任何地方
      // 会 toast 它。这里不报,导出失败就是「导出中…」闪回一下,用户完全无感。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }

  // ---- CLI12 批量删除 ----
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
      // 后端 /system/client/{ids} 收逗号分隔的多 id(client_handler.rs:95 split(',')).
      await clientApi.delete(ids.join(','))
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

  // ---- create / edit dialog ----
  // 表单使用独立的 model 字段,避免列表行与表单类型耦合。
  // grantTypeList 是数组(MultiSelect 直接读写);白名单按换行/逗号/分号录入,提交前归一成逗号串
  // (两端分隔符不一致,见 client-rules.ts)。
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editingId = $state<string | null>(null)
  // 编辑时同时保留 clientId(PUT 需要)与 clientId 文本(用于 PUT body)
  let editingClientId = $state<string>('')
  const isEdit = $derived(editingId !== null)

  interface ClientFormModel {
    clientKey: string
    clientSecret: string
    grantTypeList: string[]
    accessPath: string
    ipWhitelist: string
    deviceType: string
    activeTimeout: number
    timeout: number
    status: string
  }

  function emptyForm(): ClientFormModel {
    return {
      clientKey: '',
      clientSecret: '',
      grantTypeList: ['password'],
      accessPath: '',
      ipWhitelist: '',
      deviceType: 'pc',
      activeTimeout: 1800,
      timeout: 604800,
      status: '0',
    }
  }

  let form = $state<ClientFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    editingClientId = ''
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: ClientListVo) {
    editingId = row.id
    editingClientId = row.clientId ?? ''
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)。
    // clientSecret 在 GET 时返回脱敏 "******",直接预填(后端在 update 时若仍为脱敏值会跳过)。
    form = {
      clientKey: row.clientKey ?? '',
      clientSecret: row.clientSecret ?? '',
      // 后端已切分好数组(ClientListVo.grantTypeList),直接用。
      grantTypeList: row.grantTypeList ?? [],
      accessPath: row.accessPath ?? '',
      ipWhitelist: row.ipWhitelist ?? '',
      deviceType: row.deviceType ?? 'pc',
      activeTimeout: row.activeTimeout ?? 1800,
      timeout: row.timeout ?? 604800,
      status: row.status ?? '0',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.clientKey) {
      toast.error('客户端key不能为空')
      return
    }
    if (!isEdit && !form.clientSecret) {
      toast.error('客户端密钥不能为空')
      return
    }
    // 🔴 授权类型必填(对齐 Vue rules.grantTypeList,index.vue:340)。
    // 两端都没有兜底:后端 ClientCreateBo/ClientUpdateBo 对 grant_type 无 #[validate],
    // from_create_bo 会把空数组 join 成 "" 落库,而 supports_grant_type("") 恒不匹配
    // (auth_handler.rs:59)—— 存成空就是一个永远登不进来的客户端,且保存时毫无提示。
    if (form.grantTypeList.length === 0) {
      toast.error('授权类型不能为空')
      return
    }
    // 设备类型必填(对齐 Vue rules.deviceType,index.vue:341)。
    if (!form.deviceType) {
      toast.error('设备类型不能为空')
      return
    }
    // 白名单按换行/逗号/分号录入,提交前归一成逗号串(后端 split_comma_list 只认逗号)。
    const accessPath = toRuleText(parseRuleText(form.accessPath))
    const ipWhitelist = toRuleText(parseRuleText(form.ipWhitelist))
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: ClientUpdateBo = {
          id: editingId,
          clientId: editingClientId,
          clientKey: form.clientKey,
          clientSecret: form.clientSecret,
          // 只发 grantTypeList:后端 from_update_bo 在 grant_type 有值时优先用它,
          // grant_type_list 被静默忽略 —— 两个都发会让多选失效,故这里只发数组。
          grantTypeList: form.grantTypeList,
          accessPath,
          ipWhitelist,
          deviceType: form.deviceType || undefined,
          activeTimeout: form.activeTimeout,
          timeout: form.timeout,
          status: form.status,
        }
        await clientApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: ClientCreateBo = {
          clientKey: form.clientKey,
          clientSecret: form.clientSecret,
          grantTypeList: form.grantTypeList,
          accessPath,
          ipWhitelist,
          deviceType: form.deviceType || undefined,
          activeTimeout: form.activeTimeout,
          timeout: form.timeout,
          status: form.status,
        }
        await clientApi.create(payload)
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
  let deleteTarget = $state<ClientListVo | null>(null)

  function openDelete(row: ClientListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await clientApi.delete(deleteTarget.id)
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
    emptyMessage="未找到客户端数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    selectable
    title="客户端列表"
  >
    {#snippet toolbar({ selectedCount })}
      <HasPermi perm="system:client:export">
        <Button
          variant="outline"
          size="sm"
          onclick={handleExport}
          disabled={exporting}
        >
          <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
        </Button>
      </HasPermi>
      <HasPermi perm="system:client:add">
        <Button size="sm" onclick={openCreate}
          ><Plus class="mr-1 h-4 w-4" />新增</Button
        >
      </HasPermi>
      <HasPermi perm="system:client:remove">
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
      {@const r = row as ClientListVo}
      {#if columnId === 'accessPath'}
        {@const rules = resolveRuleList(r.accessPathList, r.accessPath)}
        {#if rules.length}
          <div class="flex flex-wrap gap-1">
            {#each rules as p}<Badge variant="outline">{p}</Badge>{/each}
          </div>
        {:else}
          <span class="text-muted-foreground">全部路径</span>
        {/if}
      {:else if columnId === 'ipWhitelist'}
        {@const rules = resolveRuleList(r.ipWhitelistList, r.ipWhitelist)}
        {#if rules.length}
          <div class="flex flex-wrap gap-1">
            {#each rules as p}<Badge variant="outline">{p}</Badge>{/each}
          </div>
        {:else}
          <span class="text-muted-foreground">全部IP</span>
        {/if}
      {:else if columnId === 'status'}
        <HasPermi perm="system:client:edit">
          <StatusSwitchCell
            value={r.status}
            checkedValue="0"
            uncheckedValue="1"
            label={`${r.status === '0' ? '停用' : '启用'}客户端 ${r.clientKey}`}
            confirmText={`确认要「${r.status === '0' ? '停用' : '启用'}」客户端「${r.clientKey}」吗?`}
            successMessage="操作成功"
            onConfirm={(next) => clientApi.changeStatus(r.clientId, next)}
            onDone={() => crudTable.reload()}
          />
        </HasPermi>
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="system:client:edit">
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
          <HasPermi perm="system:client:remove">
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
  title={isEdit ? '编辑客户端' : '新增客户端'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="clientKey">客户端key</Label>
    <Input
      id="clientKey"
      bind:value={form.clientKey}
      disabled={isEdit}
      placeholder="请输入客户端key"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="clientSecret">客户端密钥</Label>
    <Input
      id="clientSecret"
      type="password"
      bind:value={form.clientSecret}
      disabled={isEdit}
      placeholder={isEdit ? '不修改请保持原值' : '请输入客户端密钥'}
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="grantType">授权类型</Label>
    <MultiSelect
      options={grantTypeOptions}
      values={form.grantTypeList}
      placeholder="请选择授权类型"
      onChange={(next) => (form.grantTypeList = next)}
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="accessPath">
      允许访问路径
      <span class="text-muted-foreground text-xs">
        （多个路径可按换行、逗号或分号分隔；为空表示允许访问所有接口路径）
      </span>
    </Label>
    <Textarea
      id="accessPath"
      bind:value={form.accessPath}
      rows={4}
      placeholder="示例：/app/**"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="ipWhitelist">
      IP白名单
      <span class="text-muted-foreground text-xs">
        （支持精确IP、通配符和CIDR；多个规则可按换行、逗号或分号分隔；为空表示允许所有IP）
      </span>
    </Label>
    <Textarea
      id="ipWhitelist"
      bind:value={form.ipWhitelist}
      rows={4}
      placeholder={'示例：127.0.0.1\n192.168.*.*\n10.0.0.0/24'}
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="deviceType">设备类型</Label>
    <select id="deviceType" class={SELECT_CLASS} bind:value={form.deviceType}>
      <option value="">请选择</option>
      {#each deviceTypeOptions as opt (opt.value)}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="activeTimeout">
      Token活跃超时时间(秒)
      <span class="text-muted-foreground text-xs">
        （指定时间无操作则过期（单位：秒），默认30分钟（1800秒））
      </span>
    </Label>
    <Input
      id="activeTimeout"
      type="number"
      bind:value={form.activeTimeout}
      placeholder="请输入Token活跃超时时间"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="timeout">
      Token固定超时时间(秒)
      <span class="text-muted-foreground text-xs">
        （指定时间必定过期（单位：秒），默认七天（604800秒））
      </span>
    </Label>
    <Input
      id="timeout"
      type="number"
      bind:value={form.timeout}
      placeholder="请输入Token固定超时时间"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" class={SELECT_CLASS} bind:value={form.status}>
      <option value="0">正常</option>
      <option value="1">停用</option>
    </select>
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除客户端「${deleteTarget?.clientKey ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 个客户端吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>
