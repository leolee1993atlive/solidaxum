import { describe, expect, it } from 'vitest'
import type { TreeSelectNode } from './tree-select-types.js'
import {
  pruneSubtrees,
  findNodeLabel,
  flattenVisible,
  toTreeSelectNodes,
} from './tree-select-utils.js'

const tree: TreeSelectNode[] = [
  {
    id: '1',
    label: '总公司',
    children: [
      { id: '2', label: '研发部', children: [{ id: '4', label: '前端组' }] },
      { id: '3', label: '市场部' },
    ],
  },
]

describe('pruneSubtrees', () => {
  it('排除某节点时,连同其全部子孙一并移除(防循环挂载)', () => {
    const out = pruneSubtrees(tree, [2])
    const ids = flattenVisible(out).map((n) => n.id)
    expect(ids).toContain('1')
    expect(ids).toContain('3')
    expect(ids).not.toContain('2') // 自身
    expect(ids).not.toContain('4') // 子孙 —— 只删自身会留下可选的子节点,仍能造成循环
  })

  it('排除列表为空时原样返回', () => {
    expect(flattenVisible(pruneSubtrees(tree, [])).length).toBe(4)
  })
})

describe('findNodeLabel', () => {
  it('能在任意深度找到节点名称用于回显', () => {
    expect(findNodeLabel(tree, 4)).toBe('前端组')
  })
  it('找不到返回 undefined', () => {
    expect(findNodeLabel(tree, 999)).toBeUndefined()
  })
})

describe('toTreeSelectNodes', () => {
  it('把 buildTree 产物(保留原始行字段)递归映射为 {id,label,children}', () => {
    // dept 页 buildTree 产物的真实形状:节点上挂的是业务字段,不是 id/label
    const rows = [
      {
        deptId: 1,
        deptName: '总公司',
        children: [{ deptId: 2, deptName: '研发部', children: [] }],
      },
    ]
    expect(
      toTreeSelectNodes(rows, { idKey: 'deptId', labelKey: 'deptName' })
    ).toEqual([
      { id: '1', label: '总公司', children: [{ id: '2', label: '研发部' }] },
    ])
  })
  it('空 children 不产出 children 键;缺 label 字段回退为空串', () => {
    expect(
      toTreeSelectNodes([{ menuId: 5 }] as Record<string, unknown>[], {
        idKey: 'menuId',
        labelKey: 'menuName',
      })
    ).toEqual([{ id: '5', label: '' }])
  })
})
