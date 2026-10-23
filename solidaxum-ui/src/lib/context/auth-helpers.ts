/** 权限判断 —— super-admin 通配 *:*:* */
export function hasPermission(permissions: string[], perm: string): boolean {
  return permissions.includes('*:*:*') || permissions.includes(perm)
}

/** 角色判断 —— admin 角色通配 */
export function hasRole(roles: string[], role: string): boolean {
  return roles.includes('admin') || roles.includes(role)
}
