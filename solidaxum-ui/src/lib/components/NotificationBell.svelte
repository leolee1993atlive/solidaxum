<script lang="ts">
  import { goto } from '$app/navigation'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js'
  import { createSseClient } from '$lib/stores/sse.svelte.js'
  import { Bell } from '@lucide/svelte'
  import { onMount, onDestroy } from 'svelte'

  const sse = createSseClient()
  onMount(() => sse.connect())
  onDestroy(() => sse.close())

  function open(msg: { path?: string | null }) {
    sse.markAllRead()
    if (msg.path) goto(msg.path)
  }
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger
    class="focus-visible:ring-ring relative rounded-full focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
    aria-label="通知"
  >
    <Bell class="size-4" />
    {#if sse.unread > 0}
      <Badge class="absolute -right-1 -top-1 px-1 text-xs">{sse.unread}</Badge>
    {/if}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content align="end" class="w-80">
    {#if sse.messages.length === 0}
      <DropdownMenu.Item disabled>暂无通知</DropdownMenu.Item>
    {:else}
      {#each sse.messages as msg (msg.messageId ?? msg.timestamp)}
        <DropdownMenu.Item onclick={() => open(msg)}>
          <span class="truncate">{msg.message}</span>
        </DropdownMenu.Item>
      {/each}
    {/if}
  </DropdownMenu.Content>
</DropdownMenu.Root>
