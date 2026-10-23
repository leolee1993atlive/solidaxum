import { apiClient } from '$lib/api/client.js'
import { exportBlob, downloadBlob } from '$lib/api/download.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import type { TreeSelectNode } from '$lib/components/tree-select-types.js'

/** 列表行 UserListVo —— 后端 #[serde(rename = "phoneNumber")] 使 JSON 键为 phoneNumber */
export interface UserListVo {
  userId: string
  deptId?: string
  userName: string
  nickName: string
  email?: string
  phoneNumber?: string
  gender?: string
  status: string
  delFlag?: string
  remark?: string
  deptName?: string
  createByName?: string
  createTime?: string
}

/** SysUserVo (详情中的 user,比列表行多了头像/登录信息) */
export interface SysUserVo {
  userId: string
  deptId?: string
  userName: string
  nickName: string
  userType?: string
  email?: string
  phoneNumber?: string
  gender?: string
  avatar?: string
  avatarUrl?: string
  status: string
  loginIp?: string
  loginDate?: string
  remark?: string
  deptName?: string
  createTime?: string
  /** 后端 SysUserVo 展平了 BaseVo(`core/src/model/entity.rs:20-31`),故有 update_time。 */
  updateTime?: string
}

export interface RoleBrief {
  roleId: string
  roleName: string
  roleKey: string
  status: string
}
export interface PostBrief {
  postId: string
  postName: string
  postCode: string
  status: string
}

/** GET /system/user/{id} 或 GET /system/user/ 的返回 */
export interface SysUserInfoVo {
  user: SysUserVo
  roleIds: string[]
  postIds: string[]
  roles: RoleBrief[]
  posts: PostBrief[]
}

/** GET /system/user/authRole/{id} 的返回(user_handler.rs:455-475 手工拼的 json)。 */
export interface AuthRoleVo {
  user: { userId: string; userName: string; nickName: string }
  roles: {
    roleId: string
    roleName: string
    roleKey: string
    status: string
    createTime?: string
    /** 该角色是否已授予此用户 —— 后端算好的,前端据此预勾选。 */
    flag: boolean
  }[]
}

/**
 * 超级管理员 user_id —— 与后端 `crates/core/src/lib.rs:1` 的 SUPER_ADMIN_USER_ID 同值。
 * 后端 `check_user_allowed` 对该用户的改/删/重置密码一律 400,前端据此隐藏入口
 * (对齐 Vue index.vue:219,228,238,250 的四处 v-if)。
 */
export const SUPER_ADMIN_USER_ID = '1761100000000000001'

/** POST /system/user body */
export interface UserCreateBo {
  userName: string
  password: string
  deptId?: string
  nickName: string
  phoneNumber?: string
  email?: string
  gender?: string
  status?: string
  remark?: string
  postIds?: string[]
  roleIds?: string[]
}

/** PUT /system/user body */
export interface UserUpdateBo {
  userId: string
  userName: string
  deptId?: string
  nickName: string
  phoneNumber?: string
  email?: string
  gender?: string
  status?: string
  remark?: string
  postIds?: string[]
  roleIds?: string[]
}

export const userApi = {
  /** 分页列表 —— 后端 /system/user/list 与 /system/user/page 同义,这里对齐 dict 的 /list 约定 */
  page: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<UserListVo>('/system/user/list', toPageQuery(q, extra)),
  /** 全量列表 (R<Vec>) */
  list: () => apiClient.get<UserListVo[]>('/system/user'),
  /** 新建表单初始化:拉取可选角色/岗位 (无 userId,末尾斜杠) */
  getAddInfo: () => apiClient.get<SysUserInfoVo>('/system/user/'),
  /** 详情 */
  getById: (id: string) => apiClient.get<SysUserInfoVo>(`/system/user/${id}`),
  create: (data: UserCreateBo) => apiClient.post<void>('/system/user', data),
  update: (data: UserUpdateBo) => apiClient.put<void>('/system/user', data),
  /** 删除(支持逗号分隔的批量 id,这里单删) */
  delete: (id: string) => apiClient.delete<void>(`/system/user/${id}`),
  resetPwd: (userId: string, password: string) =>
    apiClient.put<void>('/system/user/resetPwd', { userId, password }),
  changeStatus: (userId: string, status: string) =>
    apiClient.put<void>('/system/user/changeStatus', { userId, status }),
  /** 导出。 */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/system/user/export', `user_${Date.now()}.xlsx`, extra),
  /** 解锁。后端 GET /system/user/unlock/{id},权限 system:user:edit。 */
  unlock: (userId: string) => apiClient.get<void>(`/system/user/unlock/${userId}`),
  /** 下载导入模板。后端 GET,无守卫,返回 xlsx。 */
  importTemplate: () =>
    downloadBlob('/system/user/importTemplate', `user_template_${Date.now()}.xlsx`),
  /**
   * 导入。multipart,字段名固定 file(user_handler.rs:711 只认这一个 name)。
   * ⚠️ Vue 还往 URL 上拼 updateSupport —— 后端**不消费**,别发,更别让 UI 声称支持。
   * 返回值是纯文本导入结果("导入成功 N 条, 失败 M 条\n..."),直接展示即可。
   */
  importData: (file: File) => {
    const fd = new FormData()
    fd.append('file', file)
    return apiClient.postForm<string>('/system/user/importData', fd)
  },
  /** U-R1 分配角色页数据。GET /system/user/authRole/{id},权限 system:user:query。 */
  getAuthRole: (userId: string) =>
    apiClient.get<AuthRoleVo>(`/system/user/authRole/${userId}`),
  /**
   * U-R1 提交授权。后端 PUT /system/user/authRole + **Query 参数**(不是 body!
   * AuthRoleQuery 用 Query 提取,user_handler.rs:486-490),roleIds 为逗号串。
   */
  updateAuthRole: (userId: string, roleIds: string[]) =>
    apiClient.put<void>(
      `/system/user/authRole?${new URLSearchParams({ userId, roleIds: roleIds.join(',') }).toString()}`,
      {}
    ),
  /** 用户下拉候选(供选人组件用)。后端 /system/user/optionselect */
  optionselect: () =>
    apiClient.get<UserListVo[]>('/system/user/optionselect'),
  /** 部门树(按当前用户数据权限过滤),TreeSelectNode 结构可直接喂 TreeSelect/TreePanel */
  deptTree: () => apiClient.get<TreeSelectNode[]>('/system/user/deptTree'),
}
