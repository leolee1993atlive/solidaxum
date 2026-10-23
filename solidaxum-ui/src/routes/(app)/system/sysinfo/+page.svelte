<script lang="ts">
  import { onMount } from 'svelte'
  import { configApi } from '$lib/api/system/config.js'
  import { ossApi } from '$lib/api/system/oss.js'
  import Main from '$lib/components/layout/Main.svelte'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { ImagePlus, Loader2, Save } from '@lucide/svelte'
  import { toast } from 'svelte-sonner'

  // 锚:原 Java 版系统信息页 —— 自定义产品标题(公司全称+系统名)
  // 与图标(公司标)。存 sys_config 键 sys.title/sys.icon,顶栏/侧栏实时生效。
  const TITLE_KEY = 'sys.title'
  const ICON_KEY = 'sys.icon'

  const auth = useAuth()

  let title = $state('')
  let iconUrl = $state('')
  let loading = $state(true)
  let saving = $state(false)
  let uploading = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()

  const ACCEPT = ['image/jpeg', 'image/png', 'image/gif', 'image/webp']
  const MAX_BYTES = 10 * 1024 * 1024

  onMount(async () => {
    try {
      const resp = await configApi.sysInfo()
      title = resp.data?.title ?? ''
      iconUrl = resp.data?.icon ?? ''
    } catch (err) {
      toast.error(err instanceof Error ? err.message : '系统信息加载失败')
    } finally {
      loading = false
    }
  })

  async function onPickIcon(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0]
    if (!file) return
    if (!ACCEPT.includes(file.type)) {
      toast.error('只能上传图片文件（jpg / jpeg / png / gif / webp）')
      return
    }
    if (file.size > MAX_BYTES) {
      toast.error('图片大小不能超过 10MB')
      return
    }
    uploading = true
    try {
      const resp = await ossApi.upload(file)
      const oss =
        (resp as { data?: { url?: string }; url?: string }).data ?? resp
      iconUrl = (oss as { url: string }).url
    } catch (err) {
      toast.error(err instanceof Error ? err.message : '图标上传失败')
    } finally {
      uploading = false
      if (fileInput) fileInput.value = ''
    }
  }

  async function save() {
    saving = true
    try {
      await configApi.updateByKey(TITLE_KEY, title.trim())
      await configApi.updateByKey(ICON_KEY, iconUrl)
      // 顶栏/侧栏即时生效
      auth.sysTitle = title.trim()
      auth.sysIcon = iconUrl
      toast.success('保存成功')
    } catch (err) {
      toast.error(err instanceof Error ? err.message : '保存失败')
    } finally {
      saving = false
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <Card class="py-6">
    <CardHeader>
      <CardTitle>系统信息</CardTitle>
    </CardHeader>
    <CardContent class="max-w-xl space-y-6">
      {#if loading}
        <div class="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2 class="size-4 animate-spin" /> 加载中…
        </div>
      {:else}
        <div class="space-y-2">
          <Label class="gap-0.5" for="sys-title">
            <span class="text-destructive">*</span> 系统标题
          </Label>
          <Input
            id="sys-title"
            bind:value={title}
            placeholder="请输入系统标题，如：XX人民医院 | 不良事件管理系统"
          />
        </div>
        <div class="space-y-2">
          <Label class="gap-0.5">
            <span class="text-destructive">*</span> 系统图标
          </Label>
          <button
            type="button"
            class="bg-background border-input hover:border-ring flex size-24 cursor-pointer items-center justify-center overflow-hidden rounded-lg border border-dashed"
            onclick={() => fileInput?.click()}
            disabled={uploading}
            aria-label="上传系统图标"
          >
            {#if uploading}
              <Loader2 class="text-muted-foreground size-5 animate-spin" />
            {:else if iconUrl}
              <img src={iconUrl} alt="系统图标" class="size-full object-contain p-1.5" />
            {:else}
              <ImagePlus class="text-muted-foreground size-6" />
            {/if}
          </button>
          <p class="text-muted-foreground text-xs">
            请上传不超过 10MB 的 jpg/jpeg/png/gif/webp 格式文件
          </p>
          <input
            bind:this={fileInput}
            type="file"
            accept={ACCEPT.join(',')}
            class="hidden"
            onchange={onPickIcon}
          />
        </div>
        <HasPermi perm="system:config:edit">
          <Button onclick={save} disabled={saving}>
            {#if saving}<Loader2 class="size-4 animate-spin" />{:else}<Save class="size-4" />{/if}
            保存
          </Button>
        </HasPermi>
      {/if}
    </CardContent>
  </Card>
</Main>
