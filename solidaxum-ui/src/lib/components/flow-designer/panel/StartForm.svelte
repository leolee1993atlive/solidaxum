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

<section class="fd-section">
  <h3 class="fd-section-title">基础配置</h3>
  <NodeCommonForm {store} {node} {readonly} />
</section>

<section class="fd-section">
  <h3 class="fd-section-title">监听器配置</h3>
  <ListenerRows
    {rows}
    {readonly}
    onChange={(next) => store.updateNodeJson(node.id, formatListeners(next))}
  />
</section>
