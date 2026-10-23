<script lang="ts">
  import { dictApi } from '$lib/api/system/dict.js'
  import type { DictTypeVo } from '$lib/api/system/dict.js'
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
  import { dictCache } from '$lib/hooks/use-dict.js'
  import { Plus, Pencil, Trash2, Download, RefreshCw } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'
  import DictDataPanel from './dict-data-panel.svelte'

  // ---- 多字段筛选 (RuoYi 风格) ----
  let filters = $state<{ dictName?: string; dictType?: string }>({})

  const queryFields = [
    { key: 'dictName', label: '字典名称', placeholder: '请输入字典名称' },
    { key: 'dictType', label: '字典类型', placeholder: '请输入字典类型' },
  ]

  // ---- columns: 普通列用 accessorFn 返回格式化文本;actions 列走 cellRenderer ----
  const columns: ColumnDef<DictTypeVo>[] = [
    { accessorKey: 'dictId', header: '字典编号' },
    { accessorKey: 'dictName', header: '字典名称' },
    { accessorKey: 'dictType', header: '字典类型' },
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
    const resp = await dictApi.typeList(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<DictTypeVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    // getRowId 必须配,否则 useCrudTable 的 enableRowSelection=false,勾选框不工作。
    getRowId: (row) => String(row.dictId),
    getExtraParams: () => ({
      dictName: filters.dictName,
      dictType: filters.dictType,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = { dictName: model.dictName, dictType: model.dictType }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 主从联动:点击类型行选中,驱动右栏数据面板 ----
  let selected = $state<DictTypeVo | null>(null)

  function selectType(row: DictTypeVo) {
    selected = row
  }

  function typeRowClass(row: DictTypeVo) {
    return row.dictId === selected?.dictId
      ? 'bg-muted hover:bg-muted cursor-pointer'
      : 'cursor-pointer'
  }

  // ---- create / edit dialog ----
  let dialogOpen = $state(false)
  let saving = $state(false)
  let editing = $state<Partial<DictTypeVo>>({})
  const isEdit = $derived(editing.dictId !== undefined)

  function openCreate() {
    editing = {}
    dialogOpen = true
  }

  function openEdit(row: DictTypeVo) {
    editing = { ...row }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!editing.dictName || !editing.dictType) {
      toast.error('字典名称和字典类型不能为空')
      return
    }
    saving = true
    try {
      if (isEdit) {
        await dictApi.typeUpdate(editing)
        toast.success('修改成功')
        // 同步右栏副标题(dictType 可改,名称/类型/备注都可能变化)
        if (selected && editing.dictId === selected.dictId) {
          selected = { ...selected, ...editing }
        }
        // 🔴 dictType 可改 —— 缓存是按 dictType 做 key 的(use-dict.ts:14),
        // 改名后旧 key 会一直悬空命中,页面上的 DictTag 停在旧字典上。
        dictCache.clear()
      } else {
        await dictApi.typeCreate(editing)
        toast.success('新增成功')
      }
      dialogOpen = false
      editing = {}
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
  let deleteTarget = $state<DictTypeVo | null>(null)

  function openDelete(row: DictTypeVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await dictApi.typeDelete(deleteTarget.dictId)
      toast.success('删除成功')
      // 删除的正是当前选中类型时,右栏回到空态
      if (selected?.dictId === deleteTarget.dictId) {
        selected = null
      }
      deleteOpen = false
      deleteTarget = null
      dictCache.clear() // 同批量删除:类型没了,缓存里的旧字典也该作废
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      deleting = false
    }
  }

  // ---- 导出 ----
  let exporting = $state(false)

  async function handleTypeExport() {
    exporting = true
    try {
      await dictApi.typeExport()
      toast.success('导出成功')
    } catch (e) {
      // exportBlob 抛裸 Error,不经 apiClient,全局没人 toast。
      toast.error(e instanceof Error ? e.message : '导出失败')
    } finally {
      exporting = false
    }
  }

  // ---- 刷新缓存 ----
  let refreshing = $state(false)

  async function handleRefreshCache() {
    refreshing = true
    try {
      await dictApi.typeRefreshCache()
      // 🔴 后端缓存刷了,前端这份 Map 不清的话页面上的 DictTag 还是旧值。
      dictCache.clear()
      toast.success('刷新成功')
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      refreshing = false
    }
  }

  // ---- 类型侧批量删除 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  async function handleBatchDelete() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchDeleting = true
    try {
      await dictApi.typeDelete(ids.join(','))
      toast.success('批量删除成功')
      batchDeleteOpen = false
      crudTable.clearSelection()
      // 删掉的可能正是右栏选中的类型,清掉选中态避免右栏悬空。
      if (selected && ids.includes(String(selected.dictId))) selected = null
      dictCache.clear() // 类型没了,缓存里的旧字典也该作废
      crudTable.reload()
    } catch {
      // apiClient 已 toast
    } finally {
      batchDeleting = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="grid gap-4 sm:gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
    <!-- 左栏:字典类型 -->
    <section class="flex min-w-0 flex-col gap-4 sm:gap-6">
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
        title="字典列表"
        onRowClick={selectType}
        rowClass={typeRowClass}
      >
        {#snippet toolbar({ selectedCount })}
          <HasPermi perm="system:dict:export">
            <Button
              variant="outline"
              size="sm"
              onclick={handleTypeExport}
              disabled={exporting}
            >
              <Download class="mr-1 h-4 w-4" />{exporting ? '导出中…' : '导出'}
            </Button>
          </HasPermi>
          <HasPermi perm="system:dict:remove">
            <Button
              variant="outline"
              size="sm"
              onclick={handleRefreshCache}
              disabled={refreshing}
            >
              <RefreshCw class="mr-1 h-4 w-4" />{refreshing
                ? '刷新中…'
                : '刷新'}
            </Button>
          </HasPermi>
          <HasPermi perm="system:dict:add">
            <Button size="sm" onclick={openCreate}>
              <Plus class="mr-1 h-4 w-4" />新增
            </Button>
          </HasPermi>
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
        {#snippet cellRenderer({ row })}
          {@const r = row as DictTypeVo}
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
        {/snippet}
      </CrudTableShell>
    </section>

    <!-- 右栏:字典数据 (随左栏选中类型刷新) -->
    <section class="min-w-0">
      <DictDataPanel
        dictType={selected?.dictType ?? ''}
        dictName={selected?.dictName ?? ''}
      />
    </section>
  </div>
</Main>

<FormDrawer
  open={dialogOpen}
  onOpenChange={(o) => (dialogOpen = o)}
  title={isEdit ? '编辑字典类型' : '新增字典类型'}
  loading={saving}
  class="sm:max-w-[520px]"
  onConfirm={handleConfirm}
>
  <div class="space-y-4">
    <div class="flex flex-col gap-3">
      <Label for="dictName">字典名称</Label>
      <Input
        id="dictName"
        bind:value={editing.dictName}
        placeholder="请输入字典名称"
      />
    </div>
    <div class="flex flex-col gap-3">
      <Label for="dictType">
        字典类型
        <span class="text-muted-foreground text-xs">
          （数据存储中的Key值，如：sys_user_gender）
        </span>
      </Label>
      <Input
        id="dictType"
        bind:value={editing.dictType}
        placeholder="请输入字典类型"
      />
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
  </div>
</FormDrawer>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除字典类型「${deleteTarget?.dictName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 条字典类型吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>
