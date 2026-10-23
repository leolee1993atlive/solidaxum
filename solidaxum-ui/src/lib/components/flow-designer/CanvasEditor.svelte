<script lang="ts">
  import { on } from 'svelte/events'
  import { newFlowNode } from '$lib/flow/edit.js'
  import { nodeKindOf } from '$lib/flow/geometry.js'
  import type { Point } from '$lib/flow/types.js'
  import { useSvelteFlow } from '@xyflow/svelte'
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte'
  import {
    isInsertable,
    NODE_DRAG_INSERTABLE,
    NODE_DRAG_MIME,
    NODE_MENU,
    type NodeMenuItem,
  } from './node-menu.js'
  import type { DesignerStore } from './state/designer.svelte.js'
  import { shortcutOf } from './state/shortcuts.js'

  /**
   * 画布的编辑层:面板拖入 / 落线插入、右键菜单、快捷键、Ctrl+V 的落点。
   * 必须作为 <SvelteFlow> 的子组件渲染——useSvelteFlow 的 store 上下文来自 SvelteFlow 本身,
   * 这样 Canvas 自己不必被 Provider 包住,FlowChart 与组件测试照旧直接渲染 Canvas。
   * host 是 Canvas 的外层容器,拖放 / 右键 / 指针跟踪都挂在它上面。
   */
  let { store, host }: { store: DesignerStore; host: HTMLElement | undefined } =
    $props()

  const { screenToFlowPosition, deleteElements, fitView } = useSvelteFlow()

  /** 指针在画布上的最后位置(屏幕坐标),Ctrl+V 的落点;不在画布上为 null */
  let pointer: Point | null = null
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null)

  // ── 面板拖入 ────────────────────────────────────────────────────────────

  function edgeUnder(e: DragEvent): string | null {
    // 拖影不在 DOM 里,elementFromPoint 命中的是拖影下面的真实元素;
    // xyflow 给每条边画了 20px 宽的透明交互路径,命中面够宽
    const el = document.elementFromPoint(e.clientX, e.clientY)
    return el?.closest('.svelte-flow__edge')?.getAttribute('data-id') ?? null
  }

  function ondragover(e: DragEvent) {
    const dt = e.dataTransfer
    if (!dt || !dt.types.includes(NODE_DRAG_MIME)) return
    e.preventDefault()
    dt.dropEffect = 'copy'
    const id = dt.types.includes(NODE_DRAG_INSERTABLE) ? edgeUnder(e) : null
    // dragover 每几十毫秒一次,只在候选边变化时写 store
    if (store.dropTargetEdgeId !== id) store.dropTargetEdgeId = id
  }

  function ondragleave(e: DragEvent) {
    // 在子元素之间移动也会触发 dragleave,只在真正离开容器时清掉
    if (host && !host.contains(e.relatedTarget as Node | null))
      store.dropTargetEdgeId = null
  }

  function ondrop(e: DragEvent) {
    const raw = e.dataTransfer?.getData(NODE_DRAG_MIME)
    if (!raw) return
    e.preventDefault()
    const item = JSON.parse(raw) as NodeMenuItem
    const edgeId = store.dropTargetEdgeId
    store.dropTargetEdgeId = null
    // screenToFlowPosition 按画布 snapGrid(10px)吸附;落点作为节点中心
    const node = newFlowNode(
      item.kind,
      screenToFlowPosition({ x: e.clientX, y: e.clientY }),
      item.nodeType
    )
    if (edgeId && isInsertable(item.kind)) {
      if (store.insertNodeOnEdge(edgeId, node) === null) return
    }
    store.addNode(node)
  }

  // ── 右键菜单 ────────────────────────────────────────────────────────────

  function nodeMenu(nodeId: string | null): MenuItem[] {
    // 右键未选中的节点 → 唯一选中;已在多选集合里 / 框选整体 → 保持集合
    if (nodeId && !store.nodes.find((n) => n.id === nodeId)?.selected)
      store.selectNode(nodeId)
    const selected = store.nodes.filter((n) => n.selected)
    const copyable = selected.some((n) => {
      const kind = nodeKindOf(n.data.json.nodeType)
      return kind !== 'start' && kind !== 'end'
    })
    return [
      {
        label: '复制',
        shortcut: 'Ctrl+C',
        disabled: !copyable,
        onselect: () => void store.copy(),
      },
      {
        label: selected.length > 1 ? `删除 ${selected.length} 个节点` : '删除',
        shortcut: 'Delete',
        // 走 xyflow 的 deleteElements:连带边一起删,并触发 ondelete → 提交历史
        onselect: () =>
          void deleteElements({ nodes: selected.map((n) => ({ id: n.id })) }),
      },
    ]
  }

  function edgeMenu(edgeId: string): MenuItem[] {
    store.selectEdge(edgeId)
    return [
      {
        label: '删除',
        shortcut: 'Delete',
        onselect: () => void deleteElements({ edges: [{ id: edgeId }] }),
      },
    ]
  }

  function paneMenu(at: Point): MenuItem[] {
    return [
      {
        label: '粘贴',
        shortcut: 'Ctrl+V',
        disabled: !store.clipboard,
        onselect: () => store.paste(at),
      },
      ...NODE_MENU.map((item, i) => ({
        label: `添加${item.label}`,
        separatorBefore: i === 0,
        onselect: () =>
          store.addNode(newFlowNode(item.kind, at, item.nodeType)),
      })),
      {
        label: '适应画布',
        separatorBefore: true,
        onselect: () => void fitView(),
      },
    ]
  }

  function oncontextmenu(e: MouseEvent) {
    const t = e.target as Element | null
    if (!host || !t || t.closest('.fd-context-menu, .svelte-flow__controls'))
      return
    e.preventDefault()
    const nodeEl = t.closest('.svelte-flow__node')
    // 边标签画在另一层(edgelabel-renderer),不在边的 <g> 里,靠 data-edge-id 找回边
    const edgeEl = t.closest('.svelte-flow__edge, .fd-edge-label')
    const r = host.getBoundingClientRect()
    let items: MenuItem[]
    if (nodeEl) items = nodeMenu(nodeEl.getAttribute('data-id'))
    else if (edgeEl)
      items = edgeMenu(
        edgeEl.getAttribute('data-id') ?? edgeEl.getAttribute('data-edge-id')!
      )
    else if (t.closest('.svelte-flow__selection-wrapper'))
      items = nodeMenu(null)
    else items = paneMenu(screenToFlowPosition({ x: e.clientX, y: e.clientY }))
    menu = { x: e.clientX - r.left, y: e.clientY - r.top, items }
  }

  // ── 快捷键 ──────────────────────────────────────────────────────────────

  function pasteTarget(): Point {
    if (pointer) return screenToFlowPosition(pointer)
    const r = host!.getBoundingClientRect()
    return screenToFlowPosition({
      x: r.left + r.width / 2,
      y: r.top + r.height / 2,
    })
  }

  function onkeydown(e: KeyboardEvent) {
    const action = shortcutOf(e)
    if (!action || !host) return
    e.preventDefault()
    if (action === 'undo') store.undo()
    else if (action === 'redo') store.redo()
    else if (action === 'copy') store.copy()
    else store.paste(pasteTarget())
  }

  $effect(() => {
    if (!host) return
    const offs = [
      on(host, 'dragover', ondragover),
      on(host, 'dragleave', ondragleave),
      on(host, 'drop', ondrop),
      on(host, 'contextmenu', oncontextmenu),
      on(
        host,
        'pointermove',
        (e) => (pointer = { x: e.clientX, y: e.clientY })
      ),
      on(host, 'pointerleave', () => (pointer = null)),
    ]
    return () => offs.forEach((off) => off())
  })
</script>

<svelte:window {onkeydown} />

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    items={menu.items}
    onclose={() => (menu = null)}
  />
{/if}
