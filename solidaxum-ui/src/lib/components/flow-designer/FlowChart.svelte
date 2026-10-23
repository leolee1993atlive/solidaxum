<script lang="ts">
  import { isApiClientNotifiedError } from '$lib/api/client.js'
  import { designerApi } from '$lib/api/workflow/designer.js'
  import { defJsonToFlow } from '$lib/flow/deserialize.js'
  import type { FlowNode, SkipEdge } from '$lib/flow/types.js'
  import Canvas from './Canvas.svelte'

  /**
   * 实例流程图(规格 §10.2):query-flow-chart 返回实例启动时的定义快照,
   * 节点/跳转带 status,画布只读、按三原色着色。父容器要给高度。
   */
  let { instanceId }: { instanceId: string } = $props()

  let nodes = $state.raw<FlowNode[]>([])
  let edges = $state.raw<SkipEdge[]>([])
  let colors = $state<string[] | null>(null)
  let loading = $state(true)
  let error = $state<string | null>(null)

  async function load(id: string) {
    loading = true
    error = null
    try {
      const resp = await designerApi.queryFlowChart(id)
      if (!resp.data) throw new Error('流程图数据为空')
      const g = defJsonToFlow(resp.data)
      nodes = g.nodes
      edges = g.edges
      colors = resp.data.chartStatusColor ?? null
    } catch (e) {
      // apiClient 已经 toast 过的错误只在图区显示一行,不重复弹
      error =
        e instanceof Error && e.message && !isApiClientNotifiedError(e)
          ? e.message
          : '流程图加载失败'
      nodes = []
      edges = []
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void load(instanceId)
  })
</script>

<div class="relative h-full w-full">
  {#if loading}
    <div
      class="text-muted-foreground absolute inset-0 z-10 flex items-center justify-center text-sm"
    >
      加载中…
    </div>
  {:else if error}
    <div
      class="text-destructive absolute inset-0 z-10 flex items-center justify-center text-sm"
    >
      {error}
    </div>
  {/if}
  <Canvas bind:nodes bind:edges readonly statusColors={colors} />
</div>
