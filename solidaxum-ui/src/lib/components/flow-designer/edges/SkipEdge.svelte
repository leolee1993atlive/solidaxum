<script lang="ts">
  import { statusClass } from '$lib/flow/chart.js'
  import {
    anchorPoint,
    nodeKindOf,
    positionToCenter,
  } from '$lib/flow/geometry.js'
  import { midpoint, orthogonalRoute } from '$lib/flow/route.js'
  import type { Point, Side, SkipEdge, SkipEdgeData } from '$lib/flow/types.js'
  import {
    BaseEdge,
    EdgeLabel,
    EdgeReconnectAnchor,
    useInternalNode,
    useStore,
    useSvelteFlow,
  } from '@xyflow/svelte'
  import type { EdgeProps, InternalNode } from '@xyflow/svelte'
  import { getDesignerContext } from '../state/context.js'

  // edgeTypes 的注册签名里 data 是可选的,这里给一个空边兜底;真实的边总带 data
  type Props = Omit<EdgeProps<SkipEdge>, 'data'> & { data?: SkipEdgeData }
  const EMPTY: SkipEdgeData = {
    waypoints: null,
    labelOffset: { x: 0, y: 0 },
    json: { skipName: '', skipType: 'PASS', skipCondition: null },
  }

  let {
    id,
    source,
    target,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    data: dataProp,
    selected,
    markerEnd,
  }: Props = $props()
  const data = $derived(dataProp ?? EMPTY)

  const { updateEdge, getViewport } = useSvelteFlow()
  const store = useStore()
  // 有 designer(不是实例流程图)且画布可拖(不是只读定义)才是可编辑态
  const designer = getDesignerContext()
  const editable = $derived(!!designer && store.nodesDraggable)
  // 一条边的两端不会变(重连是另一条边),取初始值即可
  // svelte-ignore state_referenced_locally
  const srcNode = useInternalNode(source)
  // svelte-ignore state_referenced_locally
  const tgtNode = useInternalNode(target)

  // 线头不用 xyflow 给的 sourceX/sourceY(那是句柄 DOM 的外沿,随句柄尺寸漂移),
  // 而用节点实时位置 + 规格里的锚点公式——画出来的点就是保存时写出的点。
  function anchorOf(
    n: InternalNode | undefined,
    side: Side,
    fallback: Point
  ): Point {
    if (!n) return fallback
    const kind = nodeKindOf(
      Number((n.data as { json: { nodeType: number } }).json.nodeType)
    )
    return anchorPoint(
      positionToCenter(n.internals.positionAbsolute, kind),
      kind,
      side
    )
  }

  // 存量折线走数据里的中间点;null 则按两端句柄侧自动正交路由。
  // 两端点单独暴露,选中态的重连锚要用
  const ends = $derived({
    s: anchorOf(srcNode.current, sourcePosition as Side, {
      x: sourceX,
      y: sourceY,
    }),
    t: anchorOf(tgtNode.current, targetPosition as Side, {
      x: targetX,
      y: targetY,
    }),
  })
  const points = $derived(
    data.waypoints === null
      ? orthogonalRoute(
          ends.s,
          sourcePosition as Side,
          ends.t,
          targetPosition as Side
        )
      : [ends.s, ...data.waypoints, ends.t]
  )
  const path = $derived(
    points.map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x} ${p.y}`).join(' ')
  )
  const mid = $derived(midpoint(points))
  const label = $derived(
    data.json.skipName || shortCondition(data.json.skipCondition)
  )
  const kind = $derived((data.json.skipType ?? 'PASS').toLowerCase())
  const status = $derived(statusClass(data.json.status))
  // 面板拖入时鼠标正压在这条边上:松手会插到这里
  const dropTarget = $derived(designer?.dropTargetEdgeId === id)

  /** 无名字的条件边显示条件的简写,只用于显示,不写回数据。 */
  function shortCondition(cond: string | null): string {
    if (!cond) return ''
    const body = cond.includes('@@') ? cond.slice(cond.indexOf('@@') + 2) : cond
    return body.length > 24 ? `${body.slice(0, 24)}…` : body
  }

  // 标签拖拽:屏幕位移按缩放换算成画布位移,累加到 labelOffset。
  // 阈值 3px 以内视为点击 → 选中边;拖动期间阻止事件冒泡,免得触发边选中/画布平移。
  let drag: { x: number; y: number; offset: Point; moved: boolean } | null =
    null

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return
    e.stopPropagation()
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    drag = {
      x: e.clientX,
      y: e.clientY,
      offset: { ...data.labelOffset },
      moved: false,
    }
  }

  function onpointermove(e: PointerEvent) {
    // 只读画布(nodesDraggable=false)标签也锁住;pointerup 里的点选仍然生效
    if (!drag || !store.nodesDraggable) return
    const dxScreen = e.clientX - drag.x
    const dyScreen = e.clientY - drag.y
    if (!drag.moved && Math.hypot(dxScreen, dyScreen) < 3) return
    drag.moved = true
    const zoom = getViewport().zoom
    const labelOffset = {
      x: drag.offset.x + dxScreen / zoom,
      y: drag.offset.y + dyScreen / zoom,
    }
    updateEdge(id, (edge) => ({ data: { ...edge.data, labelOffset } }))
  }

  function onpointerup(e: PointerEvent) {
    if (!drag) return
    ;(e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId)
    if (drag.moved) designer?.commit()
    else store.handleEdgeSelection(id)
    drag = null
  }
</script>

<BaseEdge
  {id}
  {path}
  {markerEnd}
  class="fd-skip fd-skip-{kind} {status} {dropTarget ? 'fd-drop-target' : ''}"
/>

<!-- 选中且可编辑时两端出现重连锚:拖到别的节点 / 句柄上换端点,条件跟着边走(Canvas.onbeforereconnect) -->
{#if selected && editable}
  <EdgeReconnectAnchor
    type="source"
    position={ends.s}
    size={14}
    class="fd-reconnect-anchor"
  />
  <EdgeReconnectAnchor
    type="target"
    position={ends.t}
    size={14}
    class="fd-reconnect-anchor"
  />
{/if}

{#if label}
  <EdgeLabel
    x={mid.x + data.labelOffset.x}
    y={mid.y + data.labelOffset.y}
    class="fd-edge-label nopan {status} {selected
      ? 'selected'
      : ''} {store.nodesDraggable ? '' : 'fd-static'}"
    data-edge-id={id}
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    onclick={(e: MouseEvent) => e.stopPropagation()}
  >
    {label}
  </EdgeLabel>
{/if}
