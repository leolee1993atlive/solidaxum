import { apiClient } from '$lib/api/client.js'
import type { UserListVo } from './user.js'

/**
 * 部门列表行 / 详情 DeptListVo —— 对齐后端 (camelCase)。
 * 部门为树形结构,后端返回 R<Vec<DeptListVo>> 已组装好 children。
 * status: "0"/"1" 字符串 (0=正常, 1=停用)
 */
export interface DeptListVo {
  deptId: string
  parentId: string
  deptName: string
  /** 类别编码,后端 DeptListVo.dept_category 一直有,前端此前未用过 */
  deptCategory?: string
  orderNum: number
  /**
   * 🔴 负责人 = user_id。后端 is_id_key 把 "leader" 列为语义 ID 键(response.rs:110),
   * 出站恒为字符串 —— 声明成 number 会在 19 位雪花 id 上静默截断。
   */
  leader?: string
  /** 服务端已翻译(dept_handler.rs:20-49 fill_leader_and_parent_names) */
  leaderName?: string
  parentName?: string
  phone?: string
  email?: string
  status: string
  createTime?: string
  children?: DeptListVo[]
}

/** POST /system/dept body */
export interface DeptCreateBo {
  parentId: string
  deptName: string
  deptCategory?: string
  orderNum: number
  leader?: string
  phone?: string
  email?: string
  status?: string
}

/** PUT /system/dept body */
export interface DeptUpdateBo extends DeptCreateBo {
  deptId: string
}

export const deptApi = {
  /** 树形全量列表 (R<Vec<DeptListVo>>),非分页 */
  list: () => apiClient.get<DeptListVo[]>('/system/dept/list'),
  /** 详情 */
  getById: (id: string) => apiClient.get<DeptListVo>(`/system/dept/${id}`),
  /** 新建 */
  create: (data: DeptCreateBo) => apiClient.post<void>('/system/dept', data),
  /** 修改 */
  update: (data: DeptUpdateBo) => apiClient.put<void>('/system/dept', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/dept/${id}`),
  /**
   * 某部门下的用户(供「负责人」下拉)。后端 GET /system/user/list/dept/{deptId},
   * 返回裸数组(R<Vec<UserListVo>>),非分页 —— 见 user_handler.rs:679-696。
   */
  listUserByDept: (deptId: string) =>
    apiClient.get<UserListVo[]>(`/system/user/list/dept/${deptId}`),
}

