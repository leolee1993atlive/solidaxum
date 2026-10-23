<script lang="ts">
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import Palette from './Palette.svelte'
  import type { DesignerStore } from './state/designer.svelte.js'

  let { store }: { store: DesignerStore } = $props()
</script>

<SvelteFlowProvider>
  <Palette {store} />
</SvelteFlowProvider>
