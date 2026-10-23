import { cleanup, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import StatusSwitch from './StatusSwitch.svelte'

afterEach(cleanup)

describe('StatusSwitch', () => {
  it('checkedValue 命中时 aria-checked=true', () => {
    render(StatusSwitch, { props: { value: 'Y', checkedValue: 'Y', uncheckedValue: 'N', onChange: vi.fn() } })
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('true')
  })

  it('未命中时 aria-checked=false', () => {
    render(StatusSwitch, { props: { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', onChange: vi.fn() } })
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('false')
  })

  it('点击后以「另一侧的值」调用 onChange', async () => {
    const onChange = vi.fn().mockResolvedValue(undefined)
    render(StatusSwitch, { props: { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', onChange } })
    screen.getByRole('switch').click()
    await waitFor(() => expect(onChange).toHaveBeenCalledWith('Y'))
  })

  it('onChange 成功后保持翻转后的视觉状态', async () => {
    const onChange = vi.fn().mockResolvedValue(undefined)
    render(StatusSwitch, { props: { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', onChange } })
    const el = screen.getByRole('switch')
    el.click()
    await waitFor(() => expect(el.getAttribute('aria-checked')).toBe('true'))
  })

  it('onChange 抛错时视觉状态回滚', async () => {
    const onChange = vi.fn().mockRejectedValue(new Error('boom'))
    render(StatusSwitch, { props: { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', onChange } })
    const el = screen.getByRole('switch')
    el.click()
    await waitFor(() => expect(onChange).toHaveBeenCalled())
    await waitFor(() => expect(el.getAttribute('aria-checked')).toBe('false'))
  })

  it('请求进行中禁止重复点击', async () => {
    let resolve: () => void = () => {}
    const onChange = vi.fn().mockReturnValue(new Promise<void>((r) => (resolve = r)))
    render(StatusSwitch, { props: { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', onChange } })
    const el = screen.getByRole('switch')
    el.click()
    el.click()
    el.click()
    await waitFor(() => expect(onChange).toHaveBeenCalledTimes(1))
    resolve()
  })

  it('disabled 时不调用 onChange', () => {
    const onChange = vi.fn()
    render(StatusSwitch, { props: { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', onChange, disabled: true } })
    screen.getByRole('switch').click()
    expect(onChange).not.toHaveBeenCalled()
  })

  it('后续 refetch 把本行改回时跟随服务端(唯一默认桶场景)', async () => {
    // oss-config 的真实序列:
    //   1. A 是 N,点开关设为默认 → 乐观值 'Y'
    //   2. refetch 落地,后端确认 A='Y'
    //   3. 之后把 B 设为默认 → 后端因「唯一默认桶」把 A 置回 'N',refetch 送来 A='N'
    // 乐观值若不在第 2 步交还话语权,第 3 步就仍显示打开 —— 界面上两个默认桶,
    // 正好破坏该页要保证的唯一性。
    const onChange = vi.fn().mockResolvedValue(undefined)
    const base = { checkedValue: 'Y', uncheckedValue: 'N', onChange }
    const { rerender } = render(StatusSwitch, { props: { value: 'N', ...base } })
    const el = screen.getByRole('switch')

    el.click()
    await waitFor(() => expect(el.getAttribute('aria-checked')).toBe('true'))

    await rerender({ value: 'Y', ...base }) // 步骤 2
    expect(el.getAttribute('aria-checked')).toBe('true')

    await rerender({ value: 'N', ...base }) // 步骤 3:B 抢走了默认位
    await waitFor(() => expect(el.getAttribute('aria-checked')).toBe('false'))
  })

  it('props.value 追平乐观值后不闪回', async () => {
    const onChange = vi.fn().mockResolvedValue(undefined)
    const base = { checkedValue: 'Y', uncheckedValue: 'N', onChange }
    const { rerender } = render(StatusSwitch, { props: { value: 'N', ...base } })
    const el = screen.getByRole('switch')
    el.click()
    await waitFor(() => expect(el.getAttribute('aria-checked')).toBe('true'))

    // 正常路径:refetch 后后端确认就是 Y,视觉不应闪一下
    await rerender({ value: 'Y', ...base })
    expect(el.getAttribute('aria-checked')).toBe('true')
  })
})
