import { describe, expect, it } from 'vitest'
import { cascadeDeleteUrl, collectMenuTreeIds } from './menu.js'

describe('cascadeDeleteUrl', () => {
  // 后端 menu_route.rs:26-29 提供 DELETE /system/menu/cascade/{menu_ids},
  // 单删对有子节点的菜单会被 has_child 拒绝。
  it('单个 id', () => {
    expect(cascadeDeleteUrl('5')).toBe('/system/menu/cascade/5')
  })
  it('逗号分隔多个 id', () => {
    expect(cascadeDeleteUrl('5,6,7')).toBe('/system/menu/cascade/5,6,7')
  })
})

describe('collectMenuTreeIds', () => {
  // Java 排除语义要求子孙同批在删除集合内,只传父 id 会被拒。
  it('收集目标及全部子孙 id', () => {
    const node = {
      menuId: '1',
      children: [{ menuId: '2', children: [{ menuId: '3' }] }, { menuId: '4' }],
    }
    expect(collectMenuTreeIds(node)).toEqual(['1', '2', '3', '4'])
  })
  it('叶子节点只含自身', () => {
    expect(collectMenuTreeIds({ menuId: '5' })).toEqual(['5'])
  })
})
