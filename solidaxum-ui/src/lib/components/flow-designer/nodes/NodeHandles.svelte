<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte'

  // 四侧句柄全部是 source 类型,画布用 loose 模式让任意句柄互连——
  // 方向由边的 source→target 决定,句柄只表示挂点位置,与 LogicFlow 锚点等价。
  let {
    connectableStart = true,
    connectableEnd = true,
  }: { connectableStart?: boolean; connectableEnd?: boolean } = $props()
</script>

<Handle
  type="source"
  id="top"
  position={Position.Top}
  isConnectableStart={connectableStart}
  isConnectableEnd={connectableEnd}
/>
<Handle
  type="source"
  id="right"
  position={Position.Right}
  isConnectableStart={connectableStart}
  isConnectableEnd={connectableEnd}
/>
<Handle
  type="source"
  id="bottom"
  position={Position.Bottom}
  isConnectableStart={connectableStart}
  isConnectableEnd={connectableEnd}
/>
<Handle
  type="source"
  id="left"
  position={Position.Left}
  isConnectableStart={connectableStart}
  isConnectableEnd={connectableEnd}
/>
