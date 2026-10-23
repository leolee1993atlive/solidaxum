<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { genApi } from '$lib/api/tool/gen.js'
  import type { GenTableVo } from '$lib/api/tool/gen.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
      import { CrudTableShell } from '$lib/components/crud/index.js'
    import Main from '$lib/components/layout/Main.svelte'
  import GenImportDialog from '$lib/components/tool/GenImportDialog.svelte'
  import GenPreviewDialog from '$lib/components/tool/GenPreviewDialog.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import type { QueryField } from '$lib/components/query-form-types.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { fromListResult, useCrudTable } from '$lib/composables/use-crud-table.svelte.js'
  import {
    Eye,
    Pencil,
    RefreshCw,
    Trash2,
    Download,
    Upload,
  } from '@lucide/svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // 列序逐字对齐 Vue index.vue:96-107,含「数据源」与「更新时间」两列。
  // 序号列**不做** —— 全仓惯例(CrudTableShell 无 type=index,全仓 0 页有序号列)。
  const columns: ColumnDef<GenTableVo>[] = [
    {
      accessorKey: 'dataName',
      header: '数据源',
      accessorFn: (r) => r.dataName ?? '-',
    },
    {
      accessorKey: 'tableName',
      header: '表名称',
      accessorFn: (r) => r.tableName ?? '-',
    },
    {
      accessorKey: 'tableComment',
      header: '表描述',
      accessorFn: (r) => r.tableComment ?? '-',
    },
    {
      accessorKey: 'className',
      header: '实体',
      accessorFn: (r) => r.className ?? '-',
    },
    {
      accessorKey: 'createTime',
      header: '创建时间',
      accessorFn: (r) => r.createTime ?? '-',
    },
    {
      accessorKey: 'updateTime',
      header: '更新时间',
      accessorFn: (r) => r.updateTime ?? '-',
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const r = await genApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(r)
  }
  const crudTable = useCrudTable<GenTableVo>({
    columns,
    sortableIds: new Set(),
    fetchPage,
    // 🔴 selectable 必须配 getRowId —— enableRowSelection = Boolean(getRowId)。
    // 缺了它勾选框勾不上,三个批量按钮恒 disabled。
    getRowId: (row) => String(row.tableId),
    getExtraParams: () => ({ ...filters }),
  })

  // ---- 多字段筛选,顺序对齐 Vue index.vue:14-40 ----
  let dataNames = $state<string[]>([])
  let filters = $state<{
    dataName?: string
    tableName?: string
    tableComment?: string
    'params[beginTime]'?: string
    'params[endTime]'?: string
  }>({})

  // 数据源下拉。选项来自后端 getDataNames —— 目前硬编码只返回 ["master"]
  // (gen_handler.rs:285-295,注 "audit #84"),值域受限属**后端有意分歧**,不是前端缺陷。
  // 用 $derived 而不是常量:options 要等 getDataNames 回来才有值。
  // ⚠️ 类型写在变量上 —— $derived 不接受类型参数($derived<T>(...) 是语法错)。
  const queryFields: QueryField[] = $derived([
    {
      key: 'dataName',
      label: '数据源',
      type: 'select',
      options: dataNames.map((d) => ({ label: d, value: d })),
    },
    { key: 'tableName', label: '表名称', placeholder: '请输入表名称' },
    { key: 'tableComment', label: '表描述', placeholder: '请输入表描述' },
    {
      key: 'createTime',
      label: '创建时间',
      // 后端 GenTableQueryBo 的 params[beginTime]/[endTime]
      // (gen_table.rs:88-91 + gen_table_repo.rs:225-238)。
      // ⚠️ 必须补足时分秒 —— 后端严格解析 %Y-%m-%d %H:%M:%S,
      //    解析失败就跳过整条筛选。Vue 自己发的是 YYYY-MM-DD,本来就不生效。
      type: 'dateRange',
      beginKey: 'params[beginTime]',
      endKey: 'params[endTime]',
    },
  ])

  onMount(async () => {
    try {
      const resp = await genApi.getDataNames()
      const d = resp.data as unknown
      dataNames = Array.isArray(d)
        ? (d as string[])
        : Object.values((d ?? {}) as Record<string, string>)
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '加载数据源失败')
    }
    // GE12:从编辑页带回的 pageNum,落到分页状态上。
    // ⚠️ 这会触发一次额外的 fetch(初始 effect 已经用 pageIndex=0 拉过一次)。
    //    可以接受:只在带 ?pageNum= 且大于 1 时才发生,不是常规路径。
    const back = Number(page.url.searchParams.get('pageNum'))
    if (Number.isFinite(back) && back > 1) {
      crudTable.table.setPageIndex(back - 1)
    }
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      dataName: model.dataName,
      tableName: model.tableName,
      tableComment: model.tableComment,
      'params[beginTime]': model['params[beginTime]'],
      'params[endTime]': model['params[endTime]'],
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // 导入
  let importOpen = $state(false)
  // 预览
  let previewOpen = $state(false)
  let previewTableId = $state<string | null>(null)
  // 删除
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<GenTableVo | null>(null)

  function openPreview(row: GenTableVo) {
    previewTableId = row.tableId
    previewOpen = true
  }
  // GE12:Vue close() 会带回原 pageNum(editTable.vue:197-202)。
  // 用 URL query 显式传,不依赖菜单 isCache 驱动的 sessionStorage 缓存。
  function editHref(tableId: string): string {
    const p = crudTable.pagination.pageIndex + 1
    return `/tool/gen-edit/index/${tableId}?pageNum=${p}`
  }
  function openEdit(row: GenTableVo) {
    goto(editHref(row.tableId))
  }
  async function syncDb(row: GenTableVo) {
    try {
      await genApi.synchDb(row.tableId)
      toast.success('同步成功')
      crudTable.reload()
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '同步失败')
    }
  }
  async function gen(row: GenTableVo) {
    try {
      await genApi.genCode(row.tableId)
      toast.success('已开始下载')
    } catch (e) {
      // ⚠️ downloadBlob 绕开 apiClient 自己 fetch,抛裸 Error —— 必须自己 toast。
      console.error(e)
      toast.error(e instanceof Error ? e.message : '下载失败')
    }
  }
  function openDelete(row: GenTableVo) {
    deleteTarget = row
    deleteOpen = true
  }
  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await genApi.remove(deleteTarget.tableId)
      toast.success('删除成功')
      deleteOpen = false
      deleteTarget = null
      crudTable.reload()
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '删除失败')
    } finally {
      deleting = false
    }
  }
  // ---- 批量操作 ----
  let batchDeleteOpen = $state(false)
  let batchDeleting = $state(false)

  async function batchGen() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) {
      toast.error('请选择要生成的数据')
      return
    }
    try {
      await genApi.genCode(ids.join(','))
      toast.success('已开始下载')
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '下载失败')
    }
  }

  function batchEdit() {
    // Vue :disabled="single" —— 必须恰好一条。
    const ids = crudTable.selectedIds
    if (ids.length !== 1) return
    goto(editHref(ids[0]))
  }

  async function handleBatchDelete() {
    const ids = crudTable.selectedIds
    if (ids.length === 0) return
    batchDeleting = true
    try {
      await genApi.remove(ids.join(','))
      toast.success('批量删除成功')
      batchDeleteOpen = false
      crudTable.clearSelection()
      crudTable.reload()
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '删除失败')
    } finally {
      batchDeleting = false
    }
  }

  function onImported() {
    importOpen = false
    crudTable.reload()
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage="暂无生成表数据。"
    globalFilter=""
    showSearch={false}
    onRefresh={crudTable.reload}
    selectable
    title="数据表列表"
  >
    {#snippet toolbar({ selectedCount })}
      <HasPermi perm="tool:gen:import">
        <Button size="sm" onclick={() => (importOpen = true)}>
          <Upload class="mr-1 h-4 w-4" />导入
        </Button>
      </HasPermi>
      <HasPermi perm="tool:gen:code">
        <Button size="sm" onclick={batchGen}>
          <Download class="mr-1 h-4 w-4" />生成
        </Button>
      </HasPermi>
      <HasPermi perm="tool:gen:edit">
        <!-- Vue :disabled="single" —— 恰好一条才可点 -->
        <Button size="sm" variant="secondary" disabled={selectedCount !== 1} onclick={batchEdit}>
          <Pencil class="mr-1 h-4 w-4" />修改
        </Button>
      </HasPermi>
      <HasPermi perm="tool:gen:remove">
        <Button
          size="sm"
          variant="destructive"
          disabled={selectedCount === 0}
          onclick={() => (batchDeleteOpen = true)}
        >
          <Trash2 class="mr-1 h-4 w-4" />删除
        </Button>
      </HasPermi>
    {/snippet}
    {#snippet queryForm()}
      <QueryForm fields={queryFields} onSearch={handleSearch} onReset={handleReset} />
    {/snippet}
    {#snippet cellRenderer({ row })}
      {@const r = row as GenTableVo}
      <div class="flex items-center gap-1">
        <HasPermi perm="tool:gen:preview">
          <Button variant="ghost" size="icon" title="预览" aria-label="预览" onclick={() => openPreview(r)}>
            <Eye class="size-4" />
          </Button>
        </HasPermi>
        <HasPermi perm="tool:gen:edit">
          <Button variant="ghost" size="icon" title="编辑" aria-label="编辑" onclick={() => openEdit(r)}>
            <Pencil class="size-4" />
          </Button>
        </HasPermi>
        <HasPermi perm="tool:gen:remove">
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
        <!-- 同步此前裸露无守卫,而后端 synch_db 有 require_permission
             → 无权用户看得到、点下去拿 403。Vue index.vue:141-148 用的是 tool:gen:edit。 -->
        <HasPermi perm="tool:gen:edit">
          <Button variant="ghost" size="icon" title="同步" aria-label="同步" onclick={() => syncDb(r)}>
            <RefreshCw class="size-4" />
          </Button>
        </HasPermi>
        <HasPermi perm="tool:gen:code">
          <Button variant="ghost" size="icon" title="生成代码" aria-label="生成代码" onclick={() => gen(r)}>
            <Download class="size-4" />
          </Button>
        </HasPermi>
      </div>
    {/snippet}
  </CrudTableShell>
</Main>

<GenImportDialog
  open={importOpen}
  dataName={filters.dataName}
  onOpenChange={(v) => (importOpen = v)}
  {onImported}
/>
<GenPreviewDialog
  open={previewOpen}
  tableId={previewTableId}
  onOpenChange={(v) => (previewOpen = v)}
/>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定删除生成配置「${deleteTarget?.tableName ?? ''}」吗？`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定删除选中的 ${crudTable.selectedIds.length} 张生成配置吗?`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>
