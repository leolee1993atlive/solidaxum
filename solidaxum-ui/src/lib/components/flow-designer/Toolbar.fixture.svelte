<script lang="ts">
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import Toolbar from './Toolbar.svelte'
  import type { DesignerStore } from './state/designer.svelte.js'

  let {
    store,
    readonly = false,
    onSave = () => {},
    onClose = () => {},
  }: {
    store: DesignerStore
    readonly?: boolean
    onSave?: () => void
    onClose?: () => void
  } = $props()
</script>

<SvelteFlowProvider>
  <Toolbar {store} {readonly} {onSave} {onClose} />
</SvelteFlowProvider>
