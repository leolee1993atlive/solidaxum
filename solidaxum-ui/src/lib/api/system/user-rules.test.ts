import { describe, expect, it } from 'vitest'
import { deptFormIssues, userFormIssues, resetPwdIssues } from './user-rules.js'

describe('deptFormIssues —— 逐条移植 Vue dept/index.vue:248-266 的 rules', () => {
  const ok = {
    parentId: '100',
    deptName: '研发部',
    orderNum: 1,
    phone: '13800138000',
    email: 'dev@example.com',
  }

  it('全字段合法时无告警', () => {
    expect(deptFormIssues(ok)).toEqual([])
  })

  it('上级部门为空要报', () => {
    expect(deptFormIssues({ ...ok, parentId: '' })).toContain('上级部门不能为空')
  })

  it('部门名称为空要报', () => {
    expect(deptFormIssues({ ...ok, deptName: '  ' })).toContain('部门名称不能为空')
  })

  it('显示排序为空/负数要报', () => {
    expect(deptFormIssues({ ...ok, orderNum: undefined })).toContain('显示排序不能为空')
  })

  it('邮箱格式非法要报,留空不报', () => {
    expect(deptFormIssues({ ...ok, email: 'not-an-email' })).toContain('请输入正确的邮箱地址')
    expect(deptFormIssues({ ...ok, email: '' })).toEqual([])
  })

  it('手机号非法要报,留空不报', () => {
    expect(deptFormIssues({ ...ok, phone: '12345' })).toContain('请输入正确的手机号码')
    expect(deptFormIssues({ ...ok, phone: '' })).toEqual([])
  })
})

describe('userFormIssues —— 逐条移植 Vue user/index.vue:551-591 的 rules', () => {
  const ok = {
    userName: 'zhangsan',
    nickName: '张三',
    password: 'abc12345',
    email: 'z@example.com',
    phoneNumber: '13800138000',
    roleIds: ['1'],
  }

  it('新增态全字段合法时无告警', () => {
    expect(userFormIssues(ok, { isEdit: false })).toEqual([])
  })

  it('用户名少于 2 位要报', () => {
    expect(userFormIssues({ ...ok, userName: 'a' }, { isEdit: false })).toContain(
      '用户名称长度必须介于 2 和 20 之间'
    )
  })

  it('昵称为空要报', () => {
    expect(userFormIssues({ ...ok, nickName: '' }, { isEdit: false })).toContain('用户昵称不能为空')
  })

  it('新增态密码为空要报,编辑态不校验密码', () => {
    expect(userFormIssues({ ...ok, password: '' }, { isEdit: false })).toContain('用户密码不能为空')
    expect(userFormIssues({ ...ok, password: '' }, { isEdit: true })).toEqual([])
  })

  it('密码含非法字符要报', () => {
    expect(userFormIssues({ ...ok, password: 'abc<script' }, { isEdit: false })).toContain(
      '密码不能包含非法字符：< > " \' \ |'
    )
  })

  it('邮箱/手机号格式非法要报,留空不报', () => {
    expect(userFormIssues({ ...ok, email: 'bad' }, { isEdit: false })).toContain(
      '请输入正确的邮箱地址'
    )
    expect(userFormIssues({ ...ok, phoneNumber: '110' }, { isEdit: false })).toContain(
      '请输入正确的手机号码'
    )
    expect(userFormIssues({ ...ok, email: '', phoneNumber: '' }, { isEdit: false })).toEqual([])
  })

  it('🔴 角色为空要报 —— MultiSelect 可以被取消勾选到空,这是唯一防线', () => {
    expect(userFormIssues({ ...ok, roleIds: [] }, { isEdit: false })).toContain('用户角色不能为空')
  })

  it('编辑自己时不校验角色(表单里根本不显示这三项)', () => {
    expect(userFormIssues({ ...ok, roleIds: [] }, { isEdit: true, editingSelf: true })).toEqual([])
  })
})

describe('resetPwdIssues —— 对齐 Vue ElMessageBox.prompt 的 inputPattern/inputValidator', () => {
  it('5-20 位且不含非法字符时通过', () => {
    expect(resetPwdIssues('abc12345')).toEqual([])
  })
  it('太短要报', () => {
    expect(resetPwdIssues('abc')).toContain('用户密码长度必须介于 5 和 20 之间')
  })
  it('含非法字符要报', () => {
    expect(resetPwdIssues('abc|def')).toContain('密码不能包含非法字符：< > " \' \ |')
  })
})
