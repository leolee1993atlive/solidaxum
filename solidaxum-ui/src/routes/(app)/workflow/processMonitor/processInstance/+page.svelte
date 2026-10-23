<script lang="ts">
  import { goto } from '$app/navigation'
  import { prettyJson } from '$lib/api/monitor/operlog.js'
  import { categoryApi } from '$lib/api/workflow/category.js'
  import type { CategoryTreeVo } from '$lib/api/workflow/category.js'
  import { buildFormUrl } from '$lib/api/workflow/flow-router.js'
  import { instanceApi } from '$lib/api/workflow/instance.js'
  import type { FlowInstanceVo } from '$lib/api/workflow/instance.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import TreePanel from '$lib/components/TreePanel.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { DICT_TAG_CLASS } from '$lib/components/dict-tag.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { Eye, Trash2 } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 左侧流程分类树(照搬 processDefinition 页的接法) ----
  let categoryTree = $state<CategoryTreeVo[]>([])
  /** 选中的分类 id;undefined = 全部(对齐 Vue handleNodeClick 里 id==='0' 归空) */
  let selectedCategory = $state<string | undefined>(undefined)

  async function loadCategoryTree() {
    categoryTree = (await categoryApi.categoryTree()).data ?? []
  }

  function handleCategoryClick(id: string | undefined) {
    // 根节点(id '0')等同「全部」
    selectedCategory = id === '0' ? undefined : id
    // selectedCategory 在 getExtraParams 里被读,改它本身就会触发重取 —— 这里要的是
    // 顺带回到第 1 页(对齐 Vue handleNodeClick → handleQuery → pageNum=1),
    // 否则在第 3 页切到一个只有 2 页的分类会停在空页上。
    crudTable.resetPagination()
  }

  // ---- 运行中/已完成双 Tab。'0' 运行中 / '1' 已完成 ----
  let activeTab = $state<'0' | '1'>('0')

  function switchTab(tab: '0' | '1') {
    if (activeTab === tab) return
    activeTab = tab
    // 切 Tab 必须重置分页(否则从第 3 页切过去会请求一个可能不存在的页码)
    crudTable.table.resetRowSelection()
    crudTable.resetPagination()
    crudTable.reload()
  }

  // ---- 列 ----
  // 序号列全站惯例不做(本仓 CrudTableShell 无 type=index),其余 label 与列序
  // 逐字对齐 Vue index.vue:85-140 —— 注意 Vue 用的是「任务名称/申请人/启动时间」,
  // 不是「当前节点/发起人/创建时间」。
  //
  // 条件列用 getter 承接:useCrudTable 的 createTable 读的是 options.columns
  // (use-crud-table.svelte.ts:221),传 getter 即可让列随 Tab / 权限变化。
  const auth = useAuth()
  // Vue 给「状态」列本身挂了 v-hasPermi=['workflow:instance:active'](index.vue:118)。
  const canSeeSuspended = $derived(
    auth.hasPermission('workflow:instance:active')
  )

  const columns = $derived.by<ColumnDef<FlowInstanceVo>[]>(() => {
    const cols: ColumnDef<FlowInstanceVo>[] = [
      {
        accessorKey: 'businessCode',
        header: '业务编码',
        accessorFn: (r) => r.businessCode ?? '-',
      },
      {
        accessorKey: 'businessTitle',
        header: '业务标题',
        accessorFn: (r) => r.businessTitle ?? '-',
      },
      // Vue 把版本号拼进流程定义名称:`{{flowName}}v{{version}}`(index.vue:99-103),
      // 同时另有独立的「版本号」列渲染 v{n}.0 —— 两处都要,不是二选一。
      {
        accessorKey: 'flowName',
        header: '流程定义名称',
        accessorFn: (r) =>
          r.flowName ? `${r.flowName}v${r.version ?? ''}` : '-',
      },
      {
        accessorKey: 'flowCode',
        header: '流程定义编码',
        accessorFn: (r) => r.flowCode ?? '-',
      },
      {
        accessorKey: 'categoryName',
        header: '流程分类',
        accessorFn: (r) => r.categoryName ?? '-',
      },
      {
        accessorKey: 'nodeName',
        header: '任务名称',
        accessorFn: (r) => r.nodeName ?? '-',
      },
      {
        accessorKey: 'createByName',
        header: '申请人',
        accessorFn: (r) => r.createByName ?? '-',
      },
      {
        accessorKey: 'version',
        header: '版本号',
        accessorFn: (r) => (r.version ? `v${r.version}.0` : '-'),
      },
    ]
    // 激活/挂起 tag(无对应字典,Vue 侧 el-tag 硬编码)。仅运行中 Tab + 有 active 权限。
    if (activeTab === '0' && canSeeSuspended) {
      cols.push({
        accessorKey: 'isSuspended',
        header: '状态',
        enableSorting: false,
      })
    }
    // 流程状态字典 tag
    cols.push({
      accessorKey: 'flowStatus',
      header: '流程状态',
      enableSorting: false,
    })
    cols.push({
      accessorKey: 'createTime',
      header: '启动时间',
      accessorFn: (r) => r.createTime ?? '-',
    })
    // 结束时间只在已完成 Tab 出现(运行中的实例本就没有 updateTime)
    if (activeTab === '1') {
      cols.push({
        accessorKey: 'updateTime',
        header: '结束时间',
        accessorFn: (r) => r.updateTime ?? '-',
      })
    }
    cols.push({
      id: 'actions',
      header: '操作',
      cell: (info) => info.row.original,
    })
    return cols
  })

  const customCellIds = new Set(['isSuspended', 'flowStatus'])

  // ---- 筛选面 ----
  let filters = $state<{
    createByIds?: string
    nodeName?: string
    flowName?: string
    flowCode?: string
  }>({})

  const queryFields = [
    { key: 'createByIds', label: '申请人', type: 'userSelect' as const },
    { key: 'nodeName', label: '任务名称', placeholder: '请输入任务名称' },
    {
      key: 'flowName',
      label: '流程定义名称',
      placeholder: '请输入流程定义名称',
    },
    {
      key: 'flowCode',
      label: '流程定义编码',
      placeholder: '请输入流程定义编码',
    },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const call =
      activeTab === '0' ? instanceApi.pageRunning : instanceApi.pageFinish
    const resp = await call(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<FlowInstanceVo>({
    // getter:列随 Tab 与权限变(状态列仅运行中+有 active 权限,结束时间列仅已完成)。
    get columns() {
      return columns
    },
    sortableIds: new Set(),
    fetchPage,
    // id 口径见 instance.ts 顶部注释 —— 实例 id 是后端 i64,本仓按 string 承载
    getRowId: (row) => String(row.id),
    getExtraParams: () => ({
      category: selectedCategory,
      createByIds: filters.createByIds,
      nodeName: filters.nodeName,
      flowName: filters.flowName,
      flowCode: filters.flowCode,
    }),
  })

  $effect(() => {
    void loadCategoryTree()
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      createByIds: model.createByIds,
      nodeName: model.nodeName,
      flowName: model.flowName,
      flowCode: model.flowCode,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 删除按实例 id(运行中 / 已完成分别走不同端点) ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<FlowInstanceVo | null>(null)

  function openDelete(row: FlowInstanceVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      const id = String(deleteTarget.id)
      if (activeTab === '0') {
        await instanceApi.deleteByInstanceIds([id])
      } else {
        await instanceApi.deleteHisByInstanceIds([id])
      }
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

  // ---- 批量删除 + selection ----
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
      if (activeTab === '0') {
        await instanceApi.deleteByInstanceIds(ids)
      } else {
        await instanceApi.deleteHisByInstanceIds(ids)
      }
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

  // ---- 作废(含作废原因,后端 comment 持久化到历史任务 message) ----
  let invalidOpen = $state(false)
  let invalidating = $state(false)
  let invalidTarget = $state<FlowInstanceVo | null>(null)
  let invalidComment = $state('')

  function openInvalid(row: FlowInstanceVo) {
    invalidTarget = row
    invalidComment = ''
    invalidOpen = true
  }

  async function handleInvalid() {
    if (!invalidTarget) return
    invalidating = true
    try {
      // 用 businessId(String)而非 instanceId —— 后端 invalid 优先按 businessId 查实例,
      // 且全站 id 契约禁止把 id 强转为数值类型(雪花 id 会截断)。
      await instanceApi.invalid({
        businessId: invalidTarget.businessId,
        comment: invalidComment || undefined,
      })
      toast.success('作废成功')
      invalidOpen = false
      invalidTarget = null
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      invalidating = false
    }
  }

  // ---- 变量弹窗(查看 JSON + 新增变量) ----
  let variableOpen = $state(false)
  let variableLoading = $state(false)
  let variableTarget = $state<FlowInstanceVo | null>(null)
  let variables = $state<Record<string, unknown>>({})
  let newVarKey = $state('')
  let newVarValue = $state('')
  let addingVar = $state(false)

  async function openVariable(row: FlowInstanceVo) {
    variableTarget = row
    variables = {}
    newVarKey = ''
    newVarValue = ''
    variableOpen = true
    variableLoading = true
    try {
      const resp = await instanceApi.instanceVariable(String(row.id))
      variables = resp.data ?? {}
    } catch {
      // apiClient 已 toast
    } finally {
      variableLoading = false
    }
  }

  async function handleAddVariable() {
    if (!variableTarget || !newVarKey.trim()) return
    addingVar = true
    try {
      let parsedValue: unknown = newVarValue
      // 值若像 JSON 则解析,否则按字符串存
      try {
        parsedValue = JSON.parse(newVarValue)
      } catch {
        // 保持字符串
      }
      const next = { ...variables, [newVarKey.trim()]: parsedValue }
      await instanceApi.updateVariable({
        instanceId: String(variableTarget.id),
        variable: next,
      })
      variables = next
      newVarKey = ''
      newVarValue = ''
      toast.success('新增变量成功')
    } catch {
      // apiClient 已 toast
    } finally {
      addingVar = false
    }
  }

  // ---- 查看(跳业务表单,用 buildFormUrl) ----
  function handleView(row: FlowInstanceVo) {
    const url = buildFormUrl({
      formPath: row.formPath,
      businessId: row.businessId,
      type: 'view',
      from: '/workflow/processMonitor/processInstance',
    })
    if (url) goto(url)
    else toast.error('该实例未配置业务表单路径')
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex gap-4">
    <TreePanel
      title="流程分类"
      placeholder="请输入流程分类名"
      nodes={categoryTree}
      selectedId={selectedCategory}
      onNodeClick={handleCategoryClick}
    />
    <div class="min-w-0 flex-1">
      <!-- 运行中/已完成双 Tab(形态照搬 processDefinition 页) -->
      <div class="mb-3 flex gap-1 border-b">
        {#each [{ v: '0', label: '运行中' }, { v: '1', label: '已完成' }] as t (t.v)}
          <button
            type="button"
            class="border-b-2 px-4 py-2 text-sm transition-colors {activeTab ===
            t.v
              ? 'border-primary text-primary-emphasis font-medium'
              : 'text-muted-foreground hover:text-foreground border-transparent'}"
            onclick={() => switchTab(t.v as '0' | '1')}
          >
            {t.label}
          </button>
        {/each}
      </div>

      <CrudTableShell
        table={crudTable.table}
        isLoading={crudTable.isLoading}
        error={crudTable.error}
        columnCount={columns.length}
        emptyMessage="未找到流程实例。"
        globalFilter=""
        showSearch={false}
        showRefresh={false}
        selectable
        title="实例列表"
        {customCellIds}
      >
        {#snippet queryForm()}
          <QueryForm
            fields={queryFields}
            onSearch={handleSearch}
            onReset={handleReset}
          />
        {/snippet}
        {#snippet toolbar({ selectedCount })}
          <HasPermi perm="workflow:instance:remove">
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
        {#snippet cellRenderer({ row, columnId })}
          {@const r = row as FlowInstanceVo}
          {#if columnId === 'isSuspended'}
            <!-- 无对应字典,直接用 DICT_TAG_CLASS 配色常量(Vue 侧 el-tag 硬编码) -->
            <Badge
              variant="outline"
              class={r.isSuspended
                ? DICT_TAG_CLASS.warning
                : DICT_TAG_CLASS.success}
            >
              {r.isSuspended ? '挂起' : '激活'}
            </Badge>
          {:else if columnId === 'flowStatus'}
            <DictTag dictType="wf_business_status" value={r.flowStatus} />
          {:else}
            <div class="flex items-center gap-1">
              <!-- 查看(跳业务表单)。守卫对齐 Vue index.vue:183。 -->
              <HasPermi perm="workflow:instance:query">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={!buildFormUrl({
                    formPath: r.formPath,
                    businessId: r.businessId,
                    type: 'view',
                    from: '/workflow/processMonitor/processInstance',
                  })}
                  title={buildFormUrl({
                    formPath: r.formPath,
                    businessId: r.businessId,
                    type: 'view',
                    from: '/workflow/processMonitor/processInstance',
                  })
                    ? '查看'
                    : '该实例未配置业务表单路径'}
                  onclick={() => handleView(r)}
                >
                  <Eye class="mr-1 h-4 w-4" />查看
                </Button>
              </HasPermi>
              <!-- 变量弹窗。打开就要发 GET instanceVariable,后端要 :variableQuery
                   (instance_handler.rs:355) —— 不加守卫则无权用户点开必吃 403。 -->
              <HasPermi perm="workflow:instance:variableQuery">
                <Button
                  variant="ghost"
                  size="sm"
                  title="变量"
                  aria-label="变量"
                  onclick={() => openVariable(r)}
                >
                  变量
                </Button>
              </HasPermi>
              <!-- 作废(仅运行中有意义,且需 businessId —— 后端 invalid 按它查实例) -->
              {#if activeTab === '0'}
                <HasPermi perm="workflow:instance:invalid">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={!r.businessId}
                    title={r.businessId ? '作废' : '该实例无业务主键,无法作废'}
                    aria-label="作废"
                    onclick={() => openInvalid(r)}
                  >
                    作废
                  </Button>
                </HasPermi>
              {/if}
              <HasPermi perm="workflow:instance:remove">
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
    </div>
  </div>
</Main>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除实例「${deleteTarget?.businessTitle ?? deleteTarget?.flowName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 条流程实例吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>

<!-- 作废弹窗:填作废原因 -->
<ConfirmDialog
  open={invalidOpen}
  onOpenChange={(o) => (invalidOpen = o)}
  title="作废流程实例"
  desc={`作废实例「${invalidTarget?.businessTitle ?? invalidTarget?.flowName ?? ''}」?作废原因将记录到审批历史。`}
  confirmText="作废"
  destructive
  handleConfirm={handleInvalid}
  isLoading={invalidating}
>
  <div class="flex flex-col gap-3">
    <Label for="invalidComment">作废原因</Label>
    <Textarea
      id="invalidComment"
      bind:value={invalidComment}
      rows={3}
      placeholder="请输入作废原因"
    />
  </div>
</ConfirmDialog>

<!-- 变量弹窗:查看 JSON + 新增变量 -->
<Dialog.Root bind:open={variableOpen}>
  <Dialog.Content class="sm:max-w-[640px]">
    <Dialog.Header>
      <Dialog.Title
        >实例变量 — {variableTarget?.businessTitle ??
          variableTarget?.flowName ??
          ''}</Dialog.Title
      >
    </Dialog.Header>
    <div class="space-y-4 py-4">
      <div>
        <div class="mb-1 text-sm font-medium">当前变量</div>
        {#if variableLoading}
          <div class="text-muted-foreground py-4 text-center text-sm">
            加载中...
          </div>
        {:else}
          <pre
            class="bg-muted max-h-60 overflow-auto rounded p-2 text-xs">{prettyJson(
              JSON.stringify(variables)
            )}</pre>
        {/if}
      </div>
      <!-- 写入走 PUT updateVariable,后端要 :variable(instance_handler.rs:369),
           与打开弹窗的 :variableQuery 是两个权限。整块隐藏而不是只禁按钮 ——
           只读用户留着两个填不出结果的输入框只会让人困惑。 -->
      <HasPermi perm="workflow:instance:variable">
        <div>
          <div class="mb-1 text-sm font-medium">新增 / 覆盖变量</div>
          <div class="flex flex-col gap-2">
            <Input bind:value={newVarKey} placeholder="变量名(KEY)" />
            <Input
              bind:value={newVarValue}
              placeholder="变量值(字符串或合法 JSON)"
            />
            <Button
              size="sm"
              disabled={addingVar || !newVarKey.trim()}
              onclick={handleAddVariable}
            >
              {addingVar ? '提交中...' : '保存变量'}
            </Button>
          </div>
        </div>
      </HasPermi>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (variableOpen = false)}
        >关闭</Button
      >
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
