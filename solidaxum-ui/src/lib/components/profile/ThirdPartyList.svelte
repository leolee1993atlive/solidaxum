<script lang="ts">
  import { socialApi } from '$lib/api/system/social.js'
  import type { SocialVo } from '$lib/api/system/social.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Info } from '@lucide/svelte'
  import { toast } from 'svelte-sonner'

  let accounts = $state<SocialVo[]>([])
  let loading = $state(true)
  let pending = $state<SocialVo | null>(null)

  async function load() {
    loading = true
    try {
      const resp = await socialApi.list()
      accounts = (resp.data ?? resp) as SocialVo[]
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '加载三方账号失败')
    } finally {
      loading = false
    }
  }

  async function doUnlock() {
    if (!pending) return
    await socialApi.unlock(pending.id)
    toast.success('解绑成功')
    pending = null
    await load()
  }

  $effect(() => {
    void load()
  })
</script>

<div class="pt-4">
  <div
    class="bg-muted/50 text-muted-foreground mb-3 flex items-start gap-2 rounded-lg border p-3 text-xs"
  >
    <Info class="mt-0.5 h-3.5 w-3.5 shrink-0" />
    <span>
      本系统未启用社交登录(后端 <code>/auth/binding/&#123;source&#125;</code>
      返回 501),因此只能查看与解绑已有绑定,无法新增绑定。
    </span>
  </div>

  {#if loading}
    <div class="text-muted-foreground py-6 text-center text-sm">加载中...</div>
  {:else if accounts.length === 0}
    <div class="text-muted-foreground py-6 text-center text-sm">
      暂无绑定的三方账号。
    </div>
  {:else}
    <ul class="grid gap-2">
      {#each accounts as a (a.id)}
        <li class="flex items-center justify-between rounded-lg border p-3">
          <div class="flex min-w-0 items-center gap-3">
            {#if a.avatar}
              <img
                src={a.avatar}
                alt=""
                class="h-8 w-8 shrink-0 rounded-full"
              />
            {/if}
            <div class="min-w-0 text-sm">
              <div class="truncate font-medium">
                {a.nickName ?? a.userName ?? '-'}
              </div>
              <div class="text-muted-foreground truncate text-xs">
                {a.source ?? '-'} · 绑定于 {a.createTime ?? '-'}
              </div>
            </div>
          </div>
          <Button size="sm" variant="ghost" onclick={() => (pending = a)}
            >解绑</Button
          >
        </li>
      {/each}
    </ul>
  {/if}
</div>

<ConfirmDialog
  open={pending !== null}
  onOpenChange={(v) => {
    if (!v) pending = null
  }}
  title="确认解绑"
  desc={`是否确认解绑【${pending?.source ?? ''}】账号？`}
  handleConfirm={doUnlock}
/>
