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
  <NodeHandles />
  <NodeLabel text={data.json.nodeName ?? ''} offset={data.labelOffset} />
</div>
