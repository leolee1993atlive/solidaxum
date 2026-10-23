<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { apiClient, TOKEN_KEY, getTokenClientId } from '$lib/api/client.js'

  let iframeUrl = $state('')

  function buildUrl(): string {
    const base = apiClient.getBaseUrl()
    const id = page.url.searchParams.get('definitionId') ?? page.url.searchParams.get('id') ?? ''
    const token = typeof localStorage !== 'undefined' ? (localStorage.getItem(TOKEN_KEY) ?? '') : ''
    const clientId = getTokenClientId()
    const disabled = page.url.searchParams.get('disabled') ?? 'false'
    const qs = new URLSearchParams({
      id,
      // disabled=true → 设计器只读(已发布的流程不允许改)
      disabled,
      onlyDesignShow: 'true',
      // 后端 /warm-flow/* 已收 auth:token/clientid
      // 拼进 iframe URL,设计器 JS 按 config 端点 tokenNameList 机制放进
      // 请求头,后端 auth 从 URL query 回读 Authorization(对齐 Vue
      // design.vue:25-26 形态)。token 为空仍拼(未登录本进不了该页,
      // 后端 401 兜底)。
      Authorization: `Bearer ${token}`,
      clientid: clientId,
    })
    return `${base}/warm-flow-ui/index.html?${qs.toString()}`
  }

  function onMessage(event: MessageEvent) {
    if (event.data?.method === 'close') {
      const activeName = page.url.searchParams.get('activeName') ?? '0'
      goto(`/workflow/processDefinition?activeName=${activeName}`)
    }
  }

  onMount(() => {
    iframeUrl = buildUrl()
    window.addEventListener('message', onMessage)
  })
  onDestroy(() => {
    if (typeof window !== 'undefined') window.removeEventListener('message', onMessage)
  })
</script>

<div class="h-[calc(100vh-120px)] w-full overflow-hidden">
  {#if iframeUrl}
    <iframe title="流程设计器" src={iframeUrl} class="h-full w-full border-0"></iframe>
  {/if}
</div>
