import { cleanup, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import StatusSwitchCell from './StatusSwitchCell.svelte'

const base = { value: 'N', checkedValue: 'Y', uncheckedValue: 'N', confirmText: '确认要设为默认吗?' }

afterEach(cleanup)

describe('StatusSwitchCell', () => {
  it('点击开关先弹确认框,不直接调 onConfirm', async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    render(StatusSwitchCell, { props: { ...base, onConfirm } })
    screen.getByRole('switch').click()
    await waitFor(() => expect(screen.getByText('确认要设为默认吗?')).toBeInTheDocument())
    expect(onConfirm).not.toHaveBeenCalled()
  })

  it('确认后以目标值调用 onConfirm 并触发 onDone', async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    const onDone = vi.fn()
    render(StatusSwitchCell, { props: { ...base, onConfirm, onDone } })
    screen.getByRole('switch').click()
    const ok = await screen.findByRole('button', { name: '确定' })
    ok.click()
    await waitFor(() => expect(onConfirm).toHaveBeenCalledWith('Y'))
    await waitFor(() => expect(onDone).toHaveBeenCalled())
  })

  it('取消则不调用 onConfirm,且开关回滚', async () => {
    const onConfirm = vi.fn()
    render(StatusSwitchCell, { props: { ...base, onConfirm } })
    const sw = screen.getByRole('switch')
    sw.click()
    const cancel = await screen.findByRole('button', { name: '取消' })
    cancel.click()
    await waitFor(() => expect(sw.getAttribute('aria-checked')).toBe('false'))
    expect(onConfirm).not.toHaveBeenCalled()
  })

  it('onConfirm 失败时开关回滚且不触发 onDone', async () => {
    const onConfirm = vi.fn().mockRejectedValue(new Error('boom'))
    const onDone = vi.fn()
    render(StatusSwitchCell, { props: { ...base, onConfirm, onDone } })
    const sw = screen.getByRole('switch')
    sw.click()
    ;(await screen.findByRole('button', { name: '确定' })).click()
    await waitFor(() => expect(onConfirm).toHaveBeenCalled())
    await waitFor(() => expect(sw.getAttribute('aria-checked')).toBe('false'))
    expect(onDone).not.toHaveBeenCalled()
  })

  it('请求进行中确认按钮禁用,防止双击重复调 onConfirm', async () => {
    // 模拟慢请求:确认按钮在 onConfirm resolve 前应处于 disabled 态,
    // 期间连点确定只会触发一次 onConfirm。
    let resolveReq: () => void = () => {}
    const onConfirm = vi.fn().mockReturnValue(
      new Promise<void>((r) => (resolveReq = r))
    )
    render(StatusSwitchCell, { props: { ...base, onConfirm } })
    screen.getByRole('switch').click()
    const ok = await screen.findByRole('button', { name: '确定' })
    ok.click()
    // 请求挂起期间,确认按钮应被禁用(ConfirmDialog 的 isLoading 通道)
    await waitFor(() => expect((ok as HTMLButtonElement).disabled).toBe(true))
    ok.click() // 双击穿透尝试
    ok.click()
    expect(onConfirm).toHaveBeenCalledTimes(1)
    resolveReq() // 放行,让测试干净退出
    await waitFor(() => expect((ok as HTMLButtonElement).disabled).toBe(false))
  })
})
