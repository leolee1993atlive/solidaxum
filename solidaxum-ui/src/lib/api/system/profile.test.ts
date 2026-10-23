import { describe, expect, it } from 'vitest'

import { passwordIssues } from './profile.js'

describe('passwordIssues', () => {
  it('三项都空时逐条报出', () => {
    expect(passwordIssues({ oldPassword: '', newPassword: '', confirmPassword: '' })).toEqual([
      '旧密码不能为空',
      '新密码不能为空',
      '确认密码不能为空',
    ])
  })

  it('新密码长度不在 6-20 之间时报错', () => {
    const issues = passwordIssues({
      oldPassword: 'old123',
      newPassword: 'abc',
      confirmPassword: 'abc',
    })
    expect(issues).toContain('长度在 6 到 20 个字符')
  })

  it('新密码含非法字符时报错', () => {
    for (const bad of ['abc<def', 'abc>def', 'abc"def', "abc'def", 'abc\\def', 'abc|def']) {
      const issues = passwordIssues({
        oldPassword: 'old123',
        newPassword: bad,
        confirmPassword: bad,
      })
      expect(issues, `应拦下 ${bad}`).toContain('不能包含非法字符：< > " \' \\ |')
    }
  })

  it('两次输入不一致时报错', () => {
    expect(
      passwordIssues({
        oldPassword: 'old123',
        newPassword: 'newpass1',
        confirmPassword: 'newpass2',
      })
    ).toContain('两次输入的密码不一致')
  })

  it('合法输入无问题', () => {
    expect(
      passwordIssues({
        oldPassword: 'old123',
        newPassword: 'newpass1',
        confirmPassword: 'newpass1',
      })
    ).toEqual([])
  })
})
