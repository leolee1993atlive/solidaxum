<script lang="ts">
  import { ossApi } from '$lib/api/system/oss.js'
  import { profileApi } from '$lib/api/system/profile.js'
  import { Camera } from '@lucide/svelte'
  import { toast } from 'svelte-sonner'

  let {
    avatarUrl,
    onUploaded,
  }: { avatarUrl?: string; onUploaded: () => void } = $props()

  let uploading = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()

  const ACCEPT = ['image/jpeg', 'image/png', 'image/gif', 'image/webp']
  const MAX_BYTES = 5 * 1024 * 1024

  async function onPick(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0]
    if (!file) return
    if (!ACCEPT.includes(file.type)) {
      toast.error('只能上传图片文件（jpg / png / gif / webp）')
      return
    }
    if (file.size > MAX_BYTES) {
      toast.error('图片大小不能超过 5MB')
      return
    }
    uploading = true
    try {
      const resp = await ossApi.upload(file)
      const oss =
        (resp as { data?: { ossId: string }; ossId?: string }).data ?? resp
      await profileApi.update({ avatar: (oss as { ossId: string }).ossId })
      onUploaded()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : '头像上传失败')
    } finally {
      uploading = false
      if (fileInput) fileInput.value = ''
    }
  }
</script>

<div class="mb-4 flex flex-col items-center gap-2">
  <label
    class="group relative cursor-pointer"
    aria-label={uploading ? '头像上传中' : '更换头像'}
  >
    {#if avatarUrl}
      <img
        src={avatarUrl}
        alt="头像"
        class="h-24 w-24 rounded-full border object-cover"
      />
    {:else}
      <div
        class="bg-muted text-muted-foreground flex h-24 w-24 items-center justify-center rounded-full border text-xs"
      >
        无头像
      </div>
    {/if}
    <!-- hover 蒙层:悬停或上传中显形 -->
    <div
      class="absolute inset-0 flex flex-col items-center justify-center gap-1 rounded-full bg-black/50 text-[11px] text-white opacity-0 transition-opacity group-hover:opacity-100"
      class:opacity-100={uploading}
    >
      <Camera class="h-4 w-4" />
      {uploading ? '上传中...' : '更换头像'}
    </div>
    <input
      bind:this={fileInput}
      type="file"
      accept="image/*"
      class="hidden"
      disabled={uploading}
      onchange={onPick}
    />
  </label>
</div>
