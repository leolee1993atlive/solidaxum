<script lang="ts">
  import { statusClass } from '$lib/flow/chart.js'
  import { NODE_SIZE } from '$lib/flow/geometry.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { NodeProps } from '@xyflow/svelte'
  import NodeHandles from './NodeHandles.svelte'
  import NodeLabel from './NodeLabel.svelte'

  let { data, selected }: NodeProps<FlowNode> = $props()
  const { w, h } = NODE_SIZE.gateway
  // 3 互斥 / 4 并行 / 5 包容:同一个菱形,用符号区分(warm-flow 1.8.9 sidebarIcons 的 50 坐标系 path)
  const kindClass = $derived(
    data.json.nodeType === 4
      ? 'fd-parallel'
      : data.json.nodeType === 5
        ? 'fd-inclusive'
        : 'fd-serial'
  )
</script>

<!-- 菱形画在 SVG 里而不是旋转盒子:盒子不转,四侧句柄才落在菱形顶点上 -->
<div
  class="fd-node fd-gateway {kindClass} {statusClass(data.json.status)}"
  class:selected
  style:width="{w}px"
  style:height="{h}px"
>
  <svg width={w} height={h} viewBox="0 0 {w} {h}" aria-hidden="true">
    <polygon points="{w / 2},1 {w - 1},{h / 2} {w / 2},{h - 1} 1,{h / 2}" />
    <g class="fd-glyph">
      {#if data.json.nodeType === 4}
        <!-- 并行:十字 -->
        <path d="M15 25 L35 25" stroke-width="3" />
        <path d="M25 15 L25 35" stroke-width="3" />
        <circle cx="25" cy="12" r="1.5" class="fd-glyph-dot" />
        <circle cx="38" cy="25" r="1.5" class="fd-glyph-dot" />
        <circle cx="25" cy="38" r="1.5" class="fd-glyph-dot" />
        <circle cx="12" cy="25" r="1.5" class="fd-glyph-dot" />
      {:else if data.json.nodeType === 5}
        <!-- 包容:同心双圆 + 中心点 -->
        <circle cx="25" cy="25" r="11" fill="none" stroke-width="2" />
        <circle cx="25" cy="25" r="5.5" class="fd-glyph-disc" stroke-width="1.5" />
        <circle cx="25" cy="25" r="1.8" class="fd-glyph-dot" />
      {:else}
        <!-- 互斥:X + 四端装饰圆点 -->
        <path d="M17 17 L33 33 M33 17 L17 33" stroke-width="2.5" />
        <circle cx="14.5" cy="14.5" r="1.3" class="fd-glyph-dot" />
        <circle cx="34.5" cy="14.5" r="1.3" class="fd-glyph-dot" />
        <circle cx="34.5" cy="34.5" r="1.3" class="fd-glyph-dot" />
        <circle cx="14.5" cy="34.5" r="1.3" class="fd-glyph-dot" />
      {/if}
    </g>
  </svg>
  <NodeHandles />
  <NodeLabel text={data.json.nodeName ?? ''} offset={data.labelOffset} />
</div>
