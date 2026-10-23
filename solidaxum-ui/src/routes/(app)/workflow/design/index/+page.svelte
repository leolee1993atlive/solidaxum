<script lang="ts">
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import Designer from '$lib/components/flow-designer/Designer.svelte'
  import Main from '$lib/components/layout/Main.svelte'

  // 入口 processDefinition gotoDesign():definitionId / disabled / activeName
  const definitionId = $derived(
    page.url.searchParams.get('definitionId') ??
      page.url.searchParams.get('id') ??
      ''
  )
  const readonly = $derived(page.url.searchParams.get('disabled') === 'true')

  function close() {
    const activeName = page.url.searchParams.get('activeName') ?? '0'
    goto(`/workflow/processDefinition?activeName=${activeName}`)
  }
</script>

<!-- 与其他页一样坐在 <Main> 的 content-card 白卡里(青绿下外框由主题抹平,靠白底
     与渐变底的反差分层)。高度钉在视口减顶栏+标签栏,让画布吃满剩余空间;
     fluid 不吃 max-w-7xl,p-4 比常规页的 py-6 紧一档,多留 16px 给画布。 -->
<Main fluid class="flex h-[calc(100vh-120px)] flex-col p-4">
  <div
    data-slot="content-card"
    class="bg-card flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border"
  >
    {#if definitionId}
      <Designer {definitionId} {readonly} onClose={close} />
    {:else}
      <div class="text-muted-foreground p-6 text-sm">
        缺少 definitionId 参数
      </div>
    {/if}
  </div>
</Main>
