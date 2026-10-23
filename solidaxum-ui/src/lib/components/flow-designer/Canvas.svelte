<script lang="ts">
  import { statusColorStyle } from '$lib/flow/chart.js'
  import {
    clearWaypointsAround,
    newSkipEdge,
    reconnectEdge,
  } from '$lib/flow/edit.js'
  import type { FlowNode, Point, SkipEdge } from '$lib/flow/types.js'
  import {
    Background,
    ConnectionMode,
    Controls,
    MarkerType,
    SvelteFlow,
  } from '@xyflow/svelte'
  import type { Connection, IsValidConnection } from '@xyflow/svelte'
  import '@xyflow/svelte/dist/style.css'
  import CanvasEditor from './CanvasEditor.svelte'
  import './canvas.css'
  import ConnectionLine from './edges/ConnectionLine.svelte'
  import SkipEdgeView from './edges/SkipEdge.svelte'
  import BetweenNode from './nodes/BetweenNode.svelte'
  import EndNode from './nodes/EndNode.svelte'
  import GatewayNode from './nodes/GatewayNode.svelte'
  import StartNode from './nodes/StartNode.svelte'
  import { getDesignerContext } from './state/context.js'

  let {
    nodes = $bindable(),
    edges = $bindable(),
    readonly = false,
    statusColors = null,
  }: {
    nodes: FlowNode[]
    edges: SkipEdge[]
    readonly?: boolean
    /** 实例流程图的三原色 [done, todo, notDone];定义态为 null(用缺省色,且节点没有 status 也不会着色) */
    statusColors?: readonly string[] | null
  } = $props()

  // 有 store(Designer 设了 context)且非只读才是可编辑态;FlowChart 没有 store
  const store = getDesignerContext()
  const editor = $derived(store && !readonly ? store : undefined)
  let host = $state<HTMLDivElement>()

  const nodeTypes = {
    start: StartNode,
    between: BetweenNode,
    end: EndNode,
    gateway: GatewayNode,
  }
  const edgeTypes = { skip: SkipEdgeView }

  function kindOf(id: string) {
    return nodes.find((n) => n.id === id)?.type
  }

  // 自环、连入开始、连出结束都不合法;句柄上的 isConnectableStart/End 是第二道保险
  const isValidConnection: IsValidConnection<SkipEdge> = (c) =>
    c.source !== c.target &&
    kindOf(c.source) !== 'end' &&
    kindOf(c.target) !== 'start'

  // 自己构造新边并返回 false,绕开 addEdge 对同一对句柄的去重(同向多边是合法流程)
  function onbeforeconnect(c: Connection): false {
    edges = [...edges, newSkipEdge(c, edges)]
    editor?.commit()
    return false
  }

  // 重连:换端点、保留条件(edit.ts);没变化返回 false,xyflow 不动数据。
  // 提交放在 onreconnect —— 它在 xyflow 写完 edges 之后才触发,返回 false 时不触发
  function onbeforereconnect(newEdge: SkipEdge, oldEdge: SkipEdge) {
    return reconnectEdge(edges, oldEdge, {
      source: newEdge.source,
      target: newEdge.target,
      sourceHandle: newEdge.sourceHandle ?? null,
      targetHandle: newEdge.targetHandle ?? null,
    })
  }

  // 拖动前的位置:dragstop 时与之比较,没挪动(阈值内松手)不记历史
  let dragStart = new Map<string, Point>()

  function onnodedragstart({ nodes: dragged }: { nodes: FlowNode[] }) {
    if (!editor) return
    editor.interacting = true
    dragStart = new Map(dragged.map((n) => [n.id, { ...n.position }]))
  }

  function onnodedragstop({ nodes: dragged }: { nodes: FlowNode[] }) {
    edges = clearWaypointsAround(
      edges,
      dragged.map((n) => n.id)
    )
    if (!editor) return
    editor.interacting = false
    const moved = dragged.some((n) => {
      const p = dragStart.get(n.id)
      return !p || p.x !== n.position.x || p.y !== n.position.y
    })
    if (moved) editor.commit()
  }

  // 连线与重连共用 connectstart / connectend(重连锚在两者之外还会再发 reconnectstart/end)
  function onconnectstart() {
    if (editor) editor.interacting = true
  }

  function onconnectend() {
    if (editor) editor.interacting = false
  }
</script>

<div class="fd-canvas" style={statusColorStyle(statusColors)} bind:this={host}>
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    {edgeTypes}
    connectionMode={ConnectionMode.Loose}
    connectionLineComponent={ConnectionLine}
    snapGrid={[10, 10]}
    {isValidConnection}
    {onbeforeconnect}
    {onbeforereconnect}
    onreconnect={() => {
      // 重连收尾不触发 connectend,interacting 要在这里自己复位
      if (editor) {
        editor.interacting = false
        editor.commit()
      }
    }}
    onreconnectend={() => {
      if (editor) editor.interacting = false
    }}
    ondelete={() => editor?.commit()}
    {onnodedragstart}
    {onnodedragstop}
    {onconnectstart}
    {onconnectend}
    defaultEdgeOptions={{ markerEnd: { type: MarkerType.ArrowClosed } }}
    deleteKey={readonly ? null : ['Delete', 'Backspace']}
    nodesDraggable={!readonly}
    nodesConnectable={!readonly}
    fitView
    minZoom={0.2}
  >
    <Background />
    <Controls showLock={false} />
    {#if editor}
      <CanvasEditor store={editor} {host} />
    {/if}
  </SvelteFlow>
</div>
