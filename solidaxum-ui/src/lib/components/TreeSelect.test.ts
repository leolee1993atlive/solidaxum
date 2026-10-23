import { cleanup, render, screen, fireEvent } from '@testing-library/svelte'
import { describe, expect, it, vi, afterEach } from 'vitest'
import TreeSelect from './TreeSelect.svelte'
import type { TreeSelectNode } from './tree-select-types.js'

afterEach(cleanup)

const tree: TreeSelectNode[] = [
  { id: '1', label: '总公司', children: [{ id: '2', label: '研发部' }] },
]

describe('TreeSelect', () => {
  it('未选中时显示占位文案', () => {
    render(TreeSelect, {
      props: { treeData: tree, placeholder: '请选择归属部门' },
    })
    expect(screen.getByText('请选择归属部门')).toBeInTheDocument()
  })

  it('给定 value 时回显节点名称(而不是裸 id)', () => {
    render(TreeSelect, { props: { treeData: tree, value: 2 } })
    expect(screen.getByText('研发部')).toBeInTheDocument()
    expect(screen.queryByText('2')).not.toBeInTheDocument()
  })

  it('点击节点触发 onChange 并带上 id', async () => {
    const onChange = vi.fn()
    render(TreeSelect, { props: { treeData: tree, onChange } })
    await fireEvent.click(screen.getByRole('button', { name: /请选择|选择/ }))
    await fireEvent.click(screen.getByRole('button', { name: '展开' }))
    await fireEvent.click(screen.getByText('研发部'))
    expect(onChange).toHaveBeenCalledWith('2')
  })

  it('excludeIds 指定的节点及其子孙不可选', async () => {
    render(TreeSelect, { props: { treeData: tree, excludeIds: [1] } })
    await fireEvent.click(screen.getByRole('button', { name: /请选择|选择/ }))
    expect(screen.queryByText('总公司')).not.toBeInTheDocument()
    expect(screen.queryByText('研发部')).not.toBeInTheDocument()
  })

  it('rootOption 合成根节点可选(菜单「主类目」场景)', async () => {
    const onChange = vi.fn()
    render(TreeSelect, {
      props: {
        treeData: tree,
        onChange,
        rootOption: { id: 0, label: '主类目' },
      },
    })
    await fireEvent.click(screen.getByRole('button', { name: /请选择|选择/ }))
    await fireEvent.click(screen.getByText('主类目'))
    expect(onChange).toHaveBeenCalledWith(0)
  })

  it('点击 chevron 展开/折叠子节点', async () => {
    render(TreeSelect, { props: { treeData: tree } })
    await fireEvent.click(screen.getByRole('button', { name: /请选择|选择/ }))
    expect(screen.queryByText('研发部')).not.toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '展开' }))
    expect(screen.getByText('研发部')).toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '折叠' }))
    expect(screen.queryByText('研发部')).not.toBeInTheDocument()
  })

  it('chevron 图标反映展开状态', async () => {
    render(TreeSelect, { props: { treeData: tree } })
    await fireEvent.click(screen.getByRole('button', { name: /请选择|选择/ }))
    const expandButton = screen.getByRole('button', { name: '展开' })
    expect(expandButton).toHaveAttribute('aria-expanded', 'false')
    await fireEvent.click(expandButton)
    const collapseButton = screen.getByRole('button', { name: '折叠' })
    expect(collapseButton).toHaveAttribute('aria-expanded', 'true')
  })
})
