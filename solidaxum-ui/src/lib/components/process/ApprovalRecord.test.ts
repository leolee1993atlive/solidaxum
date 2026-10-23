import { designerApi } from '$lib/api/workflow/designer.js'
import { instanceApi } from '$lib/api/workflow/instance.js'
import leave6Json from '$lib/flow/fixtures/leave6.json'
import type { DefJson } from '$lib/flow/types.js'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ApprovalRecord from './ApprovalRecord.svelte'

vi.mock('$lib/api/workflow/instance.js', () => ({
  instanceApi: { flowHisTaskList: vi.fn() },
}))
vi.mock('$lib/api/workflow/designer.js', () => ({
  designerApi: { queryFlowChart: vi.fn() },
}))

const list = [
  {
    id: '1',
    nodeName: '申请人',
    approver: '张三',
    flowStatus: '2',
    message: '同意',
    createTime: '2026-09-01 10:00',
  },
  {
    id: '2',
    nodeName: '副组长',
    approver: 'lisi',
    flowStatus: '1',
    createTime: '2026-09-02 10:00',
  },
]

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(designerApi.queryFlowChart).mockResolvedValue({
    code: 200,
    msg: 'ok',
    data: leave6Json as unknown as DefJson,
  })
})
afterEach(cleanup)

describe('ApprovalRecord', () => {
  it('后端返回 {list, instanceId}:默认落在流程图 tab 并按 instanceId 拉图;记录 tab 列出条目', async () => {
    vi.mocked(instanceApi.flowHisTaskList).mockResolvedValue({
      code: 200,
      msg: 'ok',
      data: { list, instanceId: '1761300000000000009' },
    })
    render(ApprovalRecord, { props: { businessId: '7' } })
    expect(instanceApi.flowHisTaskList).toHaveBeenCalledWith('7')
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: '流程图' })).toBeInTheDocument()
    )
    expect(screen.getByRole('tab', { name: '流程图' })).toHaveAttribute(
      'data-state',
      'active'
    )
    await waitFor(() =>
      expect(designerApi.queryFlowChart).toHaveBeenCalledWith(
        '1761300000000000009'
      )
    )
    await fireEvent.click(screen.getByRole('tab', { name: '审批记录' }))
    await waitFor(() =>
      expect(screen.getByText('张三 · 2026-09-01 10:00')).toBeInTheDocument()
    )
    expect(screen.getByText('lisi · 2026-09-02 10:00')).toBeInTheDocument()
    expect(screen.getByText('同意')).toBeInTheDocument()
  })

  it('没有 instanceId(流程未启动):流程图 tab 禁用,默认显示记录', async () => {
    vi.mocked(instanceApi.flowHisTaskList).mockResolvedValue({
      code: 200,
      msg: 'ok',
      data: { list: [], instanceId: null },
    })
    render(ApprovalRecord, { props: { businessId: '7' } })
    await waitFor(() =>
      expect(screen.getByText('暂无审批记录')).toBeInTheDocument()
    )
    expect(screen.getByRole('tab', { name: '流程图' })).toBeDisabled()
    expect(designerApi.queryFlowChart).not.toHaveBeenCalled()
  })
})
