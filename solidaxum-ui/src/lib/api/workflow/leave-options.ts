/**
 * 请假 demo 的三组值域 —— 列表页与 leaveEdit 表单页共用。
 *
 * 为什么要抽出来:「请假类型」曾在列表页(1病假/2事假)和表单页
 * (1病假/2事假/3年假)各写了一份,两份都和 Vue(1事假/2调休/3病假/4婚假)不一致 ——
 * 录进去的是一个值,列表显示的是另一个语义。同源 + 单测是唯一防线。
 */

import type { LeaveAddBo } from './leave.js'

export type LeaveTypeOption = { value: string; label: string }

/** 对齐 Vue leave/index.vue:176-193 与 leaveEdit.vue:83-100(两处同一份)。 */
export const LEAVE_TYPE_OPTIONS: LeaveTypeOption[] = [
  { value: '1', label: '事假' },
  { value: '2', label: '调休' },
  { value: '3', label: '病假' },
  { value: '4', label: '婚假' },
]

/** 未知值原样回显(便于发现脏数据),空值显示 '-'。 */
export function leaveTypeLabel(v?: string): string {
  if (v === undefined || v === null || v === '') return '-'
  return LEAVE_TYPE_OPTIONS.find((o) => o.value === v)?.label ?? v
}

/**
 * 流程定义在本仓的可用性 —— 连真实库 + Rust 求值器实测得出,别凭 Vue 猜:
 * - `ok`           无额外变量依赖,或依赖的变量后端已注入(leaveDays)
 * - `no-assignee`  节点办理人写作 ${userList},本仓不注入该变量 →
 *                  resolve_handlers 会把字面量当办理人 → 任务无人可办(静默)
 * - `unsupported`  条件用了 default@@ 算子 / SpEL 方法调用,Rust 引擎不支持 → 500
 */
export type FlowCodeSupport = 'ok' | 'no-assignee' | 'unsupported'

export type FlowCodeOption = {
  value: string
  label: string
  support: FlowCodeSupport
  /** 非 ok 时展示给用户的一句话原因。 */
  note?: string
}

/** 对齐 Vue leaveEdit.vue:101-126 的六项;support 口径见上方 FlowCodeSupport 注释。 */
export const FLOW_CODE_OPTIONS: FlowCodeOption[] = [
  { value: 'leave1', label: '请假申请-普通', support: 'ok' },
  { value: 'leave2', label: '请假申请-排他网关', support: 'ok' },
  { value: 'leave3', label: '请假申请-并行网关', support: 'ok' },
  {
    value: 'leave4',
    label: '请假申请-会签',
    support: 'no-assignee',
    note: '会签节点办理人取流程变量 userList,当前后端未注入 → 任务无人可办',
  },
  {
    value: 'leave5',
    label: '请假申请-并行会签网关',
    support: 'no-assignee',
    note: '同 leave4,依赖未注入的 userList',
  },
  {
    value: 'leave6',
    label: '请假申请-排他并行会签',
    support: 'unsupported',
    note: '条件用了 default@@ 与 SpEL 方法调用,本仓流程引擎不支持',
  },
]

/** 可编辑/可删除的单据状态(Vue index.vue:96,110 同一组条件)。 */
export const LEAVE_EDITABLE_STATUS = new Set(['draft', 'cancel', 'back'])

export function canEditLeave(status?: string): boolean {
  return status !== undefined && LEAVE_EDITABLE_STATUS.has(status)
}

/** 只有「待审核」能撤销(Vue index.vue:127)。 */
export function canCancelLeave(status?: string): boolean {
  return status === 'waiting'
}

/**
 * 「提交」(发起流程)的 body。
 *
 * 🔴 抽成纯函数只为一件事:**update 模式必须带 `id`**。后端 submit_and_start
 * 带 id 走就地更新、不带走 add() 新建(对齐 Java `insertOrUpdate`,
 * TestLeaveServiceImpl:157)。漏 id 的后果是从「修改」页点一次提交就凭空多出
 * 一条单据、原草稿仍停在 draft —— 编译/typecheck/单测原本一条都拦不住,
 * 因为 `id` 是可选字段。
 */
export function buildLeaveSubmitBo(input: {
  mode: 'add' | 'update'
  id?: string
  leaveType: string
  leaveDays: number
  remark?: string
  flowCode?: string
}): LeaveAddBo {
  return {
    // add 模式不发 id(后端自己发雪花 id);update 模式没有 id 就是 bug,不静默兜底。
    ...(input.mode === 'update' ? { id: input.id } : {}),
    leaveType: input.leaveType,
    leaveDays: input.leaveDays,
    remark: input.remark || undefined,
    flowCode: input.flowCode,
  }
}
