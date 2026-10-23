<script lang="ts">
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import Canvas from './Canvas.svelte'
  import { setDesignerContext } from './state/context.js'
  import type { DesignerStore } from './state/designer.svelte.js'

  // 测试用:像 Designer 一样设好 context 与 Provider,只放画布
  let {
    store,
    readonly = false,
  }: { store: DesignerStore; readonly?: boolean } = $props()

  // 测试里 store 不会换,取初值即可
  // svelte-ignore state_referenced_locally
  setDesignerContext(store)
</script>

<SvelteFlowProvider>
  <Canvas bind:nodes={store.nodes} bind:edges={store.edges} {readonly} />
</SvelteFlowProvider>
