import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { DesignerStore } from '../state/designer.svelte.js'
import SkipForm from './SkipForm.svelte'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: { queryDef: vi.fn(), saveJson: vi.fn(), listenerList: vi.fn() },
}))

afterEach(cleanup)

// 下拉是 bits-ui Select:触发器按 pointerdown 打开,选项在 portal 里按 pointerup 选中
// (同 QueryForm.test)。触发器文案即当前值。
async function pick(label: string, option: string) {
  await fireEvent.pointerDown(screen.getByLabelText(label))
  await fireEvent.pointerUp(await screen.findByRole('option', { name: option }))
}

function setup(edgeName?: string) {
  const store = new DesignerStore()
  store.setDef(leave6Json as unknown as DefJson)
  const edge = edgeName
    ? store.edges.find((e) => e.data.json.skipName === edgeName)!
    : store.edges.find((e) => e.data.json.skipCondition?.startsWith('spel@@'))!
  store.selectEdge(edge.id)
  return { store, edge }
}

describe('SkipForm', () => {
  it('回填名称/类型,改名逐字符写回 store', async () => {
    const { store, edge } = setup('大于两天')
    render(SkipForm, { props: { store, edge } })
    const name = screen.getByLabelText('跳转名称') as HTMLInputElement
    expect(name.value).toBe('大于两天')
    const type = screen.getByLabelText('跳转类型')
    expect(type.tagName).toBe('BUTTON')
    expect(type).toHaveTextContent('审批通过')
    await fireEvent.input(name, { target: { value: '超过两天' } })
    expect(store.edges.find((e) => e.id === edge.id)!.data.json.skipName).toBe(
      '超过两天'
    )
  })

  it('default@@ 条件按原始表达式显示并提示引擎不支持', () => {
    const { store, edge } = setup('大于两天')
    render(SkipForm, { props: { store, edge } })
    expect(screen.getByLabelText('跳转条件')).toHaveTextContent(/原始表达式/)
    expect(
      (screen.getByLabelText('原始表达式') as HTMLTextAreaElement).value
    ).toBe('default@@${leaveDays > 2}')
    expect(screen.getByRole('alert')).toHaveTextContent(/不支持/)
  })

  it('切到比较条件并填写 → 写回 op@@变量|值;缺值时显示校验错误', async () => {
    const { store, edge } = setup('大于两天')
    const view = render(SkipForm, { props: { store, edge } })
    await pick('跳转条件', '比较条件')
    const current = () => store.edges.find((e) => e.id === edge.id)!
    expect(current().data.json.skipCondition).toBe('eq@@|')
    // 表单是受控的:store 变了要把新 edge 喂回去
    await view.rerender({ store, edge: current() })
    expect(screen.getByRole('alert')).toHaveTextContent(/变量名/)
    await fireEvent.input(screen.getByLabelText('条件变量'), {
      target: { value: 'leaveDays' },
    })
    await view.rerender({ store, edge: current() })
    await pick('比较方式', '大于')
    await view.rerender({ store, edge: current() })
    await fireEvent.input(screen.getByLabelText('比较值'), {
      target: { value: '2' },
    })
    expect(current().data.json.skipCondition).toBe('gt@@leaveDays|2')
  })

  it('spel 用 #{ } 包裹的存量数据显示校验提示;去掉包裹后提示消失', async () => {
    const { store, edge } = setup()
    const view = render(SkipForm, { props: { store, edge } })
    expect(screen.getByLabelText('跳转条件')).toHaveTextContent('spel 表达式')
    expect(screen.getByRole('alert')).toHaveTextContent(/包裹/)
    await fireEvent.input(screen.getByLabelText('spel 表达式'), {
      target: { value: '#leaveDays > 2' },
    })
    const current = store.edges.find((e) => e.id === edge.id)!
    expect(current.data.json.skipCondition).toBe('spel@@#leaveDays > 2')
    await view.rerender({ store, edge: current })
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('无条件 → skipCondition 写 null', async () => {
    const { store, edge } = setup('大于两天')
    render(SkipForm, { props: { store, edge } })
    await pick('跳转条件', '无条件')
    expect(
      store.edges.find((e) => e.id === edge.id)!.data.json.skipCondition
    ).toBeNull()
  })

  it('只读时全部控件禁用', () => {
    const { store, edge } = setup('大于两天')
    render(SkipForm, { props: { store, edge, readonly: true } })
    expect(
      (screen.getByLabelText('跳转名称') as HTMLInputElement).disabled
    ).toBe(true)
    expect(screen.getByLabelText('跳转类型')).toBeDisabled()
  })
})
