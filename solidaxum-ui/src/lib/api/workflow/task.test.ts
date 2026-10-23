import { describe, expect, it } from 'vitest'

import { TASK_OPS, buildOperationBody } from './task.js'

describe('任务操作', () => {
  it('op 常量与后端 task_service.rs:1254-1284 的分支一致', () => {
    expect(TASK_OPS).toEqual({
      transfer: 'transferTask',
      delegate: 'delegateTask',
      addSign: 'addSignature',
      reductionSign: 'reductionSignature',
    })
  })

  it('buildOperationBody 传 taskId 与 userIds 字符串数组', () => {
    expect(buildOperationBody('7', ['1', '2'], '交给你')).toEqual({
      taskId: '7',
      userIds: ['1', '2'],
      message: '交给你',
    })
  })

  it('message 为空时不带该键', () => {
    const body = buildOperationBody('7', ['1'], '')
    expect('message' in body).toBe(false)
  })
})
