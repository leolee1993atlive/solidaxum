import type { Edge, Node } from '@xyflow/svelte'
import { describe, expectTypeOf, it } from 'vitest'
import type {
  DefJson,
  FlowNode,
  FlowNodeData,
  NodeJson,
  SkipEdge,
  SkipEdgeData,
  SkipJson,
} from './types.js'

// 这些断言在 pnpm check(svelte-check)里做编译期校验;vitest 下是空跑用例。
describe('flow types 与 @xyflow/svelte 的兼容性', () => {
  it('节点/边 data 满足 Svelte Flow 的 Record<string, unknown> 约束', () => {
    expectTypeOf<FlowNodeData>().toExtend<Record<string, unknown>>()
    expectTypeOf<SkipEdgeData>().toExtend<Record<string, unknown>>()
  })

  it('FlowNode / SkipEdge 可直接喂给 <SvelteFlow nodes edges>', () => {
    expectTypeOf<FlowNode>().toExtend<Node>()
    expectTypeOf<SkipEdge>().toExtend<Edge>()
  })
})

describe('DefJson 契约(对齐 def_json.rs)', () => {
  it('一份最小但字段齐全的定义能通过类型检查', () => {
    const skip = {
      nowNodeCode: 'start',
      nextNodeCode: 'end',
      skipName: null,
      skipType: 'PASS',
      skipCondition: null,
      coordinate: '300,225;300,300',
    } satisfies SkipJson

    const node: NodeJson = {
      nodeType: 0,
      nodeCode: 'start',
      nodeName: '开始',
      permissionFlag: null,
      nodeRatio: null,
      coordinate: '300,200',
      anyNodeSkip: null,
      listenerType: null,
      listenerPath: null,
      formCustom: null,
      formPath: null,
      ext: null,
      skipList: [skip],
    }

    const def: DefJson = {
      id: '1761300000000000001',
      flowCode: 'leave',
      flowName: '请假',
      modelValue: 'CLASSICS',
      category: null,
      version: '1',
      isPublish: 0,
      formCustom: null,
      formPath: null,
      listenerType: null,
      listenerPath: null,
      ext: null,
      createBy: null,
      updateBy: null,
      nodeList: [node],
    }

    expectTypeOf(def.id).toEqualTypeOf<string | null | undefined>()
    expectTypeOf(def.nodeList).items.toEqualTypeOf<NodeJson>()
    expectTypeOf(skip.skipType).toExtend<'PASS' | 'REJECT' | 'NONE' | null>()
  })
})
