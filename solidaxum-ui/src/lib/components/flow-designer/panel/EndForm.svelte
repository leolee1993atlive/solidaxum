<script lang="ts">
  import type { FlowNode } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'
  import NodeCommonForm from './NodeCommonForm.svelte'

  let {
    store,
    node,
    readonly = false,
  }: { store: DesignerStore; node: FlowNode; readonly?: boolean } = $props()
</script>

<section class="fd-section">
  <h3 class="fd-section-title">基础配置</h3>
  <NodeCommonForm {store} {node} {readonly} />
</section>
