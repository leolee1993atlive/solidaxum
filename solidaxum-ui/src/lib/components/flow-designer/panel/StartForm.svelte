<script lang="ts">
  import { formatListeners, parseListeners } from '$lib/flow/node-fields.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'
  import ListenerRows from './ListenerRows.svelte'
  import NodeCommonForm from './NodeCommonForm.svelte'

  let {
    store,
    node,
    readonly = false,
  }: { store: DesignerStore; node: FlowNode; readonly?: boolean } = $props()

  const rows = $derived(
    parseListeners(node.data.json.listenerType, node.data.json.listenerPath)
  )
</script>

<NodeCommonForm {store} {node} {readonly} />

<div class="mt-4">
  <div class="text-muted-foreground mb-1 text-xs">监听器</div>
  <ListenerRows
    {rows}
    {readonly}
    onChange={(next) => store.updateNodeJson(node.id, formatListeners(next))}
  />
</div>
