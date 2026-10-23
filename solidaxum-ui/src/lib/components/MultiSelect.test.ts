import { cleanup, render, screen, fireEvent } from '@testing-library/svelte'
import { describe, expect, it, vi, afterEach } from 'vitest'
import MultiSelect from './MultiSelect.svelte'

afterEach(cleanup)

const options = [
  { label: '超级管理员', value: '1' },
  { label: '普通角色', value: '2' },
  { label: '停用角色', value: '3', disabled: true },
]

describe('MultiSelect', () => {
  it('未选中时显示占位文案', () => {
    render(MultiSelect, { props: { options, values: [], placeholder: '请选择角色' } })
    expect(screen.getByText('请选择角色')).toBeInTheDocument()
  })

  it('已选中时回显标签而非裸 id', () => {
    render(MultiSelect, { props: { options, values: ['1', '2'] } })
    expect(screen.getByText('超级管理员, 普通角色')).toBeInTheDocument()
  })

  it('勾选选项时 onChange 带上累加后的数组', async () => {
    const onChange = vi.fn()
    render(MultiSelect, { props: { options, values: ['1'], onChange } })
    await fireEvent.click(screen.getByRole('button', { name: /超级管理员/ }))
    await fireEvent.click(screen.getByLabelText('普通角色'))
    expect(onChange).toHaveBeenCalledWith(['1', '2'])
  })

  it('取消勾选时从数组移除', async () => {
    const onChange = vi.fn()
    render(MultiSelect, { props: { options, values: ['1', '2'], onChange } })
    await fireEvent.click(screen.getByRole('button', { name: /超级管理员/ }))
    await fireEvent.click(screen.getByLabelText('超级管理员'))
    expect(onChange).toHaveBeenCalledWith(['2'])
  })

  it('disabled 选项点击不产生变更', async () => {
    const onChange = vi.fn()
    render(MultiSelect, { props: { options, values: [], onChange } })
    await fireEvent.click(screen.getByRole('button', { name: /请选择/ }))
    await fireEvent.click(screen.getByLabelText('停用角色'))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('搜索框按关键字过滤选项', async () => {
    render(MultiSelect, { props: { options, values: [] } })
    await fireEvent.click(screen.getByRole('button', { name: /请选择/ }))
    await fireEvent.input(screen.getByPlaceholderText('搜索'), {
      target: { value: '普通' },
    })
    expect(screen.getByLabelText('普通角色')).toBeInTheDocument()
    expect(screen.queryByLabelText('超级管理员')).not.toBeInTheDocument()
  })
})
