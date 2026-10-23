import { describe, it, expect } from 'vitest'
import { buildTree } from './tree.js'

type Row = { id: number; parentId: number; name: string; children?: Row[] }

describe('buildTree', () => {
  it('nests flat rows by parent key', () => {
    const flat: Row[] = [
      { id: 1, parentId: 0, name: 'a' },
      { id: 2, parentId: 1, name: 'a-1' },
      { id: 3, parentId: 1, name: 'a-2' },
      { id: 4, parentId: 0, name: 'b' },
    ]
    const tree = buildTree(flat, {
      idKey: 'id',
      parentKey: 'parentId',
      rootValue: 0,
    })
    expect(tree.map((n) => n.name)).toEqual(['a', 'b'])
    expect(tree[0].children?.map((n) => n.name)).toEqual(['a-1', 'a-2'])
    expect(tree[1].children).toBeUndefined()
  })

  it('passes through already-nested rows unchanged when parentKey missing', () => {
    const nested: Row[] = [
      {
        id: 1,
        parentId: 0,
        name: 'a',
        children: [{ id: 2, parentId: 1, name: 'a-1' }],
      },
    ]
    const tree = buildTree(nested, {
      idKey: 'id',
      parentKey: 'parentId',
      rootValue: 0,
    })
    expect(tree).toHaveLength(1)
    expect(tree[0].children?.[0].name).toBe('a-1')
  })

  it('treats orphan rows (parent not present) as roots', () => {
    const flat: Row[] = [{ id: 5, parentId: 999, name: 'orphan' }]
    const tree = buildTree(flat, {
      idKey: 'id',
      parentKey: 'parentId',
      rootValue: 0,
    })
    expect(tree.map((n) => n.name)).toEqual(['orphan'])
  })
})
