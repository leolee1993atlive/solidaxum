<script lang="ts">
  import { onMount } from 'svelte'
  import { connectionStore } from '$lib/stores/connection.js'
  import { AlertCircle, Wifi, WifiOff } from '@lucide/svelte'

  let showAlert = $derived($connectionStore.status === 'disconnected')

  onMount(() => {
    connectionStore.init()

    return () => {
      connectionStore.destroy()
    }
  })

  function handleRetry() {
    connectionStore.checkConnection()
  }
</script>

{#if showAlert}
  <div
    class="bg-destructive text-destructive-foreground fixed top-0 right-0 left-0 z-[100] px-4 py-2 shadow-lg"
    role="alert"
  >
    <div class="flex items-center justify-center gap-3">
      <WifiOff class="h-4 w-4 shrink-0" />
      <span class="text-sm font-medium">
        无法连接到后端服务，请检查服务是否运行
      </span>
      <button
        class="ml-2 text-xs underline opacity-90 hover:no-underline hover:opacity-100"
        onclick={handleRetry}
      >
        重试
      </button>
    </div>
  </div>
{/if}
