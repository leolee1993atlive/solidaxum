<!-- src/routes/(app)/system/user/user-import-dialog.svelte -->
<script lang="ts">
  import { toast } from 'svelte-sonner'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import { userApi } from '$lib/api/system/user.js'
  import { UploadCloud } from '@lucide/svelte'

  let {
    open = $bindable(false),
    onImported,
  }: { open?: boolean; onImported?: () => void } = $props()

  let file = $state<File | null>(null)
  let uploading = $state(false)
  /** 导入结果:后端返回纯文本(含 \n),用 pre-wrap 原样展示。 */
  let result = $state<string | null>(null)

  function onFileChange(e: Event) {
    const input = e.target as HTMLInputElement
    file = input.files?.[0] ?? null
  }

  function onDrop(e: DragEvent) {
    e.preventDefault()
    const f = e.dataTransfer?.files?.[0]
    if (f) file = f
  }

  async function handleSubmit() {
    if (!file) {
      toast.error('请选择要导入的 xls / xlsx 文件')
      return
    }
    uploading = true
    try {
      const res = await userApi.importData(file)
      result = res.data ?? '导入完成'
      file = null
      onImported?.()
    } catch {
      // apiClient 已 toast
    } finally {
      uploading = false
    }
  }

  async function handleTemplate() {
    try {
      await userApi.importTemplate()
    } catch (e) {
      // downloadBlob 抛裸 Error,不经 apiClient。
      toast.error(e instanceof Error ? e.message : '模板下载失败')
    }
  }

  function close() {
    open = false
    file = null
    result = null
  }
</script>

<Dialog.Root
  bind:open
  onOpenChange={(v) => {
    if (!v) {
      file = null
      result = null
    }
  }}>
  <Dialog.Content class="sm:max-w-[520px]">
    <Dialog.Header><Dialog.Title>用户导入</Dialog.Title></Dialog.Header>

    {#if result}
      <div class="space-y-2">
        <p class="text-sm font-medium">导入结果</p>
        <!-- 后端返回的是纯文本(user_service.rs:597-601),不是 HTML,无需 sanitize -->
        <pre class="max-h-64 overflow-auto rounded-md bg-muted p-3 text-xs whitespace-pre-wrap">{result}</pre>
      </div>
      <Dialog.Footer>
        <Button onclick={close}>关闭</Button>
      </Dialog.Footer>
    {:else}
      <div class="space-y-3">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="flex flex-col items-center gap-2 rounded-md border border-dashed border-input p-6 text-center hover:border-ring"
          ondragover={(e) => e.preventDefault()}
          ondrop={onDrop}>
          <UploadCloud class="size-8 text-muted-foreground" />
          <p class="text-sm">将文件拖到此处，或</p>
          <input type="file" accept=".xls,.xlsx" class="text-sm" onchange={onFileChange} />
          {#if file}
            <p class="text-xs text-muted-foreground">已选择：{file.name}</p>
          {/if}
        </div>

        <label class="flex items-start gap-2 text-sm">
          <!-- ⚠️ 后端不消费 updateSupport:import_data 的 multipart 循环只认 name=="file"
               (user_handler.rs:711),service 遇到同名用户一律跳过(user_service.rs:551-560)。
               照 Vue 的位置渲染但禁用 + 就地说明,不做"点了没反应"的假开关。 -->
          <Checkbox disabled checked={false} />
          <span class="text-muted-foreground">
            是否更新已经存在的用户数据
            <br />（后端暂不支持，同名用户在导入时会被跳过）
          </span>
        </label>

        <p class="text-xs text-muted-foreground">
          仅允许导入 xls、xlsx 格式文件。模板列：用户序号 / 部门名称 / 用户账号 / 用户昵称 /
          用户邮箱 / 手机号码 / 用户性别 / 账号状态。
          <button class="text-primary-emphasis underline" type="button" onclick={handleTemplate}>
            下载模板
          </button>
        </p>
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={close}>取消</Button>
        <Button disabled={uploading || !file} onclick={handleSubmit}>
          {uploading ? '导入中…' : '确定'}
        </Button>
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
