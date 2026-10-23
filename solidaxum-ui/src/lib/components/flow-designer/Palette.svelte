<script lang="ts">
  import { newFlowNode } from '$lib/flow/edit.js'
  import { useStore, useSvelteFlow } from '@xyflow/svelte'
  import {
    isInsertable,
    NODE_DRAG_INSERTABLE,
    NODE_DRAG_MIME,
    NODE_MENU,
    type NodeMenuItem,
  } from './node-menu.js'
  import type { DesignerStore } from './state/designer.svelte.js'

  // Provider 子组件:点击落视口中心要 getViewport 与画布尺寸
  let { store }: { store: DesignerStore } = $props()

  const { getViewport } = useSvelteFlow()
  const flow = useStore()

  /** 点击 = 不想拖的快捷路径:落在当前视口中心,吸附 10px 网格(与画布 snapGrid 一致) */
  function add(item: NodeMenuItem) {
    const { x, y, zoom } = getViewport()
    const center = {
      x: Math.round((flow.width / 2 - x) / zoom / 10) * 10,
      y: Math.round((flow.height / 2 - y) / zoom / 10) * 10,
    }
    store.addNode(newFlowNode(item.kind, center, item.nodeType))
  }

  // Firefox 不给 <button> 起拖,所以面板项是 role=button 的 div;
  // 不 setData 的话 Firefox 同样不起拖
  function dragStart(e: DragEvent, item: NodeMenuItem) {
    if (!e.dataTransfer) return
    e.dataTransfer.setData(NODE_DRAG_MIME, JSON.stringify(item))
    if (isInsertable(item.kind))
      e.dataTransfer.setData(NODE_DRAG_INSERTABLE, '')
    e.dataTransfer.effectAllowed = 'copy'
  }
</script>

<aside class="fd-palette" aria-label="节点面板">
  {#each NODE_MENU as item (item.label)}
    <div
      class="fd-palette-item"
      role="button"
      tabindex="0"
      draggable="true"
      data-kind={item.kind}
      ondragstart={(e) => dragStart(e, item)}
      ondragend={() => (store.dropTargetEdgeId = null)}
      onclick={() => add(item)}
      onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          add(item)
        }
      }}
    >
      <svg class="fd-palette-icon" viewBox="0 0 28 28" aria-hidden="true">
        {#if item.kind === 'between'}
          <rect x="2" y="5" width="24" height="18" rx="4" />
        {:else if item.kind === 'gateway'}
          <polygon points="14,2 26,14 14,26 2,14" />
          <text x="14" y="15" text-anchor="middle" dominant-baseline="middle"
            >{item.nodeType === 4 ? '+' : item.nodeType === 5 ? 'O' : 'X'}</text
          >
        {:else}
          <circle
            cx="14"
            cy="14"
            r="11"
            class:fd-palette-end={item.kind === 'end'}
          />
        {/if}
      </svg>
      <span>{item.label}</span>
    </div>
  {/each}
</aside>
