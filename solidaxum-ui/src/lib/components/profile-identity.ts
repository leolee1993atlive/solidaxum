import type { UserInfo, UserRoleBrief } from '$lib/api/types.js'

/** 身份区最多显示的角色徽章数——下拉菜单宽 w-56,再多会换行撑破。 */
export const MAX_ROLE_BADGES = 2

export interface IdentitySummary {
  /** 昵称优先,回落用户名,都没有则占位符。 */
  displayName: string
  /** 头像加载失败时的 fallback 字符。 */
  initial: string
  /** 实际渲染的角色徽章。 */
  shownRoles: UserRoleBrief[]
  /** 被折叠掉的角色个数,0 表示不显示 +N。 */
  overflowCount: number
  /** null 表示后端没给部门,组件应整行不渲染而非显示占位符。 */
  deptName: string | null
}

/** 由 getInfo 的 user 推导下拉身份区所需的全部展示值。 */
export function summarizeIdentity(user: UserInfo | null): IdentitySummary {
  const displayName = user?.nickName || user?.userName || '-'
  const roles = user?.roles ?? []
  const shownRoles = roles.slice(0, MAX_ROLE_BADGES)
  return {
    displayName,
    initial: displayName.slice(0, 1).toUpperCase(),
    shownRoles,
    overflowCount: roles.length - shownRoles.length,
    deptName: user?.deptName || null,
  }
}
