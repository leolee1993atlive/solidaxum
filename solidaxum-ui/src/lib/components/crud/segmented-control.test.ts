import { cleanup, render, screen, fireEvent } from '@testing-library/svelte'
import { describe, expect, it, vi, afterEach } from 'vitest'
import SegmentedControl from './SegmentedControl.svelte'

afterEach(cleanup)

const options = [
  { label: '全部', value: '' },
  { label: '待提交', value: '1' },
  { label: '返回重报', value: '6' },
]

describe('SegmentedControl', () => {
  it('渲染全部选项,选中项 aria-pressed', () => {
    render(SegmentedControl, { props: { options, value: '1' } })
    const all = screen.getByRole('button', { name: '全部' })
    const active = screen.getByRole('button', { name: '待提交' })
    expect(all.getAttribute('aria-pressed')).toBe('false')
    expect(active.getAttribute('aria-pressed')).toBe('true')
  })

  it('点击触发 onchange 且携带选项值', async () => {
    const onchange = vi.fn()
    render(SegmentedControl, { props: { options, value: '', onchange } })
    await fireEvent.click(screen.getByRole('button', { name: '返回重报' }))
    expect(onchange).toHaveBeenCalledWith('6')
  })

  it('点击已选中项不触发 onchange', async () => {
    const onchange = vi.fn()
    render(SegmentedControl, { props: { options, value: '1', onchange } })
    await fireEvent.click(screen.getByRole('button', { name: '待提交' }))
    expect(onchange).not.toHaveBeenCalled()
  })

  it('按 role=group 暴露为分组(键盘可达的原生 button 组)', () => {
    render(SegmentedControl, { props: { options, value: '' } })
    expect(screen.getByRole('group')).toBeTruthy()
    expect(screen.getAllByRole('button').length).toBe(options.length)
  })
})
