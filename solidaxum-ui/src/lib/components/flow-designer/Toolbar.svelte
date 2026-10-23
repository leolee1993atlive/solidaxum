<script lang="ts">
  import { Button } from '$lib/components/ui/button/index.js'
  import { Redo2, Undo2 } from '@lucide/svelte'
  import { useSvelteFlow } from '@xyflow/svelte'
  import type { DesignerStore } from './state/designer.svelte.js'

  // 必须放在 <SvelteFlowProvider> 之内:fitView 来自 hook。加节点的入口在左侧 Palette。
  let {
    store,
    readonly = false,
    onSave,
    onClose,
  }: {
    store: DesignerStore
    readonly?: boolean
    onSave: () => void
    onClose: () => void
  } = $props()

  const { fitView } = useSvelteFlow()
</script>

<div class="flex flex-wrap items-center gap-2 border-b px-3 py-2">
  {#if !readonly}
    <Button
      variant="outline"
      size="icon"
      title="撤销 (Ctrl+Z)"
      aria-label="撤销"
      disabled={!store.canUndo}
      onclick={() => store.undo()}><Undo2 class="h-4 w-4" /></Button
    >
    <Button
      variant="outline"
      size="icon"
      title="重做 (Ctrl+Y)"
      aria-label="重做"
      disabled={!store.canRedo}
      onclick={() => store.redo()}><Redo2 class="h-4 w-4" /></Button
    >
    <span class="mx-1 h-5 border-l"></span>
  {/if}
  <Button variant="outline" size="sm" onclick={() => fitView()}>适应画布</Button
  >
  <span class="flex-1"></span>
  {#if store.warnings.length}
    <span class="text-destructive text-xs">{store.warnings.join(';')}</span>
  {/if}
  {#if !readonly}
    <Button size="sm" disabled={store.saving || store.loading} onclick={onSave}
      >{store.saving ? '保存中…' : '保存'}</Button
    >
  {/if}
  <Button variant="ghost" size="sm" onclick={onClose}>关闭</Button>
</div>
