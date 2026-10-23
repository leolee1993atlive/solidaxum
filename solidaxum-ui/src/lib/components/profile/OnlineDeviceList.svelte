<script lang="ts">
  import { onlineApi } from '$lib/api/monitor/online.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { toast } from 'svelte-sonner'

  let devices = $state<Array<Record<string, unknown>>>([])
  let loading = $state(true)
  let pendingTokenId = $state<string | null>(null)

  async function load() {
    loading = true
    try {
      const resp = await onlineApi.currentUserInfo()
      const raw = (resp.data ?? resp) as unknown
      devices = (Array.isArray(raw) ? raw : [raw]) as Record<string, unknown>[]
    } finally {
      loading = false
    }
  }

  async function doLogout() {
    if (!pendingTokenId) return
    await onlineApi.deleteMySession(pendingTokenId)
    toast.success('已下线该设备')
    pendingTokenId = null
    await load()
  }

  $effect(() => {
    void load()
  })
</script>

<div class="mt-4 overflow-hidden rounded-lg border">
  <Table.Root>
    <Table.Header>
      <Table.Row>
        <Table.Head>登录地址</Table.Head>
        <Table.Head>浏览器</Table.Head>
        <Table.Head>操作系统</Table.Head>
        <Table.Head>登录时间</Table.Head>
        <Table.Head class="text-center">操作</Table.Head>
      </Table.Row>
    </Table.Header>
    <Table.Body>
      {#if loading}
        <Table.Row>
          <Table.Cell
            colspan={5}
            class="text-muted-foreground py-6 text-center"
          >
            加载中...
          </Table.Cell>
        </Table.Row>
      {:else if devices.length === 0}
        <Table.Row>
          <Table.Cell
            colspan={5}
            class="text-muted-foreground py-6 text-center"
          >
            暂无在线设备。
          </Table.Cell>
        </Table.Row>
      {:else}
        {#each devices as d (d.tokenId)}
          <Table.Row>
            <Table.Cell>
              {d.ipaddr ?? '-'}{d.loginLocation ? ` (${d.loginLocation})` : ''}
            </Table.Cell>
            <Table.Cell>{d.browser ?? '-'}</Table.Cell>
            <Table.Cell>{d.os ?? '-'}</Table.Cell>
            <Table.Cell>{d.loginTime ?? '-'}</Table.Cell>
            <Table.Cell class="text-center">
              <Button
                size="sm"
                variant="ghost"
                onclick={() => (pendingTokenId = String(d.tokenId))}
              >
                下线
              </Button>
            </Table.Cell>
          </Table.Row>
        {/each}
      {/if}
    </Table.Body>
  </Table.Root>
</div>

<ConfirmDialog
  open={pendingTokenId !== null}
  onOpenChange={(v) => {
    if (!v) pendingTokenId = null
  }}
  title="确认下线"
  desc="是否确认下线该设备？下线后该设备需要重新登录。"
  handleConfirm={doLogout}
/>
