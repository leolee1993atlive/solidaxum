import { describe, expect, it } from 'vitest'
import { menuIconNames, filterIconNames, resolveMenuIcon } from './icon-map.js'

describe('menuIconNames', () => {
  // 🔴 选择器的选项必须是 resolveMenuIcon 认得的名字的全集 ——
  // 给一个映射表里没有的名字,用户选完侧边栏仍是默认方块。
  it('返回的每个名字都能解析出「非默认」图标', () => {
    const fallback = resolveMenuIcon('__definitely_not_mapped__')
    for (const name of menuIconNames()) {
      // LayoutGrid 是回落值;'system' 本身就映射到 LayoutGrid,单独放行
      if (name === 'system') continue
      expect(resolveMenuIcon(name)).not.toBe(fallback)
    }
  })

  it('按字母序稳定排序(网格顺序不能每次渲染都变)', () => {
    const names = menuIconNames()
    expect(names).toEqual([...names].sort())
  })

  it('不重复', () => {
    const names = menuIconNames()
    expect(new Set(names).size).toBe(names.length)
  })

  // 实测库里在用但曾不在映射表里的 4 个(§1 事实 2)。
  it.each(['input', 'form', 'upload', 'dashboard'])(
    '覆盖库里在用的 %s',
    (name) => {
      expect(menuIconNames()).toContain(name)
    }
  )

  // 实测库里在用的其余高频名,抽查几个,防止有人误删映射。
  it.each(['system', 'user', 'peoples', 'monitor', 'workflow', 'tool'])(
    '覆盖 %s',
    (name) => {
      expect(menuIconNames()).toContain(name)
    }
  )
})

describe('filterIconNames', () => {
  it('空关键词返回全部', () => {
    expect(filterIconNames('')).toEqual(menuIconNames())
    expect(filterIconNames('   ')).toEqual(menuIconNames())
  })
  it('子串匹配', () => {
    expect(filterIconNames('tree')).toEqual(['tree', 'tree-table'])
  })
  it('大小写不敏感', () => {
    expect(filterIconNames('USER')).toContain('user')
  })
  it('无命中返回空数组', () => {
    expect(filterIconNames('zzzzz')).toEqual([])
  })
})
