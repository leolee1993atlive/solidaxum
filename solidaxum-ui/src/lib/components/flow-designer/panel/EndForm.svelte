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

<NodeCommonForm {store} {node} {readonly} />
