import type { FlowNode, SkipEdge } from './types.js'

export interface Graph {
  nodes: FlowNode[]
  edges: SkipEdge[]
}

/**
 * 历史栈与剪贴板用的图快照:只留数据字段(id / 种类 / 位置 / 两端 / data),
 * 去掉 selected / dragging / measured 这些 xyflow 运行时字段,再深拷贝——
 * 快照里的 data.json 不能与画布上的对象共享引用,否则面板改一个字段会连历史一起改掉。
 */
export function cloneGraph(g: Graph): Graph {
  return structuredClone({
    nodes: g.nodes.map(({ id, type, position, data }) => ({
      id,
      type,
      position,
      data,
    })),
    edges: g.edges.map(
      ({ id, source, target, sourceHandle, targetHandle, type, data }) => ({
        id,
        source,
        target,
        sourceHandle,
        targetHandle,
        type,
        data,
      })
    ),
  })
}
