<script lang="ts">
  import { toast } from 'svelte-sonner'
  import type { ColumnDef } from '@tanstack/table-core'
  import { page } from '$app/state'

  import { definitionApi } from '$lib/api/workflow/definition.js'
  import type { DefinitionListVo, DefinitionForm } from '$lib/api/workflow/definition.js'
  import {
    buildDefinitionForm,
    toDefinitionBody,
    EMPTY_DEFINITION_FORM,
  } from '$lib/api/workflow/definition.js'
  import { categoryApi } from '$lib/api/workflow/category.js'
  import type { CategoryTreeVo } from '$lib/api/workflow/category.js'
  import TreePanel from '$lib/components/TreePanel.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import Main from '$lib/components/layout/Main.svelte'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import { DICT_TAG_CLASS } from '$lib/components/dict-tag.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { goto } from '$app/navigation'
  import { Copy, Trash2, Send, SendHorizonal, Zap, FileCode, Pencil, Upload, Download } from '@lucide/svelte'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{ flowName?: string; flowCode?: string }>({})

  const queryFields = [
    { key: 'flowName', label: '流程名称', placeholder: '请输入流程名称' },
    { key: 'flowCode', label: '流程编码', placeholder: '请输入流程编码' },
  ]

  // ---- dict label helpers ----
  // 发布状态走 customCellIds 三态 tag,这里只需 activity 文本
  function activityLabel(v?: number): string {
    if (v === 0) return '未激活'
    if (v === 1) return '已激活'
    return v == null ? '-' : String(v)
  }

  // 发布状态三态配色(对齐 Vue el-tag:index.vue:151-153)
  // 0=未发布(danger) / 1=已发布(success) / 其他=失效(danger)
  function publishBadgeClass(v?: number): string {
    return v === 1 ? DICT_TAG_CLASS.success : DICT_TAG_CLASS.danger
  }
  function publishBadgeLabel(v?: number): string {
    if (v === 0) return '未发布'
    if (v === 1) return '已发布'
    if (v == null) return '-'
    return '失效'
  }

  // ---- columns(对齐 Vue index.vue:119-156) ----
  // 去掉「定义编号」「创建时间」(Vue 无);label 改为 Vue 的「流程定义名称」「标识KEY」「版本号」;
  // 版本号渲染 v{n}.0;列序对齐 Vue:flowName/flowCode/categoryName/version/activityStatus/isPublish。
  const columns: ColumnDef<DefinitionListVo>[] = [
    {
      accessorKey: 'flowName',
      header: '流程定义名称',
      accessorFn: (row) => row.flowName ?? '-',
    },
    { accessorKey: 'flowCode', header: '标识KEY', accessorFn: (row) => row.flowCode ?? '-' },
    {
      accessorKey: 'categoryName',
      header: '流程分类',
      accessorFn: (row) => row.categoryName ?? '-',
    },
    {
      accessorKey: 'version',
      header: '版本号',
      accessorFn: (row) => (row.version != null ? `v${row.version}.0` : '-'),
    },
    // 激活状态:Vue 是行内 switch,这里仍按列展示文本
    {
      accessorKey: 'activityStatus',
      header: '激活状态',
      accessorFn: (row) => activityLabel(row.activityStatus),
    },
    // 发布状态三态 tag(走 customCellIds,见 cellRenderer)
    { accessorKey: 'isPublish', header: '发布状态', enableSorting: false },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  // ---- 左侧流程分类树 ----
  let categoryTree = $state<CategoryTreeVo[]>([])
  /** 选中的分类 id;undefined = 全部(对齐 Vue handleNodeClick 里 id==='0' 归空) */
  let selectedCategory = $state<string | undefined>(undefined)

  async function loadCategoryTree() {
    categoryTree = (await categoryApi.categoryTree()).data ?? []
  }

  function handleCategoryClick(id: string | undefined) {
    // 根节点(id '0')等同「全部」,与 Vue index.vue:435-441 一致
    selectedCategory = id === '0' ? undefined : id
    // selectedCategory 在 getExtraParams 里被读,改它本身就会触发重取;
    // 这里要的是顺带回到第 1 页(Vue handleNodeClick → handleQuery → pageNum=1)。
    crudTable.resetPagination()
  }

  // ---- 已发布/未发布 Tab。取值对齐 Vue activeName:'0' 已发布 / '1' 未发布 ----
  let activeTab = $state<'0' | '1'>('0')

  // 从设计器回跳时恢复原 Tab(对齐 Vue getPageList 对 route.query.activeName 的处理)
  const initialTab = page.url.searchParams.get('activeName')
  if (initialTab === '1') activeTab = '1'

  function switchTab(tab: '0' | '1') {
    if (activeTab === tab) return
    activeTab = tab
    crudTable.table.resetRowSelection()
    // 必须重置分页:activeTab 只在 fetchPage 里被读(在 untrack 内),不构成响应式依赖,
    // 所以要靠 reload 触发重取;而不回第 1 页的话,从第 3 页切到只有 1 页的 Tab 会停在空页。
    // (Vue handleClick → handleQuery 里就是 pageNum = 1,index.vue:447-454)
    crudTable.resetPagination()
    crudTable.reload()
  }

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    // 未发布流程(新建/复制/导入的产物)只在 unPublishList 可见,list 查不到
    const call = activeTab === '0' ? definitionApi.list : definitionApi.unPublishList
    const resp = await call(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<DefinitionListVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    // 🔴 selectable 必须配 getRowId —— enableRowSelection 由 Boolean(getRowId) 决定。
    // 缺了它勾选框永远勾不上,导出与「修改」(disabled={single})恒禁用。
    getRowId: (row) => String(row.id),
    getExtraParams: () => ({
      flowName: filters.flowName,
      flowCode: filters.flowCode,
      category: selectedCategory,
    }),
  })

  // $effect for category tree loading
  $effect(() => {
    void loadCategoryTree()
  })

  function handleSearch(model: Record<string, string>) {
    filters = { flowName: model.flowName, flowCode: model.flowCode }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 行选择 ----
  const selectedRows = $derived(
    crudTable.table.getSelectedRowModel().rows.map((r) => r.original)
  )
  const single = $derived(selectedRows.length !== 1)
  const multiple = $derived(selectedRows.length === 0)

  // isPublish 列走自定义 cell(三态 tag)
  const customCellIds = new Set(['isPublish'])

  // ---- 导出(工具栏按钮 + 依赖行选择,取勾选第一行) ----
  async function handleExportDef() {
    const row = selectedRows[0]
    if (!row) return
    try {
      await definitionApi.exportDef(row.id, row.flowCode)
      toast.success('导出成功')
    } catch (e) {
      // downloadBlob 走自造 fetch、抛裸 Error,不经 apiClient —— 全局 toast 管不到,
      // 这里不自己报就是「点了没反应」。同 exportBlob 的注意事项。
      toast.error(e instanceof Error ? e.message : '导出失败')
    }
  }

  // ---- 部署流程文件(multipart) ----
  let deployOpen = $state(false)
  let deploying = $state(false)
  let deployCategory = $state('')
  let deployFile = $state<File | null>(null)

  function openDeployDialog() {
    deployCategory = selectedCategory ?? ''
    deployFile = null
    deployOpen = true
  }

  function onDeployFileChange(e: Event) {
    const input = e.target as HTMLInputElement
    deployFile = input.files?.[0] ?? null
  }

  async function handleDeploy() {
    if (!deployFile) {
      toast.error('请选择流程定义 JSON 文件')
      return
    }
    deploying = true
    try {
      // 注意:category 当前后端不消费(definition_handler.rs:403-420 只取第一个带 filename 的字段)
      await definitionApi.importDef(deployFile, deployCategory)
      toast.success('部署成功')
      deployOpen = false
      // 导入产物是未发布的,切到未发布 Tab 才看得到(对齐 Vue)
      activeTab = '1'
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      deploying = false
    }
  }

  // ---- publish / unPublish ----
  async function handlePublish(row: DefinitionListVo) {
    try {
      await definitionApi.publish(row.id)
      toast.success('发布成功')
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    }
  }

  async function handleUnPublish(row: DefinitionListVo) {
    try {
      await definitionApi.unPublish(row.id)
      toast.success('已取消发布')
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    }
  }

  // ---- copy ----
  async function handleCopy(row: DefinitionListVo) {
    try {
      await definitionApi.copy(row.id)
      toast.success('复制成功')
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    }
  }

  // ---- active ----
  async function handleActive(row: DefinitionListVo) {
    try {
      await definitionApi.active(row.id, true)
      toast.success('激活成功')
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    }
  }

  // ---- 设计器入口(按发布状态给可编辑/只读) ----
  function gotoDesign(row: DefinitionListVo, readonly: boolean) {
    const qs = new URLSearchParams({
      definitionId: row.id,
      disabled: String(readonly),
      activeName: activeTab,
    })
    goto(`/workflow/design/index?${qs.toString()}`)
  }

  // ---- single delete ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<DefinitionListVo | null>(null)

  function openDelete(row: DefinitionListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await definitionApi.delete(String(deleteTarget.id))
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

  // ---- batch delete ----
  let batchDeleteOpen = $state(false)

  function confirmBatchDelete() {
    if (selectedRows.length === 0) return
    batchDeleteOpen = true
  }

  async function doBatchDelete() {
    const ids = selectedRows.map((r) => r.id).join(',')
    await definitionApi.delete(ids)
    toast.success('删除成功')
    batchDeleteOpen = false
    crudTable.table.resetRowSelection()
    crudTable.reload()
  }

  // ---- 新增/编辑弹窗 ----
  let dialogOpen = $state(false)
  let dialogTitle = $state('新增流程')
  let form = $state<DefinitionForm>({ ...EMPTY_DEFINITION_FORM })
  let autoPass = $state(false)
  /** 编辑时禁改设计器模式(对齐 Vue :disabled="!!form.id") */
  const isEdit = $derived(form.id !== '')

  function openAddDialog() {
    form = { ...EMPTY_DEFINITION_FORM }
    // 左树选了分类就预填(对齐 Vue handleAdd,index.vue:614-622)
    if (selectedCategory) form.category = selectedCategory
    autoPass = false
    dialogTitle = '新增流程'
    dialogOpen = true
  }

  async function openEditDialog() {
    const row = selectedRows[0]
    if (!row) return
    const vo = await definitionApi.getById(row.id)
    const built = buildDefinitionForm(vo.data ?? {})
    form = built.form
    autoPass = built.autoPass
    dialogTitle = '修改流程'
    dialogOpen = true
  }

  async function submitDialog() {
    if (!form.category) return toast.error('分类名称不能为空')
    if (!form.flowCode.trim()) return toast.error('流程定义编码不能为空')
    if (!form.flowName.trim()) return toast.error('流程定义名称不能为空')
    if (!form.modelValue) return toast.error('设计器模式不能为空')

    const body = toDefinitionBody(form, autoPass)
    if (form.id) {
      await definitionApi.edit(body)
    } else {
      await definitionApi.add(body)
      // 新建产物是未发布的,Vue 提交后直接切到「未发布」Tab,否则用户看不到刚建的流程
      activeTab = '1'
    }
    toast.success('操作成功')
    dialogOpen = false
    crudTable.reload()
  }

  // ---- view XML dialog ----
  let xmlOpen = $state(false)
  let xmlLoading = $state(false)
  let xmlContent = $state('')
  let xmlTarget = $state<DefinitionListVo | null>(null)

  async function openXml(row: DefinitionListVo) {
    xmlTarget = row
    xmlContent = ''
    xmlOpen = true
    xmlLoading = true
    try {
      const resp = await definitionApi.getXml(row.id)
      xmlContent = resp.data ?? ''
    } catch {
      // apiClient 已 toast
    } finally {
      xmlLoading = false
    }
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
      <CrudTableShell
        table={crudTable.table}
        isLoading={crudTable.isLoading}
        error={crudTable.error}
        columnCount={columns.length}
        emptyMessage="未找到流程定义数据。"
        globalFilter=""
        showSearch={false}
        showRefresh={false}
        selectable
        customCellIds={customCellIds}>
        <!-- 已发布/未发布是流程自身的状态,不是顶层视图:做成分段页签占标题位,
             不再在卡外单独摆一排(那排裸下划线按钮在青绿的渐变底上没有自己的面)。
             不用 Tabs.Content:表格本身就是内容,切换靠 switchTab 重查。 -->
        {#snippet heading()}
          <Tabs.Root value={activeTab} onValueChange={(v) => switchTab(v as '0' | '1')}>
            <Tabs.List>
              <Tabs.Trigger value="0">已发布</Tabs.Trigger>
              <Tabs.Trigger value="1">未发布</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        {/snippet}
        {#snippet toolbar()}
          <HasPermi perm="workflow:definition:add">
            <Button size="sm" onclick={() => openAddDialog()}>添加</Button>
          </HasPermi>
          <!-- 部署流程文件(multipart 上传) -->
          <!-- 走 importDef → 后端 import_def(definition_handler.rs:395)要的是 :import,不是 :add -->
          <HasPermi perm="workflow:definition:import">
            <Button size="sm" variant="secondary" onclick={openDeployDialog}>
              <Upload class="mr-1 h-4 w-4" />部署
            </Button>
          </HasPermi>
          <!-- 导出(工具栏按钮 + 依赖行选择,取勾选第一行,对齐 Vue index.vue:681) -->
          <HasPermi perm="workflow:definition:export">
            <Button size="sm" variant="secondary" disabled={single} onclick={handleExportDef}>
              <Download class="mr-1 h-4 w-4" />导出
            </Button>
          </HasPermi>
          <HasPermi perm="workflow:definition:edit">
            <Button size="sm" variant="secondary" disabled={single} onclick={() => openEditDialog()}>
              修改
            </Button>
          </HasPermi>
          <HasPermi perm="workflow:definition:remove">
            <Button size="sm" variant="destructive" disabled={multiple} onclick={() => confirmBatchDelete()}>
              删除
            </Button>
          </HasPermi>
        {/snippet}
        {#snippet queryForm()}
          <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
        {/snippet}
        {#snippet cellRenderer({ row, columnId })}
          {@const r = row as DefinitionListVo}
          {#if columnId === 'isPublish'}
            <!-- 发布状态三态 tag(无对应字典,Vue el-tag 硬编码) -->
            <Badge variant="outline" class={publishBadgeClass(r.isPublish)}>
              {publishBadgeLabel(r.isPublish)}
            </Badge>
          {:else}
            <div class="flex items-center gap-1">
            <HasPermi perm="workflow:definition:query">
              {#if r.isPublish === 0}
                <Button size="sm" variant="ghost" onclick={() => gotoDesign(r, false)}>流程设计</Button>
              {:else}
                <Button size="sm" variant="ghost" onclick={() => gotoDesign(r, true)}>查看流程</Button>
              {/if}
            </HasPermi>
            {#if r.isPublish === 0}
              <HasPermi perm="workflow:definition:publish">
                <Button size="sm" variant="ghost" title="发布" aria-label="发布" onclick={() => handlePublish(r)}>
                  <Send class="size-4" />
                </Button>
              </HasPermi>
            {:else}
              <HasPermi perm="workflow:definition:publish">
                <Button size="sm" variant="ghost" title="取消发布" aria-label="取消发布" onclick={() => handleUnPublish(r)}>
                  <SendHorizonal class="size-4" />
                </Button>
              </HasPermi>
            {/if}
            <HasPermi perm="workflow:definition:copy">
              <Button size="sm" variant="ghost" title="复制" aria-label="复制" onclick={() => handleCopy(r)}>
                <Copy class="size-4" />
              </Button>
            </HasPermi>
            {#if r.activityStatus === 0}
              <!-- 走 definitionApi.active → active_definition(definition_handler.rs:379)要的是 :active,不是 :edit -->
              <HasPermi perm="workflow:definition:active">
                <Button size="sm" variant="ghost" title="激活" aria-label="激活" onclick={() => handleActive(r)}>
                  <Zap class="size-4" />
                </Button>
              </HasPermi>
            {/if}
            <HasPermi perm="workflow:definition:remove">
              <Button size="sm" variant="ghost" title="删除" aria-label="删除" onclick={() => openDelete(r)}>
                <Trash2 class="size-4 text-destructive" />
              </Button>
            </HasPermi>
            <HasPermi perm="workflow:definition:query">
              <Button size="sm" variant="ghost" title="查看 XML" aria-label="查看 XML" onclick={() => openXml(r)}>
                <FileCode class="size-4" />
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
  desc={`确定要删除流程定义「${deleteTarget?.flowName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete} />

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认删除"
  desc={`是否确认删除流程定义编码为【${selectedRows.map((r) => r.flowCode).join('、')}】的数据项？`}
  confirmText="删除"
  destructive
  handleConfirm={doBatchDelete}
/>

<!-- 新增/编辑弹窗 -->
<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="sm:max-w-[650px]">
    <Dialog.Header><Dialog.Title>{dialogTitle}</Dialog.Title></Dialog.Header>
    <div class="grid gap-4 py-2">
      <div class="grid grid-cols-[120px_1fr] items-center gap-3">
        <span class="text-sm">流程类别</span>
        <TreeSelect treeData={categoryTree} value={form.category} placeholder="请选择流程类别" onChange={(id) => { form.category = String(id) }} />

        <span class="text-sm">流程编码</span>
        <Input bind:value={form.flowCode} maxlength={40} placeholder="请输入流程编码" />

        <span class="text-sm">流程名称</span>
        <Input bind:value={form.flowName} maxlength={100} placeholder="请输入流程名称" />

        <span class="text-sm">设计器模式</span>
        <div class="flex gap-2">
          {#each [{ v: 'CLASSICS', label: '经典模式' }, { v: 'MIMIC', label: '仿钉钉模式' }] as m (m.v)}
            <Button
              size="sm"
              variant={form.modelValue === m.v ? 'default' : 'outline'}
              disabled={isEdit}
              onclick={() => (form.modelValue = m.v)}
            >
              {m.label}
            </Button>
          {/each}
        </div>

        <span class="text-sm">流程配置</span>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" bind:checked={autoPass} />
          下一节点执行人是当前任务处理人自动审批
        </label>

        <span class="text-sm">是否动态表单</span>
        <div class="flex gap-2">
          <Button size="sm" variant="outline" disabled>是</Button>
          <Button
            size="sm"
            variant={form.formCustom === 'N' ? 'default' : 'outline'}
            onclick={() => (form.formCustom = 'N')}
          >
            否
          </Button>
        </div>

        <span class="text-sm">表单路径</span>
        <Input bind:value={form.formPath} maxlength={100} placeholder="请输入表单路径" />
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (dialogOpen = false)}>取消</Button>
      <Button onclick={submitDialog}>保存</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- 查看 XML:只读展示流程定义的 xmlString,无确定提交动作 -->
<Dialog.Root bind:open={xmlOpen}>
  <Dialog.Content class="sm:max-w-[720px]">
    <Dialog.Header>
      <Dialog.Title>流程 XML — {xmlTarget?.flowName ?? ''}</Dialog.Title>
    </Dialog.Header>
    <div class="py-4">
      {#if xmlLoading}
        <div class="text-muted-foreground text-center text-sm py-8">加载中...</div>
      {:else}
        <Textarea value={xmlContent} readonly rows={20} class="font-mono text-xs" />
      {/if}
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (xmlOpen = false)}>关闭</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- 部署流程文件弹窗(multipart 上传) -->
<Dialog.Root bind:open={deployOpen}>
  <Dialog.Content class="sm:max-w-[520px]">
    <Dialog.Header><Dialog.Title>部署流程文件</Dialog.Title></Dialog.Header>
    <div class="grid gap-4 py-2">
      <div class="grid grid-cols-[100px_1fr] items-center gap-3">
        <span class="text-sm">流程分类</span>
        <!-- 注:当前后端 import_def 不消费 category 字段(definition_handler.rs:403),
             导入后分类取自 JSON 文件内容;此处按 Vue 形态保留选择器 -->
        <TreeSelect
          treeData={categoryTree}
          value={deployCategory}
          placeholder="请选择流程分类(当前后端暂不生效)"
          rootOption={{ id: '', label: '不指定' }}
          onChange={(id) => {
            deployCategory = String(id)
          }} />
        <span class="text-sm">流程文件</span>
        <input
          type="file"
          accept=".json,application/json"
          class="text-sm"
          onchange={onDeployFileChange} />
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (deployOpen = false)}>取消</Button>
      <Button disabled={deploying || !deployFile} onclick={handleDeploy}>
        {deploying ? '部署中...' : '部署'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
