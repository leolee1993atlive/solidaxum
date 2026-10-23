<script lang="ts">
  import { page } from '$app/state'
  import { noticeApi } from '$lib/api/system/notice.js'
  import type {
    NoticeListVo,
    NoticeCreateBo,
    NoticeUpdateBo,
  } from '$lib/api/system/notice.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import DictTag from '$lib/components/DictTag.svelte'
  import FormDrawer from '$lib/components/FormDrawer.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import RichTextEditor from '$lib/components/RichTextEditor.svelte'
  import SafeHtml from '$lib/components/SafeHtml.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import { Plus, Pencil, Trash2, Eye } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 多字段筛选 (RuoYi 风格) ----
  // NTC3:createByName 后端匹配登录名 user_name(notice_repo.rs:147),不是昵称 —— 用登录名试。
  let filters = $state<{
    noticeTitle?: string
    createByName?: string
    noticeType?: string
  }>({})

  const queryFields = [
    { key: 'noticeTitle', label: '公告标题', placeholder: '请输入公告标题' },
    { key: 'createByName', label: '操作人员', placeholder: '请输入操作人员' },
    {
      key: 'noticeType',
      label: '类型',
      type: 'select' as const,
      dictType: 'sys_notice_type',
    },
  ]

  // SELECT_CLASS 与 user/role 页一致:统一原生 select 外观,匹配 shadcn input 高度/边框。
  const SELECT_CLASS =
    'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-[3px]'

  // ---- columns: 普通列用 accessorFn 返回格式化文本;actions 列走 cellRenderer ----
  // noticeType / status 改走 cellRenderer 渲染 DictTag(值域对接 sys_notice_type / sys_notice_status)。
  const customCellIds = new Set(['noticeType', 'status'])
  const columns: ColumnDef<NoticeListVo>[] = [
    // 「公告编号」不出列:Vue 那列写的是 v-if="false"(index.vue:88),是明确关掉的。
    { accessorKey: 'noticeTitle', header: '公告标题' },
    { accessorKey: 'noticeType', header: '公告类型' },
    { accessorKey: 'status', header: '状态' },
    {
      accessorKey: 'createByName',
      header: '创建者',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'createTime',
      header: '创建时间',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { id: 'actions', header: '操作', cell: (info) => info.row.original },
  ]

  async function fetchPage(params: {
    pageNo: number
    pageSize: number
    extra?: Record<string, string | number | undefined | null>
  }) {
    const resp = await noticeApi.list(
      { pageNum: params.pageNo, pageSize: params.pageSize },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<NoticeListVo>({
    columns,
    // 排序未接线(fetchPage 不转发 orderByColumn/isAsc),留着 sortableIds
    // 只会长出点不动的表头控件。接线属独立议题,全仓其余页面同样未接线。
    sortableIds: new Set(),
    fetchPage,
    getExtraParams: () => ({
      noticeTitle: filters.noticeTitle,
      createByName: filters.createByName,
      noticeType: filters.noticeType,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      noticeTitle: model.noticeTitle,
      createByName: model.createByName,
      noticeType: model.noticeType,
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

  interface NoticeFormModel {
    noticeTitle: string
    noticeType: string
    noticeContent: string
    status: string
  }

  function emptyForm(): NoticeFormModel {
    return {
      noticeTitle: '',
      noticeType: '1',
      noticeContent: '',
      status: '0',
    }
  }

  let form = $state<NoticeFormModel>(emptyForm())

  function openCreate() {
    editingId = null
    form = emptyForm()
    dialogOpen = true
  }

  function openEdit(row: NoticeListVo) {
    editingId = row.noticeId
    // 列表行字段与详情一致,直接用行数据预填(避免额外网络往返)
    form = {
      noticeTitle: row.noticeTitle ?? '',
      noticeType: row.noticeType ?? '1',
      noticeContent: row.noticeContent ?? '',
      status: row.status ?? '0',
    }
    dialogOpen = true
  }

  async function handleConfirm() {
    if (!form.noticeTitle) {
      toast.error('公告标题不能为空')
      return
    }
    saving = true
    try {
      if (isEdit && editingId !== null) {
        const payload: NoticeUpdateBo = {
          noticeId: editingId,
          noticeTitle: form.noticeTitle,
          noticeType: form.noticeType || undefined,
          noticeContent: form.noticeContent || undefined,
          status: form.status,
        }
        await noticeApi.update(payload)
        toast.success('修改成功')
      } else {
        const payload: NoticeCreateBo = {
          noticeTitle: form.noticeTitle,
          noticeType: form.noticeType || undefined,
          noticeContent: form.noticeContent || undefined,
          status: form.status,
        }
        await noticeApi.create(payload)
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
  let deleteTarget = $state<NoticeListVo | null>(null)

  function openDelete(row: NoticeListVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await noticeApi.delete(deleteTarget.noticeId)
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

  // ---- NTC2 详情弹窗 ----
  let detailOpen = $state(false)
  let detail = $state<NoticeListVo | null>(null)

  // 行操作入口:直接用列表行数据,不发单查 —— 单查端点不做 create_by_name 翻译
  // (见 §1 事实 1 的 ⚠️),单查拿到的创建者是 None,会显示 '-'。
  function openDetailFromRow(row: NoticeListVo) {
    detail = row
    detailOpen = true
  }

  // 深链接入口:?noticeId=xxx 带参进页面直接开详情(对齐 Vue index.vue:359,397-399)。
  // 此场景没有行数据,只能发单查 —— 创建者可能显示 '-',是已知取舍。
  // 只在 id 变化时触发,避免关闭弹窗后被 effect 重新打开。
  let lastDeepLinkId = $state<string | null>(null)
  $effect(() => {
    const id = page.url.searchParams.get('noticeId')
    if (id && id !== lastDeepLinkId) {
      lastDeepLinkId = id
      void openDetailFromDeepLink(id)
    }
  })

  async function openDetailFromDeepLink(noticeId: string) {
    try {
      const resp = await noticeApi.get(noticeId)
      detail = resp.data ?? null
      detailOpen = true
    } catch {
      // apiClient 已 toast
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
    title="公告列表"
    emptyMessage="未找到公告数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
  >
    {#snippet toolbar()}
      <HasPermi perm="system:notice:add">
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
      {@const r = row as NoticeListVo}
      {#if columnId === 'noticeType'}
        <DictTag dictType="sys_notice_type" value={r.noticeType} />
      {:else if columnId === 'status'}
        <DictTag dictType="sys_notice_status" value={r.status} />
      {:else}
        <div class="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            title="详情"
            aria-label="详情"
            onclick={() => openDetailFromRow(r)}
          >
            <Eye class="size-4" />
          </Button>
          <HasPermi perm="system:notice:edit">
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
          <HasPermi perm="system:notice:remove">
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
  title={isEdit ? '编辑公告' : '新增公告'}
  loading={saving}
  class="sm:max-w-[720px]"
  onConfirm={handleConfirm}
>
  <div class="flex flex-col gap-3">
    <Label for="noticeTitle">公告标题</Label>
    <Input
      id="noticeTitle"
      bind:value={form.noticeTitle}
      placeholder="请输入公告标题"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="noticeType">公告类型</Label>
    <select id="noticeType" class={SELECT_CLASS} bind:value={form.noticeType}>
      <option value="1">通知</option>
      <option value="2">公告</option>
    </select>
  </div>
  <div class="flex flex-col gap-3">
    <Label>公告内容</Label>
    <RichTextEditor
      bind:value={form.noticeContent}
      placeholder="请输入公告内容"
    />
  </div>
  <div class="flex flex-col gap-3">
    <Label for="status">状态</Label>
    <select id="status" class={SELECT_CLASS} bind:value={form.status}>
      <option value="0">正常</option>
      <option value="1">关闭</option>
    </select>
  </div>
</FormDrawer>

<Dialog.Root bind:open={detailOpen}>
  <Dialog.Content class="sm:max-w-[640px]">
    <Dialog.Header>
      <Dialog.Title>公告详情</Dialog.Title>
      <Dialog.Description class="sr-only">只读查看公告内容</Dialog.Description>
    </Dialog.Header>
    <div class="space-y-4">
      <div class="text-lg font-semibold">{detail?.noticeTitle || '-'}</div>
      <div
        class="text-muted-foreground flex flex-wrap items-center gap-4 text-sm"
      >
        <span class="flex items-center gap-1">
          类型：<DictTag
            dictType="sys_notice_type"
            value={detail?.noticeType ?? ''}
          />
        </span>
        <span class="flex items-center gap-1">
          状态：<DictTag
            dictType="sys_notice_status"
            value={detail?.status ?? ''}
          />
        </span>
        <span>创建者：{detail?.createByName || '-'}</span>
        <span>创建时间：{detail?.createTime || '-'}</span>
      </div>
      <hr class="border-border" />
      <!-- NTC2:正文必须走 SafeHtml(sanitizeHtml 白名单) —— 仓库护栏 html-contract.test.ts 只允许 SafeHtml 组件渲染原始 HTML -->
      <SafeHtml html={detail?.noticeContent ?? ''} />
    </div>
    <Dialog.Footer>
      <Dialog.Close>关闭</Dialog.Close>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除公告「${deleteTarget?.noticeTitle ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>
