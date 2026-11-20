<script lang="ts">
  import { statusClass } from '$lib/flow/chart.js'
  import { NODE_SIZE } from '$lib/flow/geometry.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { NodeProps } from '@xyflow/svelte'
  import NodeHandles from './NodeHandles.svelte'
  import NodeLabel from './NodeLabel.svelte'

  let { data, selected }: NodeProps<FlowNode> = $props()
  const { w, h } = NODE_SIZE.between
</script>

<div
  class="fd-node fd-between {statusClass(data.json.status)}"
  class:selected
  style:width="{w}px"
  style:height="{h}px"
>
  <!-- 左上角审批图标(照 warm-flow 1.8.9 between.js getIconShape 转写,绝对定位不占布局) -->
  <svg class="fd-between-icon" viewBox="0 0 24 24" aria-hidden="true">
    <rect x="2" y="3" width="18" height="16" rx="3" class="fd-icon-card" />
    <circle cx="11" cy="10" r="3.5" class="fd-icon-badge" />
    <path d="M9 10 l2 2 l4 -4" stroke-linecap="round" stroke-linejoin="round" />
    <line x1="6" y1="16.5" x2="16" y2="16.5" class="fd-icon-line" stroke-linecap="round" />
  </svg>
  <NodeHandles />
  <NodeLabel text={data.json.nodeName ?? ''} offset={data.labelOffset} />
</div>
