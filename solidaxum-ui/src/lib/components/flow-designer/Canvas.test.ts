import { defJsonToFlow } from '$lib/flow/deserialize.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import { NODE_SIZE } from '$lib/flow/geometry.js'
import type { DefJson, FlowNode, NodeKind } from '$lib/flow/types.js'
import { cleanup, render, waitFor } from '@testing-library/svelte'
import { Position } from '@xyflow/svelte'
import type { NodeHandle } from '@xyflow/svelte'
import { afterEach, describe, expect, it } from 'vitest'
import Canvas from './Canvas.svelte'

afterEach(cleanup)

/**
 * happy-dom 没有布局,xyflow 量不到节点尺寸与句柄位置就不画边;
 * 走它的 SSR 路径:节点自带 initialWidth/Height 与四侧句柄的静态几何。
 */
function staticLayout(nodes: FlowNode[]): FlowNode[] {
  return nodes.map((n) => {
    const { w, h } = NODE_SIZE[n.type as NodeKind]
    const at = (
      id: string,
      position: Position,
      x: number,
      y: number
    ): NodeHandle => ({
      id,
      type: 'source',
      position,
      x: x - 5,
      y: y - 5,
      width: 10,
      height: 10,
    })
    return {
      ...n,
      initialWidth: w,
      initialHeight: h,
      handles: [
        at('top', Position.Top, w / 2, 0),
        at('right', Position.Right, w, h / 2),
        at('bottom', Position.Bottom, w / 2, h),
        at('left', Position.Left, 0, h / 2),
      ],
    }
  })
}

function graph() {
  const g = defJsonToFlow(leave6Json as unknown as DefJson)
  return { nodes: staticLayout(g.nodes), edges: g.edges }
}

describe('Canvas readonly', () => {
  it('编辑态:边标签可拖(没有 fd-static)', async () => {
    const { nodes, edges } = graph()
    const { container } = render(Canvas, { props: { nodes, edges } })
    await waitFor(() =>
      expect(container.querySelectorAll('.fd-edge-label').length).toBe(2)
    )
    expect(container.querySelectorAll('.fd-edge-label.fd-static')).toHaveLength(
      0
    )
  })

  it('只读态:边标签锁住(fd-static),节点不可拖', async () => {
    const { nodes, edges } = graph()
    const { container } = render(Canvas, {
      props: { nodes, edges, readonly: true },
    })
    await waitFor(() =>
      expect(container.querySelectorAll('.fd-edge-label').length).toBe(2)
    )
    expect(container.querySelectorAll('.fd-edge-label.fd-static')).toHaveLength(
      2
    )
    expect(
      container.querySelectorAll('.svelte-flow__node.draggable')
    ).toHaveLength(0)
  })
})
