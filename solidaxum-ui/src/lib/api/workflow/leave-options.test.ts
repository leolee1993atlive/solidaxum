import { describe, expect, it } from 'vitest'
import {
  LEAVE_TYPE_OPTIONS,
  leaveTypeLabel,
  FLOW_CODE_OPTIONS,
  canEditLeave,
  canCancelLeave,
  buildLeaveSubmitBo,
} from './leave-options.js'

describe('LEAVE_TYPE_OPTIONS —— 逐字对齐 Vue leave/index.vue:176-193', () => {
  it('四项且顺序值域一致', () => {
    expect(LEAVE_TYPE_OPTIONS).toEqual([
      { value: '1', label: '事假' },
      { value: '2', label: '调休' },
      { value: '3', label: '病假' },
      { value: '4', label: '婚假' },
    ])
  })

  // 🔴 反向断言:列表页曾把 1 映射成「病假」、表单页曾把 3 映射成「年假」,
  // 两处各写各的且都错。这条测试就是钉住这个回归。
  it('1 是事假不是病假,3 是病假不是年假', () => {
    expect(leaveTypeLabel('1')).toBe('事假')
    expect(leaveTypeLabel('1')).not.toBe('病假')
    expect(leaveTypeLabel('3')).toBe('病假')
    expect(leaveTypeLabel('3')).not.toBe('年假')
  })

  it('未知值与空值回落', () => {
    expect(leaveTypeLabel('9')).toBe('9')
    expect(leaveTypeLabel(undefined)).toBe('-')
  })
})

describe('FLOW_CODE_OPTIONS —— 六条定义连真实库核实存在且已发布', () => {
  it('六项,value/label 与 Vue leaveEdit.vue:101-126 一致', () => {
    expect(FLOW_CODE_OPTIONS.map((o) => o.value)).toEqual([
      'leave1',
      'leave2',
      'leave3',
      'leave4',
      'leave5',
      'leave6',
    ])
    expect(FLOW_CODE_OPTIONS[0].label).toBe('请假申请-普通')
    expect(FLOW_CODE_OPTIONS[5].label).toBe('请假申请-排他并行会签')
  })

  // 可用性标注来自 flow_skip.skip_condition / flow_node.permission_flag 的实测。
  // 标注错了 = 又给用户递一个必 500 的选项。
  it('leave1/leave3 无依赖;leave2 依赖 leaveDays;leave4/5 依赖 userList;leave6 引擎不支持', () => {
    const by = (v: string) => FLOW_CODE_OPTIONS.find((o) => o.value === v)!
    expect(by('leave1').support).toBe('ok')
    expect(by('leave3').support).toBe('ok')
    expect(by('leave2').support).toBe('ok') // 后端已注入 leaveDays,可用
    expect(by('leave4').support).toBe('no-assignee')
    expect(by('leave5').support).toBe('no-assignee')
    expect(by('leave6').support).toBe('unsupported')
  })
})

describe('状态守卫 —— 对齐 Vue index.vue:96,110,127', () => {
  it('draft/cancel/back 可改可删,其余不可', () => {
    for (const s of ['draft', 'cancel', 'back']) expect(canEditLeave(s)).toBe(true)
    for (const s of ['waiting', 'finish', 'invalid', 'termination']) {
      expect(canEditLeave(s)).toBe(false)
    }
  })

  it('只有 waiting 可撤销', () => {
    expect(canCancelLeave('waiting')).toBe(true)
    for (const s of ['draft', 'cancel', 'back', 'finish', 'invalid', 'termination']) {
      expect(canCancelLeave(s)).toBe(false)
    }
  })

  it('status 缺失时一律不可操作(宁可少给入口,也不给一个点了就 400 的按钮)', () => {
    expect(canEditLeave(undefined)).toBe(false)
    expect(canCancelLeave(undefined)).toBe(false)
  })
})

describe('buildLeaveSubmitBo', () => {
  // 🔴 从「修改」页提交时不发 id,
  // 后端 submit_and_start 于是走 add() 新建 —— 列表凭空多一行、原草稿仍是 draft。
  it('update 模式必须带 id(不带 = 后端会新建一条重复单据)', () => {
    const bo = buildLeaveSubmitBo({
      mode: 'update',
      id: '1761100000000000001',
      leaveType: '1',
      leaveDays: 5,
      flowCode: 'leave1',
    })
    expect(bo.id).toBe('1761100000000000001')
    // 雪花 id 必须原样是字符串,不能被转成 number(会截断)
    expect(typeof bo.id).toBe('string')
  })

  it('add 模式不发 id(后端自己发雪花 id)', () => {
    const bo = buildLeaveSubmitBo({
      mode: 'add',
      leaveType: '1',
      leaveDays: 1,
      flowCode: 'leave1',
    })
    expect('id' in bo).toBe(false)
  })

  it('空 remark 归一为 undefined,不发空串', () => {
    const bo = buildLeaveSubmitBo({ mode: 'add', leaveType: '1', leaveDays: 1, remark: '' })
    expect(bo.remark).toBeUndefined()
  })
})
