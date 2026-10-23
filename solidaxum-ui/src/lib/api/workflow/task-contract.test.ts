import { describe, expect, it, vi, beforeEach } from 'vitest'

// apiClient 是 class 实例(client.ts:179)。vi.mock 替换整个模块导出,
// task.ts 拿到的是 mock 后对象,方法用 vi.fn()(返回值类型不关心,只验调用形状)。
vi.mock('$lib/api/client.js', () => ({
  apiClient: {
    put: vi.fn(),
    post: vi.fn(),
    get: vi.fn(),
    delete: vi.fn(),
    getList: vi.fn(),
  },
}))

import { taskApi } from './task.js'
import { apiClient } from '$lib/api/client.js'

beforeEach(() => vi.clearAllMocks())

describe('updateAssignee 请求契约', () => {
  // 后端 task_handler.rs:257-262 是 AppJson<Vec<i64>> —— body 必须是**裸数组**。
  // 包成 {taskIds:[...]} 反序列化失败 → 400。
  it('body 是裸数组,不是 {taskIds}', () => {
    taskApi.updateAssignee('7', ['1', '2'])
    expect(apiClient.put).toHaveBeenCalledWith('/workflow/task/updateAssignee/7', ['1', '2'])
  })

  it('userId 走路径段', () => {
    taskApi.updateAssignee('42', ['9'])
    expect(vi.mocked(apiClient.put).mock.calls[0][0]).toBe('/workflow/task/updateAssignee/42')
  })
})

describe('urge 请求契约', () => {
  // 后端 FlowUrgeTaskBo(dto.rs:708-713) 只有 taskId(必填单值) + message。
  // Vue 发的 taskIdList 数组在这里会导致 taskId 缺键 → 400。
  it('发 taskId 单值 + message,不是 taskIdList', () => {
    taskApi.urge('5', '催一下')
    expect(apiClient.post).toHaveBeenCalledWith('/workflow/task/urgeTask', {
      taskId: '5',
      message: '催一下',
    })
  })

  it('无消息内容时不发 message 键', () => {
    taskApi.urge('5')
    expect(apiClient.post).toHaveBeenCalledWith('/workflow/task/urgeTask', { taskId: '5' })
  })
})

describe('urgeMany 批量语义由前端循环实现', () => {
  // 后端没有批量端点,taskIdList 不存在。逐条发,任一失败不中断,返回计数。
  it('逐条调用 urge,调用次数等于任务数,返回成功/失败计数', async () => {
    vi.mocked(apiClient.post)
      .mockResolvedValueOnce({ code: 200 } as never)
      .mockRejectedValueOnce(new Error('404'))
      .mockResolvedValueOnce({ code: 200 } as never)
    const r = await taskApi.urgeMany(['1', '2', '3'], '催')
    expect(apiClient.post).toHaveBeenCalledTimes(3)
    expect(r).toEqual({ ok: 2, fail: 1 })
  })

  it('全部成功时返回 {ok:N, fail:0}', async () => {
    vi.mocked(apiClient.post).mockResolvedValue({ code: 200 } as never)
    const r = await taskApi.urgeMany(['1', '2'], '催')
    expect(r).toEqual({ ok: 2, fail: 0 })
  })

  it('逐条请求走 silent —— 失败 N 条不刷 N 个 toast,由调用方汇总', async () => {
    vi.mocked(apiClient.post).mockResolvedValue({ code: 200 } as never)
    await taskApi.urgeMany(['1', '2'], '催')
    for (const call of vi.mocked(apiClient.post).mock.calls) {
      expect(call[2]).toEqual({ silent: true })
    }
  })

  it('单条 urge 不传 silent(保持全局 toast 行为)', () => {
    taskApi.urge('5', '催一下')
    expect(vi.mocked(apiClient.post).mock.calls[0][2]).toBeUndefined()
  })
})

describe('pageByAllTaskFinish 端点契约', () => {
  // 全部待办页的已办 Tab 必须走「全部已办」,不是「我的已办」pageByTaskFinish。
  it('路径是 /workflow/task/pageByAllTaskFinish,不是 pageByTaskFinish', () => {
    taskApi.pageByAllTaskFinish({ pageNum: 1, pageSize: 10 })
    expect(vi.mocked(apiClient.getList).mock.calls[0][0]).toBe(
      '/workflow/task/pageByAllTaskFinish'
    )
  })
})
