<script lang="ts">
  import { untrack } from 'svelte'
  import { dictApi } from '$lib/api/system/dict.js'
  import type { DictDataVo } from '$lib/api/system/dict.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import {
    resolveDictTagClass,
    DICT_TAG_CLASS,
  } from '$lib/components/dict-tag.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { dictCache } from '$lib/hooks/use-dict.js'
  import { cn } from '$lib/utils.js'
  import { Plus, Pencil, Trash2, Download } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // 回显样式的 6 项值域。**键从 DICT_TAG_CLASS 导出**(全仓权威表),
  // 不另抄字面量;中文标签对齐 Vue listClassOptions(dict/index.vue:476-483)。
  const LIST_CLASS_LABEL: Record<string, string> = {
    default: '默认',
    primary: '主要',
    success: '成功',
    info: '信息',
    warning: '警告',
    danger: '危险',
  }
  const LIST_CLASS_OPTIONS = Object.keys(DICT_TAG_CLASS).map((value) => ({
    value,
    label: `${LIST_CLASS_LABEL[value] ?? value}(${value})`,
  }))

  type Props = {
    /** 左栏当前选中的字典类型;为空时展示空态提示。 */
    dictType: string
    /** 左栏当前选中的字典名称(用于副标题展示)。 */
    dictName?: string
  }
  const { dictType, dictName = '' }: Props = $props()

  // ---- 筛选 (数据标签) ----
  let dictLabelFilter = $state<string>('')

  const queryFields = [
    { key: 'dictLabel', label: '数据标签', placeholder: '请输入数据标签' },
  ]

  // SELECT_CLASS: 统一原生 select 外观,匹配 shadcn input。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // ---- columns: 普通列用 accessorFn 返回格式化文本;actions 列走 cellRenderer ----
  // 不做「是否默认」列(Vue 弹窗/列表均不暴露 isDefault)。listClass 列由 cellRenderer 渲染彩色 tag。
  const columns: ColumnDef<DictDataVo>[] = [
    { accessorKey: 'dictCode', header: '字典编码' },
    { accessorKey: 'dictLabel', header: '数据标签' },
    { accessorKey: 'dictValue', header: '数据键值' },
    { accessorKey: 'dictSort', header: '排序' },
    {
      accessorKey: 'listClass',
      header: '样式属性',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
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
    extra?: Record<string, string | number | undefined | null>
  }) {
    if (!dictType) {
      return { rows: [], totalRowCount: 0 }
    }
    const resp = await dictApi.dataList(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const customCellIds = new Set(['dictLabel'])

  const crudTable = useCrudTable<DictDataVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    // getRowId 必须配,否则勾选框不工作。用 dictCode(字典数据主键),不是 dictId。
    getRowId: (row) => String(row.dictCode),
    // dictType 来自父页选中行(prop),dictLabelFilter 来自搜索;两者都进入响应式依赖,
    // 变化时 useCrudTable 的 $effect 会重新拉取。
    getExtraParams: () => ({
      dictType,
      dictLabel: dictLabelFilter || undefined,
    }),
    // 与同页的类型表区分 sessionStorage 缓存键。
    cacheScope: 'data',
  })

  // 切换选中类型时回到第一页并清空标签筛选 (对齐 Vue 主从联动行为)。
  // untrack: 只取挂载时的初始值,之后由 $effect 手动跟踪比较。
  let prevDictType = untrack(() => dictType)
  $effect(() => {
    if (dictType !== prevDictType) {
      prevDictType = dictType
      dictLabelFilter = ''
      crudTable.resetPagination()
    }
  })

  function handleSearch(model: Record<string, string>) {
    dictLabelFilter = model.dictLabel ?? ''
    crudTable.resetPagination()
  }
  function handleReset() {
    dictLabelFilter = ''
    crudTable.resetPagination()
  }

  // ---- create / edit dialog ----
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editing = $state<Partial<DictDataVo>>({})
  const isEdit = $derived(editing.dictCode !== undefined)

  function openCreate() {
    // 新建时默认值
    editing = {
      dictType,
      dictSort: 0,
      isDefault: 'N', // 弹窗不暴露,但 BO 默认 N,显式带上对齐 Vue。
      cssClass: '',
      listClass: '',
    }
    dialogOpen = true
  }

  function openEdit(row: DictDataVo) {
    editing = { ...row }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (
      !editing.dictLabel ||
      editing.dictValue === undefined ||
      editing.dictValue === ''
    ) {
      toast.error('数据标签和数据值不能为空')
      return
    }
    saving = true
    try {
      // 新建时强制使用当前选中的 dictType (只读,不可编辑)
      const payload: Partial<DictDataVo> = {
        ...editing,
        dictType,
        dictSort: Number(editing.dictSort ?? 0),
      }
      if (isEdit) {
        await dictApi.dataUpdate(payload)
        toast.success('修改成功')
      } else {
        await dictApi.dataCreate(payload)
        toast.success('新增成功')
      }
      dialogOpen = false
      editing = {}
      // 🔴 dictCache 是模块级 Map 且无 TTL(use-dict.ts:5)—— 不清的话,改完标签
      // 全站 DictTag 到刷新页面前都还是旧值。改标签正是本页最常见的操作。
      dictCache.clear()
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
  let deleteTarget = $state<DictDataVo | null>(null)

  function openDelete(row: DictDataVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await dictApi.dataDelete(deleteTarget.dictCode)
      toast.success('删除成功')
      deleteOpen = false
      deleteTarget = null
      dictCache.clear() // 同 handleConfirm:少一项也得让全站 DictTag 重新拉
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      deleting = false
    }
  }

  // ---- 导出 ----
  let exporting = $state(false)

  async function handleDataExport() {
    exporting = true
    try {
      await dictApi.dataExport()
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛裸 Error,不经 apiClient,全局没人 toast。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }

  // ---- 数据侧批量删除 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  async function handleBatchDelete() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchDeleting = true
    try {
      await dictApi.dataDelete(ids.join(','))
      toast.success('批量删除成功')
      batchDeleteOpen = false
      crudTable.clearSelection()
      // 字典数据变了,前端那份 Map 里的旧值作废 —— 不清的话页面上的 DictTag 还是旧的。
      dictCache.clear()
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchDeleting = false
    }
  }
</script>

<div class="flex min-w-0 flex-col gap-4 sm:gap-6">
  {#if !dictType}
    <Card.Root>
      <Card.Content class="text-muted-foreground py-12 text-center">
        请先选择左侧字典类型
      </Card.Content>
    </Card.Root>
  {:else}
    <CrudTableShell
      table={crudTable.table}
      isLoading={crudTable.isLoading}
      error={crudTable.error}
      columnCount={columns.length}
      emptyMessage="未找到字典数据。"
      globalFilter=""
      showSearch={false}
      showRefresh={false}
      selectable
      title={dictType
        ? dictName
          ? `字典数据 / ${dictName} / ${dictType}`
          : `字典数据 / ${dictType}`
        : '字典数据'}
      {customCellIds}
    >
      {#snippet toolbar({ selectedCount })}
        {#if dictType}
          <HasPermi perm="system:dict:export">
            <Button
              variant="outline"
              size="sm"
              onclick={handleDataExport}
              disabled={exporting}
            >
              <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
            </Button>
          </HasPermi>
          <HasPermi perm="system:dict:add">
            <Button size="sm" onclick={openCreate}>
              <Plus class="mr-1 h-4 w-4" />新增
            </Button>
          </HasPermi>
        {/if}
        <HasPermi perm="system:dict:remove">
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
        <QueryForm
          fields={queryFields}
          onSearch={handleSearch}
          onReset={handleReset}
        />
      {/snippet}
      {#snippet cellRenderer({ row, columnId })}
        {@const r = row as DictDataVo}
        {#if columnId === 'dictLabel'}
          {@const cls = cn(resolveDictTagClass(r.listClass), r.cssClass ?? '')}
          {#if cls.trim()}
            <Badge variant="outline" class={cls}>{r.dictLabel}</Badge>
          {:else}
            <span>{r.dictLabel}</span>
          {/if}
        {:else}
          <div class="flex items-center gap-1">
            <HasPermi perm="system:dict:edit">
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
            <HasPermi perm="system:dict:remove">
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
  {/if}
</div>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑字典数据' : '新增字典数据'}
  loading={saving}
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="dictLabel">数据标签</Label>
    <Input
      id="dictLabel"
      bind:value={editing.dictLabel}
      placeholder="请输入数据标签"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="dictValue">数据键值</Label>
    <Input
      id="dictValue"
      bind:value={editing.dictValue}
      placeholder="请输入数据键值"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="dictSort">显示排序</Label>
    <Input
      id="dictSort"
      type="number"
      bind:value={editing.dictSort}
      placeholder="请输入显示排序"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="cssClass">样式属性</Label>
    <Input
      id="cssClass"
      bind:value={editing.cssClass}
      placeholder="请输入样式属性"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="listClass">回显样式</Label>
    <select id="listClass" class={SELECT_CLASS} bind:value={editing.listClass}>
      <!-- 实测库里 3 行 list_class 为空串,必须留空选项,否则编辑它们会被迫赋值 -->
      <option value="">不设置</option>
      {#each LIST_CLASS_OPTIONS as opt (opt.value)}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label for="remark">备注</Label>
    <Textarea
      id="remark"
      bind:value={editing.remark}
      placeholder="请输入备注"
      rows={3}
    />
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除字典数据「${deleteTarget?.dictLabel ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 条字典数据吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>
