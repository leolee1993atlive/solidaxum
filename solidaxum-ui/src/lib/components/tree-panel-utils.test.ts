import { describe, expect, it } from 'vitest'
import { filterTreeByLabel } from './tree-panel-utils.js'
import type { TreeSelectNode } from './tree-select-types.js'

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

describe('filterTreeByLabel', () => {
  it('关键字为空时原样返回', () => {
    expect(filterTreeByLabel(tree, '')).toEqual(tree)
  })

  it('命中节点保留其祖先链', () => {
    const out = filterTreeByLabel(tree, '市场')
    expect(out).toHaveLength(1)
    expect(out[0].label).toBe('总公司')
    expect(out[0].children?.map((n) => n.label)).toEqual(['市场部'])
  })

  it('命中节点的子孙全部保留', () => {
    const out = filterTreeByLabel(tree, '研发')
    expect(out[0].children?.[0].children?.map((n) => n.label)).toEqual(['前端组'])
  })

  it('无命中时返回空数组', () => {
    expect(filterTreeByLabel(tree, '不存在')).toEqual([])
  })
})
