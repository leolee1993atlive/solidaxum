<script lang="ts">
  import { parseSkipCondition } from '$lib/flow/condition.js'
  import type { FlowNode, SkipEdge } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'
  import NodeCommonForm from './NodeCommonForm.svelte'

  // 网关本身没有可编辑属性,分支逻辑在出边上;这里把全部出边列出来,点一下就选中那条边,
  // 是多出线网关整理条件的主要入口。
  let {
    store,
    node,
    readonly = false,
  }: { store: DesignerStore; node: FlowNode; readonly?: boolean } = $props()

  const outEdges = $derived(store.edges.filter((e) => e.source === node.id))

  function condText(e: SkipEdge): string {
    const c = parseSkipCondition(e.data.json.skipCondition)
    if (c.kind === 'none') return '无条件'
    if (c.kind === 'compare') return `${c.variable} ${c.op} ${c.value}`
    if (c.kind === 'spel') return `spel: ${c.expr}`
    return c.raw
  }
</script>

<NodeCommonForm {store} {node} {readonly} />

<div class="mt-4">
  <div class="text-muted-foreground mb-1 text-xs">出线({outEdges.length})</div>
  {#if outEdges.length === 0}
    <div class="text-muted-foreground text-sm">
      还没有出线,从菱形的句柄拖出去连到下一节点
    </div>
  {:else}
    <ul class="divide-y rounded-md border">
      {#each outEdges as e (e.id)}
        <li>
          <button
            type="button"
            class="hover:bg-muted flex w-full flex-col items-start px-2 py-1.5 text-left text-sm"
            onclick={() => store.selectEdge(e.id)}
          >
            <span class="font-medium">{e.data.json.skipName || '(未命名)'}</span
            >
            <span class="text-muted-foreground truncate text-xs"
              >{condText(e)}</span
            >
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
