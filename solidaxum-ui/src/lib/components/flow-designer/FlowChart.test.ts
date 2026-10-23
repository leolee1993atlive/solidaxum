import { designerApi } from '$lib/api/workflow/designer.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import { cleanup, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import FlowChart from './FlowChart.svelte'

vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: { queryFlowChart: vi.fn() },
}))

/** 给 leave6 的节点/边打上运行态:前两个节点已办、第三个待办,其余未到 */
function chartDef(): DefJson {
  const def = JSON.parse(JSON.stringify(leave6Json)) as DefJson
  def.nodeList.forEach((n, i) => {
    n.status = i < 2 ? 2 : i === 2 ? 1 : 0
    n.skipList.forEach((s) => (s.status = i < 2 ? 2 : 0))
  })
  def.chartStatusColor = ['1,2,3', '4,5,6', '7,8,9']
  return def
}

beforeEach(() => vi.clearAllMocks())
afterEach(cleanup)

describe('FlowChart', () => {
  it('按 instanceId 拉流程图,节点带状态 class,三原色注入画布容器', async () => {
    vi.mocked(designerApi.queryFlowChart).mockResolvedValue({
      code: 200,
      msg: 'ok',
      data: chartDef(),
    })
    const { container } = render(FlowChart, { props: { instanceId: '9' } })
    expect(designerApi.queryFlowChart).toHaveBeenCalledWith('9')
    await waitFor(() => expect(screen.queryByText('加载中…')).toBeNull())
    const canvas = container.querySelector('.fd-canvas') as HTMLElement
    expect(canvas.style.getPropertyValue('--fd-done').trim()).toBe('1,2,3')
    expect(canvas.style.getPropertyValue('--fd-notdone').trim()).toBe('7,8,9')
    await waitFor(() =>
      expect(container.querySelectorAll('.svelte-flow__node').length).toBe(14)
    )
    expect(container.querySelectorAll('.fd-node.fd-status-done')).toHaveLength(
      2
    )
    expect(container.querySelectorAll('.fd-node.fd-status-todo')).toHaveLength(
      1
    )
    expect(
      container.querySelectorAll('.fd-node.fd-status-notdone')
    ).toHaveLength(11)
  })

  it('后端没给 data → 图区显示错误,不抛', async () => {
    vi.mocked(designerApi.queryFlowChart).mockResolvedValue({
      code: 200,
      msg: 'ok',
    })
    render(FlowChart, { props: { instanceId: '9' } })
    await waitFor(() =>
      expect(screen.getByText('流程图数据为空')).toBeInTheDocument()
    )
  })
})
