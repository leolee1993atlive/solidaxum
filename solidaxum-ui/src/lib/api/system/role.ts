import { apiClient } from '$lib/api/client.js'
import { exportBlob } from '$lib/api/download.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import type { UserListVo } from './user.js'

/** 列表行 / 详情 RoleListVo —— 对齐后端 (camelCase)。dataScope 为 "1".."5" 字符串。 */
export interface RoleListVo {
  roleId: string
  roleName: string
  roleKey: string
  roleSort: number
  dataScope?: string
  status: string
  menuCheckStrictly?: boolean
  deptCheckStrictly?: boolean
  remark?: string
  createTime?: string
}

/** POST /system/role body */
export interface RoleCreateBo {
  roleName: string
  roleKey: string
  roleSort?: number
  dataScope?: string
  status: string
  remark?: string
}

/** PUT /system/role body */
export interface RoleUpdateBo extends RoleCreateBo {
  roleId: string
}

/**
 * 超级管理员角色 id —— 与后端 `role_service.rs` 的 `SUPER_ADMIN_ROLE_ID` 同值。
 * 后端 `check_role_allowed` 对该角色的改/删/授权一律 400，前端据此隐藏入口(对齐 Vue index.vue:120,129,138,147)。
 */
export const SUPER_ADMIN_ROLE_ID = '1761300000000000001'

/** PUT /system/role/permission body —— 菜单权限与数据权限合并提交。 */
export interface RolePermissionBo {
  roleId: string
  menuIds: string[]
  deptIds: string[]
  dataScope: string
  /** 父子联动开关(true=联动)。后端持久化在 sys_role 上,下次打开要用它还原开关。 */
  menuCheckStrictly: boolean
  deptCheckStrictly: boolean
}

/** GET /system/role/deptTree/{roleId} 返回 —— 部门树 + 已勾选 id。 */
export interface RoleDeptTreeVo {
  checkedKeys: string[]
  depts: { id: string; label: string; children?: RoleDeptTreeVo['depts'] }[]
}

/**
 * 纯函数: 批量授权/取消授权的 query 串。
 * 后端 `AuthUserBatchQuery` 走 Query 提取(不是 body),`userIds` 为逗号分隔。
 */
export function authUserBatchQuery(roleId: string, userIds: string[]): string {
  return new URLSearchParams({
    roleId: String(roleId),
    userIds: userIds.join(','),
  }).toString()
 }

export const roleApi = {
  /** 分页列表 */
  page: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<RoleListVo>('/system/role/page', toPageQuery(q, extra)),
  /** 全量列表 (R<Vec>),供用户表单等下拉用 */
  list: () => apiClient.get<RoleListVo[]>('/system/role'),
  /** 详情 */
  getById: (id: string) => apiClient.get<RoleListVo>(`/system/role/${id}`),
  /** 新建 */
  create: (data: RoleCreateBo) => apiClient.post<void>('/system/role', data),
  /** 修改 */
  update: (data: RoleUpdateBo) => apiClient.put<void>('/system/role', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/role/${id}`),
  /** 切换状态 */
  changeStatus: (roleId: string, status: string) =>
    apiClient.put<void>('/system/role/changeStatus', { roleId, status }),
  /**
   * R9 导出。后端 POST /system/role/export + Form<RolePageQuery>(role_handler.rs:460-465),
   * 权限 system:role:export。⚠️ exportBlob 抛裸 Error,调用方 catch 里必须自己 toast。
   */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/system/role/export', `role_${Date.now()}.xlsx`, extra),
  /** 分配权限:菜单 + 数据范围合并更新 */
  updatePermission: (data: RolePermissionBo) =>
    apiClient.put<void>('/system/role/permission', data),
  /** 角色的部门树 + 已勾选部门 id(数据权限自定义用) */
  deptTree: (roleId: string) =>
    apiClient.get<RoleDeptTreeVo>(`/system/role/deptTree/${roleId}`),
  /** 已授权用户分页。后端 `role_handler.rs:361` 返回 TableDataInfo<UserListVo>,行就是用户。 */
  allocatedList: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<UserListVo>(
      '/system/role/authUser/allocatedList',
      toPageQuery(q, extra)
    ),
  /** 未授权用户分页。同上,行是 UserListVo。 */
  unallocatedList: (
    q: PageQuery,
    extra?: Record<string, string | number | undefined | null>
  ) =>
    apiClient.getList<UserListVo>(
      '/system/role/authUser/unallocatedList',
      toPageQuery(q, extra)
    ),
  /** 取消单个用户授权(PUT + JSON body) */
  cancelAuthUser: (roleId: string, userId: string) =>
    apiClient.put<void>('/system/role/authUser/cancel', { roleId, userId }),
  /** 批量取消授权(PUT + query 参数,body 空对象) */
  cancelAuthUsers: (roleId: string, userIds: string[]) =>
    apiClient.put<void>(
      `/system/role/authUser/cancelAll?${authUserBatchQuery(roleId, userIds)}`,
      {}
    ),
  /** 批量授予角色(PUT + query 参数,body 空对象) */
  selectAuthUsers: (roleId: string, userIds: string[]) =>
    apiClient.put<void>(
      `/system/role/authUser/selectAll?${authUserBatchQuery(roleId, userIds)}`,
      {}
    ),
}
