import { describe, expect, it } from 'vitest'
import { filterTreeNodes } from './tree-filter.js'

type N = { name: string; status?: string; children?: N[] }

const tree: N[] = [
  {
    name: '系统管理',
    status: '0',
    children: [
      { name: '用户管理', status: '0' },
      { name: '角色管理', status: '1' },
    ],
  },
  { name: '监控页', status: '1' },
]

describe('filterTreeNodes', () => {
  it('谓词恒真时原样返回同一引用(不做无谓拷贝)', () => {
    expect(filterTreeNodes(tree, () => true)).toBe(tree)
  })

  it('命中子节点时保留其祖先链', () => {
    const out = filterTreeNodes(tree, (n: N) => n.name === '用户管理')
    expect(out).toHaveLength(1)
    expect(out[0].name).toBe('系统管理')
    expect(out[0].children?.map((c) => c.name)).toEqual(['用户管理'])
  })

  it('父命中而子不命中时,子分支被剪掉(不整棵带出来)', () => {
    const out = filterTreeNodes(tree, (n: N) => n.name === '系统管理')
    expect(out[0].children).toEqual([])
  })

  it('全不命中返回空数组', () => {
    expect(filterTreeNodes(tree, (n: N) => n.name === '不存在')).toEqual([])
  })

  it('不改动入参(原树的 children 不被就地替换)', () => {
    filterTreeNodes(tree, (n: N) => n.name === '用户管理')
    expect(tree[0].children).toHaveLength(2)
  })
})
