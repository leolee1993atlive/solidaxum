<script lang="ts">
  import type { DesignerStore } from '../state/designer.svelte.js'
  import BetweenForm from './BetweenForm.svelte'
  import EndForm from './EndForm.svelte'
  import GatewayForm from './GatewayForm.svelte'
  import SkipForm from './SkipForm.svelte'
  import StartForm from './StartForm.svelte'

  // 按选中元素路由到表单;{#key} 让换一个元素时表单整体重建,表单内的草稿态不会串
  let {
    store,
    readonly = false,
  }: { store: DesignerStore; readonly?: boolean } = $props()

  const node = $derived(store.selectedNode)
  const edge = $derived(store.selectedEdge)
  const title = $derived(
    edge
      ? '跳转'
      : node?.type === 'between'
        ? '审批节点'
        : node?.type === 'gateway'
          ? '网关'
          : node?.type === 'start'
            ? '开始节点'
            : node?.type === 'end'
              ? '结束节点'
              : '流程'
  )
</script>

<aside class="w-80 shrink-0 overflow-auto border-l p-3 text-sm">
  <div class="text-muted-foreground mb-2 text-xs">{title}</div>
  {#if edge}
    {#key edge.id}<SkipForm {store} {edge} {readonly} />{/key}
  {:else if node}
    {#key node.id}
      {#if node.type === 'between'}
        <BetweenForm {store} {node} {readonly} />
      {:else if node.type === 'gateway'}
        <GatewayForm {store} {node} {readonly} />
      {:else if node.type === 'start'}
        <StartForm {store} {node} {readonly} />
      {:else}
        <EndForm {store} {node} {readonly} />
      {/if}
    {/key}
  {:else if store.base}
    <div class="font-medium">{store.base.flowName}</div>
    <div class="text-muted-foreground mt-1 text-xs">
      {store.base.flowCode} · v{store.base.version}
    </div>
    <p class="text-muted-foreground mt-3 text-xs">
      流程编码、名称、分类与表单路径在"流程定义"列表页维护;这里只编辑节点与跳转。点画布上的节点或线开始编辑。
    </p>
  {/if}
</aside>
