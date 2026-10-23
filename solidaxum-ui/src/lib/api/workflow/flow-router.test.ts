import { describe, expect, it } from 'vitest'
import { buildFormUrl } from './flow-router'

describe('buildFormUrl', () => {
  // DB 实测: flow_definition.form_path 六条全是 /workflow/leaveEdit/index
  const FORM_PATH = '/workflow/leaveEdit/index'

  it('剥掉 formPath 末尾的 /index —— Vue 路由风格,Svelte 无此段', () => {
    expect(buildFormUrl({ formPath: FORM_PATH, businessId: '42', type: 'view' })).toBe(
      '/workflow/leaveEdit/42?mode=view'
    )
  })

  it('businessId 落在路径段而不是 query —— Svelte 路由是 [[id]]', () => {
    const url = buildFormUrl({ formPath: FORM_PATH, businessId: '42', type: 'view' })
    expect(url).not.toBeNull()
    expect(url!.startsWith('/workflow/leaveEdit/42')).toBe(true)
    expect(url!).not.toContain('id=42')
  })

  it('approval 带 taskId', () => {
    expect(
      buildFormUrl({ formPath: FORM_PATH, businessId: '42', type: 'approval', taskId: '99' })
    ).toBe('/workflow/leaveEdit/42?mode=approval&taskId=99')
  })

  it('带 from 时附上返回路径 —— leaveEdit 的 back() 否则硬跳 /demo/leave', () => {
    expect(
      buildFormUrl({
        formPath: FORM_PATH,
        businessId: '42',
        type: 'approval',
        taskId: '99',
        from: '/task/taskWaiting',
      })
    ).toBe('/workflow/leaveEdit/42?mode=approval&taskId=99&from=%2Ftask%2FtaskWaiting')
  })

  it('formPath 不以 /index 结尾时原样保留', () => {
    expect(buildFormUrl({ formPath: '/workflow/otherForm', businessId: '7', type: 'view' })).toBe(
      '/workflow/otherForm/7?mode=view'
    )
  })

  it('formPath 带尾斜杠也能正确拼接,不产生双斜杠', () => {
    expect(buildFormUrl({ formPath: '/workflow/leaveEdit/', businessId: '7', type: 'view' })).toBe(
      '/workflow/leaveEdit/7?mode=view'
    )
  })

  it('formPath 为空时返回 null —— 调用方据此禁用按钮而不是跳到 /undefined', () => {
    expect(buildFormUrl({ formPath: undefined, businessId: '42', type: 'view' })).toBeNull()
    expect(buildFormUrl({ formPath: '', businessId: '42', type: 'view' })).toBeNull()
  })

  it('businessId 为空时返回 null', () => {
    expect(buildFormUrl({ formPath: FORM_PATH, businessId: undefined, type: 'view' })).toBeNull()
  })

  it('businessId 全程按字符串处理 —— 雪花 id 精度', () => {
    const big = '1927145501234567890'
    expect(buildFormUrl({ formPath: FORM_PATH, businessId: big, type: 'view' })).toContain(big)
  })

  // 注:上面两条对返回值调 .startsWith/.toContain 的用例,TS 会因 string|null 报错,
  // 已在断言前用 expect(url).not.toBeNull() + url! 收窄;toContain 直接作用于
  // expect() 返回的 assertion 对象,不受此影响。
})
