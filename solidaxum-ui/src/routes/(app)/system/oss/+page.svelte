<script lang="ts">
  import { goto } from '$app/navigation'
  import { configApi } from '$lib/api/system/config.js'
  import { ossApi, isImageSuffix } from '$lib/api/system/oss.js'
  import type { SysOssVo } from '$lib/api/system/oss.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import QueryForm from '$lib/components/QueryForm.svelte'
  import { CrudTableShell } from '$lib/components/crud/index.js'
  import Main from '$lib/components/layout/Main.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import {
    fromListResult,
    useCrudTable,
  } from '$lib/composables/use-crud-table.svelte.js'
  import {
    Download,
    Trash2,
    Upload,
    Image as ImageIcon,
    Settings,
  } from '@lucide/svelte'
  import type { ColumnDef } from '@tanstack/table-core'
  import { toast } from 'svelte-sonner'

  // ---- 筛选(原名/后缀/服务商;不含时间范围) ----
  let filters = $state<{
    fileName?: string
    originalName?: string
    fileSuffix?: string
    service?: string
  }>({})

  const queryFields = [
    { key: 'fileName', label: '文件名', placeholder: '请输入文件名' },
    { key: 'originalName', label: '原名', placeholder: '请输入原名' },
    { key: 'fileSuffix', label: '文件后缀', placeholder: '请输入文件后缀' },
    { key: 'service', label: '服务商', placeholder: '请输入服务商' },
  ]

  // ---- columns ----
  // 不做「文件编号」(ossId) 列 —— Vue 用 v-if="false" 隐藏(index.vue:113)。
  // 含「创建时间」列(Vue sortable="custom",index.vue:129)。
  // 可排序列对齐 Vue —— createTime / service(index.vue:129,135)。
  const columns: ColumnDef<SysOssVo>[] = [
    {
      accessorKey: 'fileName',
      header: '文件名',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'originalName',
      header: '原文件名',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'fileSuffix',
      header: '文件类型',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    { id: 'fileView', header: '文件展示', cell: (info) => info.row.original },
    {
      accessorKey: 'createTime',
      header: '创建时间',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'createByName',
      header: '上传者',
      cell: (info) => (info.getValue() as string | undefined) || '-',
    },
    {
      accessorKey: 'service',
      header: '服务商',
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
    const resp = await ossApi.list(
      {
        pageNum: params.pageNo,
        pageSize: params.pageSize,
        // 排序必须真发出去:后端 build_order_by(page.rs:87)按驼峰键白名单映射到列名
        // (oss_service.rs:150-154 收 ossId/fileName/createTime),不发就是点了没反应。
        orderByColumn: params.sort,
        isAsc: params.order,
      },
      params.extra ?? {}
    )
    return fromListResult(resp)
  }

  const crudTable = useCrudTable<SysOssVo>({
    columns,
    // 可排序列对齐 Vue(createTime / service,见 index.vue:129,135)。
    // ⚠️ service 暂不可排序 —— 后端排序白名单只有 ossId/fileName/createTime
    // (oss_service.rs:150-154),传 service 会被 build_order_by 丢弃并回落 "oss_id desc"。
    // 标成可排序就是个点了没反应的表头,故这里只留 createTime。
    sortableIds: new Set(['createTime']),
    fetchPage,
    // getRowId 必须配,否则 enableRowSelection=false。
    getRowId: (row) => String(row.ossId),
    getExtraParams: () => ({
      fileName: filters.fileName,
      originalName: filters.originalName,
      fileSuffix: filters.fileSuffix,
      service: filters.service,
    }),
  })

  function handleSearch(model: Record<string, string>) {
    filters = {
      fileName: model.fileName,
      originalName: model.originalName,
      fileSuffix: model.fileSuffix,
      service: model.service,
    }
    crudTable.resetPagination()
  }
  function handleReset() {
    filters = {}
    crudTable.resetPagination()
  }

  // ---- 批量删除 ----
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
      // ossApi.delete 支持逗号分隔的多 ossId(oss.ts:48 / 后端 /resource/oss/{ossIds})。
      await ossApi.delete(ids.join(','))
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

  // ---- upload(弹窗内文件/图片分流,对齐 Vue index.vue:64-69,168-182) ----
  // Vue 用 fileUpload/imageUpload 双组件按 type 切换;本仓不引上传库,用原生 input + accept 区分。
  // 0=文件(任意类型),1=图片(accept=image/*)。
  let uploadDialogOpen = $state(false)
  let uploadType = $state<0 | 1>(0) // 0=文件,1=图片
  let selectedFile = $state<File | null>(null)
  let uploading = $state(false)

  const uploadTitle = $derived(uploadType === 1 ? '上传图片' : '上传文件')
  const uploadAccept = $derived(uploadType === 1 ? 'image/*' : undefined)

  function openUploadFile() {
    uploadType = 0
    selectedFile = null
    uploadDialogOpen = true
  }
  function openUploadImage() {
    uploadType = 1
    selectedFile = null
    uploadDialogOpen = true
  }

  function handleFileChange(e: Event) {
    const input = e.target as HTMLInputElement
    selectedFile = input.files && input.files.length > 0 ? input.files[0] : null
  }

  async function handleUpload() {
    if (!selectedFile) {
      toast.error('请先选择文件')
      return
    }
    uploading = true
    try {
      const resp = await ossApi.upload(selectedFile)
      toast.success(`上传成功:${resp.data?.fileName ?? selectedFile.name}`)
      // 清空已选文件并关闭弹窗
      selectedFile = null
      const inputEl = document.getElementById(
        'oss-upload-input'
      ) as HTMLInputElement | null
      if (inputEl) inputEl.value = ''
      uploadDialogOpen = false
      crudTable.resetPagination()
    } catch {
      // apiClient 已 toast
    } finally {
      uploading = false
    }
  }

  // ---- 配置管理跳转 ----
  function gotoOssConfig() {
    goto('/system/oss-config')
  }

  // ---- delete confirm ----
  let deleteOpen = $state(false)
  let deleting = $state(false)
  let deleteTarget = $state<SysOssVo | null>(null)

  function openDelete(row: SysOssVo) {
    deleteTarget = row
    deleteOpen = true
  }

  async function handleDelete() {
    if (!deleteTarget) return
    deleting = true
    try {
      await ossApi.delete(String(deleteTarget.ossId))
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

  // ---- preview toggle ----
  const PREVIEW_KEY = 'sys.oss.previewListResource'
  let previewEnabled = $state(true)
  $effect(() => {
    void (async () => {
      try {
        const v = await configApi.getByKey(PREVIEW_KEY)
        previewEnabled = String(v) === 'true'
      } catch {
        previewEnabled = true
      }
    })()
  })
  async function togglePreview() {
    const next = !previewEnabled
    await configApi.updateByKey(PREVIEW_KEY, String(next))
    previewEnabled = next
    toast.success(next ? '已启用预览' : '已禁用预览')
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <CrudTableShell
    table={crudTable.table}
    isLoading={crudTable.isLoading}
    error={crudTable.error}
    columnCount={columns.length}
    emptyMessage="未找到文件数据。"
    globalFilter=""
    showSearch={false}
    showRefresh={false}
    customCellIds={new Set(['fileView'])}
    selectable
    title="文件列表"
  >
    {#snippet toolbar({ selectedCount })}
      <!-- 上传文件 / 上传图片 两个按钮,分别开弹窗(对齐 Vue index.vue:64-69) -->
      <HasPermi perm="system:oss:upload">
        <Button variant="outline" size="sm" onclick={openUploadFile}>
          <Upload class="mr-1 h-4 w-4" />上传文件
        </Button>
      </HasPermi>
      <HasPermi perm="system:oss:upload">
        <Button variant="outline" size="sm" onclick={openUploadImage}>
          <ImageIcon class="mr-1 h-4 w-4" />上传图片
        </Button>
      </HasPermi>
      <!-- 预览开关(切换 sys.oss.previewListResource,与缩略图列联动) -->
      <HasPermi perm="system:oss:list">
        <Button
          size="sm"
          variant={previewEnabled ? 'destructive' : 'secondary'}
          onclick={togglePreview}
        >
          预览开关：{previewEnabled ? '禁用' : '启用'}
        </Button>
      </HasPermi>
      <!-- 配置管理跳转 -->
      <HasPermi perm="system:ossConfig:list">
        <Button variant="outline" size="sm" onclick={gotoOssConfig}>
          <Settings class="mr-1 h-4 w-4" />配置管理
        </Button>
      </HasPermi>
      <HasPermi perm="system:oss:remove">
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
      {@const r = row as SysOssVo}
      {#if columnId === 'fileView'}
        {@const url = r.url ?? ''}
        {#if previewEnabled && isImageSuffix(r.fileSuffix)}
          <a href={url} target="_blank" rel="noreferrer noopener">
            <img
              src={url}
              alt={r.originalName}
              class="h-[100px] w-[100px] rounded object-cover"
            />
          </a>
        {:else}
          <span class="text-muted-foreground text-xs break-all">{url}</span>
        {/if}
      {:else}
        <div class="flex items-center gap-1">
          <HasPermi perm="system:oss:download">
            <Button
              variant="ghost"
              size="icon"
              title="下载"
              aria-label="下载"
              onclick={() => ossApi.download(r.ossId, r.originalName)}
            >
              <Download class="size-4" />
            </Button>
          </HasPermi>
          <HasPermi perm="system:oss:remove">
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

<ConfirmDialog
  open={deleteOpen}
  onOpenChange={(v) => (deleteOpen = v)}
  title="确认删除"
  desc={`确定要删除文件「${deleteTarget?.fileName ?? ''}」吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={deleting}
  handleConfirm={handleDelete}
/>

<!-- 批量删除 -->
<ConfirmDialog
  open={batchDeleteOpen}
  onOpenChange={(v) => (batchDeleteOpen = v)}
  title="确认批量删除"
  desc={`确定要删除选中的 ${crudTable.selectedIds.length} 个文件吗?删除后不可恢复。`}
  confirmText="删除"
  destructive
  isLoading={batchDeleting}
  handleConfirm={handleBatchDelete}
/>

<!-- 上传弹窗(文件/图片按 uploadType 切 accept),提交即上传 -->
<Dialog.Root bind:open={uploadDialogOpen}>
  <Dialog.Content class="sm:max-w-[520px]">
    <Dialog.Header>
      <Dialog.Title>{uploadTitle}</Dialog.Title>
    </Dialog.Header>
    <div class="flex flex-col gap-3 px-2 pb-2">
      <Label for="oss-upload-input">文件</Label>
      <Input
        id="oss-upload-input"
        type="file"
        accept={uploadAccept}
        onchange={handleFileChange}
        disabled={uploading}
      />
      {#if selectedFile}
        <span class="text-muted-foreground text-sm"
          >已选择:{selectedFile.name}</span
        >
      {/if}
    </div>
    <Dialog.Footer>
      <Button
        variant="outline"
        onclick={() => (uploadDialogOpen = false)}
        disabled={uploading}>取消</Button
      >
      <Button onclick={handleUpload} disabled={uploading}>上传</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
