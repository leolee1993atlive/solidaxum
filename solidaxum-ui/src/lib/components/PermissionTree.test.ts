import { cleanup, render, screen, fireEvent } from '@testing-library/svelte'
import { describe, expect, it, vi, afterEach } from 'vitest'
import PermissionTree from './PermissionTree.svelte'
import type { PermissionNode } from './permission-tree.js'

afterEach(cleanup)

const nodes: PermissionNode[] = [
  {
    id: '1',
    label: '系统管理',
    disabled: false,
    hidden: false,
    buttons: [],
    children: [
      {
        id: '11',
        label: '用户管理',
        disabled: false,
        hidden: false,
        buttons: [
          { id: '111', label: '新增', disabled: false },
          { id: '112', label: '删除', disabled: true },
        ],
        children: [],
      },
    ],
  },
]

describe('PermissionTree', () => {
  it('渲染节点名称与按钮权限复选框', () => {
    render(PermissionTree, { props: { nodes, checked: new Set<string>(), linked: true } })
    expect(screen.getByText('系统管理')).toBeInTheDocument()
    expect(screen.getByLabelText('用户管理')).toBeInTheDocument()
    expect(screen.getByLabelText('新增')).toBeInTheDocument()
  })

  it('勾选目录时 onChange 收到级联后的集合', async () => {
    const onChange = vi.fn()
    render(PermissionTree, { props: { nodes, checked: new Set<string>(), linked: true, onChange } })
    await fireEvent.click(screen.getByLabelText('系统管理'))
    expect(onChange).toHaveBeenCalledTimes(1)
    expect([...onChange.mock.calls[0][0]].sort((a: string, b: string) => a.length - b.length || a.localeCompare(b))).toEqual(['1', '11', '111'])
  })

  it('停用按钮不可点击', async () => {
    const onChange = vi.fn()
    render(PermissionTree, { props: { nodes, checked: new Set<string>(), linked: true, onChange } })
    await fireEvent.click(screen.getByLabelText('删除'))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('「全选」勾上时把全部可选项交给 onChange', async () => {
    const onChange = vi.fn()
    render(PermissionTree, { props: { nodes, checked: new Set<string>(), linked: true, onChange } })
    await fireEvent.click(screen.getByLabelText('全选/全不选'))
    expect([...onChange.mock.calls[0][0]].sort((a: string, b: string) => a.length - b.length || a.localeCompare(b))).toEqual(['1', '11', '111'])
  })

  it('「父子联动」开关回调出新值', async () => {
    const onLinkedChange = vi.fn()
    render(PermissionTree, {
      props: { nodes, checked: new Set<string>(), linked: true, onLinkedChange },
    })
    await fireEvent.click(screen.getByLabelText('父子联动'))
    expect(onLinkedChange).toHaveBeenCalledWith(false)
  })

  it('showButtonColumn=false 时不渲染按钮列表头(数据权限树)', () => {
    render(PermissionTree, {
      props: { nodes, checked: new Set<string>(), linked: true, showButtonColumn: false },
    })
    expect(screen.queryByText('按钮权限')).not.toBeInTheDocument()
  })
})
