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
      <svg
        class="fd-palette-icon fd-palette-{item.kind === 'gateway'
          ? item.nodeType === 4
            ? 'parallel'
            : item.nodeType === 5
              ? 'inclusive'
              : 'serial'
          : item.kind}"
        viewBox="0 0 28 28"
        aria-hidden="true"
      >
        {#if item.kind === 'between'}
          <!-- 蓝卡 + 审批小图标(照 sidebarIcons betweenIcon) -->
          <rect x="3" y="6" width="22" height="16" rx="4" class="fd-palette-shape" />
          <g class="fd-palette-mini">
            <rect x="6.5" y="9" width="10" height="8.5" rx="2" />
            <circle cx="10" cy="12.2" r="1.8" />
            <path d="M9 12.4 l1.2 1.2 l2.2 -2.2" />
          </g>
        {:else if item.kind === 'gateway'}
          <!-- 菱形 = 旋转圆角矩形(rx≈5/50 坐标系)+ 符号 path + 圆点 -->
          <rect
            x="4.5"
            y="4.5"
            width="19"
            height="19"
            rx="2.8"
            transform="rotate(45 14 14)"
            class="fd-palette-shape"
          />
          {#if item.nodeType === 4}
            <path d="M8.5 14 L19.5 14" stroke-width="1.7" />
            <path d="M14 8.5 L14 19.5" stroke-width="1.7" />
            <circle cx="14" cy="7" r="0.85" fill="currentColor" />
            <circle cx="21" cy="14" r="0.85" fill="currentColor" />
            <circle cx="14" cy="21" r="0.85" fill="currentColor" />
            <circle cx="7" cy="14" r="0.85" fill="currentColor" />
          {:else if item.nodeType === 5}
            <circle cx="14" cy="14" r="6.2" fill="none" stroke="currentColor" stroke-width="1.2" />
            <circle
              cx="14"
              cy="14"
              r="3.1"
              fill="currentColor"
              fill-opacity="0.15"
              stroke="currentColor"
              stroke-width="0.9"
            />
            <circle cx="14" cy="14" r="1" fill="currentColor" />
          {:else}
            <path d="M9.5 9.5 L18.5 18.5 M18.5 9.5 L9.5 18.5" stroke-width="1.5" />
            <circle cx="8.1" cy="8.1" r="0.75" fill="currentColor" />
            <circle cx="19.9" cy="8.1" r="0.75" fill="currentColor" />
            <circle cx="19.9" cy="19.9" r="0.75" fill="currentColor" />
            <circle cx="8.1" cy="19.9" r="0.75" fill="currentColor" />
          {/if}
        {:else if item.kind === 'end'}
          <!-- 红双环:粗外环 + 细内环 -->
          <circle cx="14" cy="14" r="11" class="fd-palette-shape" />
          <circle cx="14" cy="14" r="7.5" class="fd-palette-ring" />
        {:else}
          <!-- 绿环 + 实内核 -->
          <circle cx="14" cy="14" r="11" class="fd-palette-shape" />
          <circle cx="14" cy="14" r="7" class="fd-palette-core" />
        {/if}
      </svg>
      <span>{item.label}</span>
    </div>
  {/each}
</aside>
