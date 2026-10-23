import type { UserInfo } from '$lib/api/types.js'
import { describe, expect, it } from 'vitest'
import { summarizeIdentity } from './profile-identity.js'

const base: UserInfo = { userId: '1', userName: 'admin' }

describe('summarizeIdentity', () => {
  it('昵称优先于用户名', () => {
    expect(summarizeIdentity({ ...base, nickName: '张三' }).displayName).toBe(
      '张三'
    )
  })

  it('无昵称时回落用户名', () => {
    expect(summarizeIdentity(base).displayName).toBe('admin')
  })

  it('user 为 null 时给占位符,不再是字面量 User', () => {
    // 回归:旧实现写死 `auth.user?.username || 'User'`,字段名拼错(应为
    // userName)导致所有用户恒显示 'User'。
    expect(summarizeIdentity(null).displayName).toBe('-')
  })

  it('取昵称首字大写作头像 fallback', () => {
    expect(summarizeIdentity({ ...base, nickName: 'admin' }).initial).toBe('A')
    expect(summarizeIdentity({ ...base, nickName: '张三' }).initial).toBe('张')
  })

  it('角色不超过 2 个时全显示且无溢出', () => {
    const r = summarizeIdentity({
      ...base,
      roles: [
        { roleId: '1', roleName: '管理员' },
        { roleId: '2', roleName: '普通角色' },
      ],
    })
    expect(r.shownRoles).toHaveLength(2)
    expect(r.overflowCount).toBe(0)
  })

  it('角色超过 2 个时折叠为 +N', () => {
    const r = summarizeIdentity({
      ...base,
      roles: [
        { roleId: '1', roleName: '管理员' },
        { roleId: '2', roleName: '普通角色' },
        { roleId: '3', roleName: '审计员' },
        { roleId: '4', roleName: '访客' },
      ],
    })
    expect(r.shownRoles.map((x) => x.roleName)).toEqual(['管理员', '普通角色'])
    expect(r.overflowCount).toBe(2)
  })

  it('无角色时两者皆空', () => {
    const r = summarizeIdentity(base)
    expect(r.shownRoles).toEqual([])
    expect(r.overflowCount).toBe(0)
  })

  it('deptName 缺失回 null,供组件整行不渲染', () => {
    expect(summarizeIdentity(base).deptName).toBeNull()
    expect(summarizeIdentity({ ...base, deptName: '技术部' }).deptName).toBe(
      '技术部'
    )
  })
})
