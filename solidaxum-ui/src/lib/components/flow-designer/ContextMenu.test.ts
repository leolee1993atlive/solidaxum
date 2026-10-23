import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import ContextMenu, { type MenuItem } from './ContextMenu.svelte'

afterEach(cleanup)

function setup(items: MenuItem[]) {
  const onclose = vi.fn()
  const view = render(ContextMenu, { props: { x: 10, y: 20, items, onclose } })
  return { ...view, onclose }
}

describe('ContextMenu', () => {
  it('按顺序渲染菜单项、快捷键提示与分隔线;置灰项不可点', async () => {
    const paste = vi.fn()
    const add = vi.fn()
    const { onclose } = setup([
      { label: '粘贴', shortcut: 'Ctrl+V', disabled: true, onselect: paste },
      { label: '添加审批人', separatorBefore: true, onselect: add },
    ])
    const items = screen.getAllByRole('menuitem')
    expect(
      items.map((el) => el.textContent?.replace(/\s+/g, ' ').trim())
    ).toEqual(['粘贴 Ctrl+V', '添加审批人'])
    expect(screen.getAllByRole('separator')).toHaveLength(1)
    expect(items[0]).toBeDisabled()

    await fireEvent.click(items[1])
    expect(add).toHaveBeenCalledOnce()
    expect(onclose).toHaveBeenCalledOnce()
    expect(paste).not.toHaveBeenCalled()
  })

  it('Esc、菜单外按下、滚轮都关闭;菜单内按下不关', async () => {
    const { onclose } = setup([{ label: '删除', onselect: () => {} }])
    await fireEvent.pointerDown(screen.getByRole('menuitem'))
    expect(onclose).not.toHaveBeenCalled()
    await fireEvent.pointerDown(document.body)
    expect(onclose).toHaveBeenCalledTimes(1)
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onclose).toHaveBeenCalledTimes(2)
    await fireEvent.wheel(window)
    expect(onclose).toHaveBeenCalledTimes(3)
  })

  it('画布不能平移到菜单上(nopan),定位用传入坐标', () => {
    setup([{ label: '删除', onselect: () => {} }])
    const menu = screen.getByRole('menu')
    expect(menu).toHaveClass('nopan')
    expect(menu.style.left).toBe('10px')
    expect(menu.style.top).toBe('20px')
  })
})
