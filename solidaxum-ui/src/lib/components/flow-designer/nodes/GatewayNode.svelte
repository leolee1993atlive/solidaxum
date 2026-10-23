<script lang="ts">
  import { statusClass } from '$lib/flow/chart.js'
  import { NODE_SIZE } from '$lib/flow/geometry.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { NodeProps } from '@xyflow/svelte'
  import NodeHandles from './NodeHandles.svelte'
  import NodeLabel from './NodeLabel.svelte'

  let { data, selected }: NodeProps<FlowNode> = $props()
  const { w, h } = NODE_SIZE.gateway
  // 3 互斥 / 4 并行 / 5 包容:同一个菱形,用符号区分
  const glyph = $derived(
    data.json.nodeType === 4 ? '+' : data.json.nodeType === 5 ? 'O' : 'X'
  )
</script>

<!-- 菱形画在 SVG 里而不是旋转盒子:盒子不转,四侧句柄才落在菱形顶点上 -->
<div
  class="fd-node fd-gateway {statusClass(data.json.status)}"
  class:selected
  style:width="{w}px"
  style:height="{h}px"
>
  <svg width={w} height={h} viewBox="0 0 {w} {h}" aria-hidden="true">
    <polygon points="{w / 2},1 {w - 1},{h / 2} {w / 2},{h - 1} 1,{h / 2}" />
    <text
      x={w / 2}
      y={h / 2 + 1}
      text-anchor="middle"
      dominant-baseline="middle">{glyph}</text
    >
  </svg>
  <NodeHandles />
  <NodeLabel text={data.json.nodeName ?? ''} offset={data.labelOffset} />
</div>
