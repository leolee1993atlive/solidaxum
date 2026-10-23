import { tick } from 'svelte'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import Fixture from './Toolbar.fixture.svelte'
import { DesignerStore } from './state/designer.svelte.js'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: { queryDef: vi.fn(), saveJson: vi.fn() },
}))

afterEach(cleanup)

function setup(readonly = false) {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  render(Fixture, { props: { store, readonly } })
  return store
}

describe('Toolbar 撤销 / 重做', () => {
  it('加节点的按钮不再在工具栏;撤销重做随历史置灰,点击生效', async () => {
    const store = setup()
    expect(screen.queryByRole('button', { name: /审批人/ })).toBeNull()
    const undo = screen.getByRole('button', { name: '撤销' })
    const redo = screen.getByRole('button', { name: '重做' })
    expect(undo).toBeDisabled()
    expect(redo).toBeDisabled()

    const applicant = store.nodes.find(
      (n) => n.data.json.nodeName === '申请人'
    )!
    store.updateNodeJson(applicant.id, { nodeName: '改名' })
    await tick() // 按钮的 disabled 随 store.canUndo 在下一个微任务刷新
    expect(undo).toBeEnabled()
    await fireEvent.click(undo)
    expect(
      store.nodes.find((n) => n.id === applicant.id)!.data.json.nodeName
    ).toBe('申请人')
    await tick()
    expect(redo).toBeEnabled()
    await fireEvent.click(redo)
    expect(
      store.nodes.find((n) => n.id === applicant.id)!.data.json.nodeName
    ).toBe('改名')
  })

  it('只读态没有撤销重做与保存', () => {
    setup(true)
    expect(screen.queryByRole('button', { name: '撤销' })).toBeNull()
    expect(screen.queryByRole('button', { name: '重做' })).toBeNull()
    expect(screen.queryByRole('button', { name: '保存' })).toBeNull()
    expect(screen.getByRole('button', { name: '适应画布' })).toBeInTheDocument()
  })
})
