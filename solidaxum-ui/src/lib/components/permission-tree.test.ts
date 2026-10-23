import { describe, expect, it } from 'vitest'
import {
  buildPermissionTree,
  fromTreeSelectNodes,
  nodeCheckState,
  toggleNode,
  toggleButton,
  submitIds,
  allSelectableIds,
  isAllSelected,
} from './permission-tree.js'
import type { MenuTreeSelectVo } from '$lib/api/system/menu.js'

/** 目录1 ─ 菜单11(按钮111/112) ─ 菜单12(停用) */
const menus: MenuTreeSelectVo[] = [
  {
    id: '1',
    label: '系统管理',
    parentId: '0',
    weight: 1,
    menuType: 'M',
    visible: '0',
    status: '0',
    children: [
      {
        id: '11',
        label: '用户管理',
        parentId: '1',
        weight: 1,
        menuType: 'C',
        visible: '0',
        status: '0',
        children: [
          { id: '111', label: '用户新增', parentId: '11', weight: 1, menuType: 'F', visible: '0', status: '0' },
          { id: '112', label: '用户删除', parentId: '11', weight: 2, menuType: 'F', visible: '0', status: '1' },
        ],
      },
      {
        id: '12',
        label: '隐藏菜单',
        parentId: '1',
        weight: 2,
        menuType: 'C',
        visible: '1',
        status: '0',
      },
    ],
  },
]

const tree = buildPermissionTree(menus)

describe('buildPermissionTree', () => {
  it('F 类子节点抽成 buttons,不出现在 children 里', () => {
    const userMenu = tree[0].children[0]
    expect(userMenu.children).toHaveLength(0)
    expect(userMenu.buttons.map((b) => b.id)).toEqual(['111', '112'])
  })

  it('停用按钮标 disabled', () => {
    expect(tree[0].children[0].buttons[1].disabled).toBe(true)
  })

  it('visible=1 标 hidden(仅展示用,不影响可勾选)', () => {
    expect(tree[0].children[1].hidden).toBe(true)
    expect(tree[0].children[1].disabled).toBe(false)
  })

  it('停用目录使子孙节点与按钮一并 disabled', () => {
    const disabledDir: MenuTreeSelectVo[] = [
      {
        ...menus[0],
        status: '1',
      },
    ]
    const t = buildPermissionTree(disabledDir)
    expect(t[0].disabled).toBe(true)
    expect(t[0].children[0].disabled).toBe(true)
    expect(t[0].children[0].buttons[0].disabled).toBe(true)
  })
})

describe('回填(不级联)', () => {
  it('checkedKeys 只勾父节点时,子节点仍为未选中', () => {
    const checked = new Set(['1'])
    expect(nodeCheckState(tree[0], checked)).toBe('checked')
    expect(nodeCheckState(tree[0].children[0], checked)).toBe('unchecked')
  })

  it('只勾子节点时父显示半选', () => {
    const checked = new Set(['11'])
    expect(nodeCheckState(tree[0], checked)).toBe('indeterminate')
  })
})

describe('toggleNode(父子联动)', () => {
  it('勾目录 → 子菜单与其可用按钮全部进集合(停用按钮除外)', () => {
    const next = toggleNode(tree, new Set(), '1', true, { linked: true })
    expect([...next].sort((a, b) => a.length - b.length || a.localeCompare(b))).toEqual(['1', '11', '12', '111'])
  })

  it('取消勾选一个子菜单 → 父从 checked 移除(变半选)', () => {
    const all = toggleNode(tree, new Set(), '1', true, { linked: true })
    const next = toggleNode(tree, all, '11', false, { linked: true })
    expect(next.has('11')).toBe(false)
    expect(next.has('111')).toBe(false)
    expect(next.has('1')).toBe(false)
    expect(nodeCheckState(tree[0], next)).toBe('indeterminate')
  })

  it('联动关闭时只切换自身', () => {
    const next = toggleNode(tree, new Set(), '1', true, { linked: false })
    expect([...next]).toEqual(['1'])
  })
})

describe('toggleButton', () => {
  it('联动开时勾满某菜单全部可用按钮 → 该菜单自动进 checked', () => {
    const next = toggleButton(tree, new Set(), '111', true, { linked: true })
    expect(next.has('111')).toBe(true)
    expect(next.has('11')).toBe(true)
  })

  it('停用按钮不可勾选', () => {
    const next = toggleButton(tree, new Set(), '112', true, { linked: true })
    expect(next.has('112')).toBe(false)
  })
})

describe('submitIds', () => {
  it('联动开:半选祖先随选中集一起提交', () => {
    const checked = new Set(['11'])
    expect(submitIds(tree, checked, { linked: true })).toEqual(['1', '11'])
  })

  it('联动关:只有按钮的祖先链被补进来,菜单的祖先不补', () => {
    const checked = new Set(['11', '111'])
    expect(submitIds(tree, checked, { linked: false })).toEqual(['1', '11', '111'])
    expect(submitIds(tree, new Set(['11']), { linked: false })).toEqual(['11'])
  })
})

describe('全选', () => {
  it('allSelectableIds 排除 disabled 项', () => {
    expect(allSelectableIds(tree)).toEqual(['1', '11', '12', '111'])
  })

  it('isAllSelected 在全部可选项都选中时为 true', () => {
    expect(isAllSelected(tree, new Set(['1', '11', '12', '111']))).toBe(true)
    expect(isAllSelected(tree, new Set(['1', '11']))).toBe(false)
  })
})

describe('fromTreeSelectNodes', () => {
  it('部门树映射为无按钮的权限树节点', () => {
    const depts = fromTreeSelectNodes([
      { id: '100', label: '总公司', children: [{ id: '101', label: '研发部' }] },
    ])
    expect(depts[0].buttons).toEqual([])
    expect(depts[0].children[0].id).toBe('101')
    expect(depts[0].disabled).toBe(false)
  })
})
