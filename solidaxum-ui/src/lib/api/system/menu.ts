import { apiClient } from '$lib/api/client.js'
import type { RouterVo } from '$lib/api/types.js'

/**
 * 菜单列表行 / 详情 MenuListVo —— 对齐后端 (camelCase)。
 * 菜单为树形结构,后端返回 R<Vec<MenuListVo>> 已组装好 children。
 * menuType: M=目录 / C=菜单 / F=按钮
 * visible / status: "0"/"1" 字符串
 */
export interface MenuListVo {
  menuId: string
  menuName: string
  parentId: string
  orderNum: number
  path: string
  component?: string
  menuType: string
  visible: string
  status: string
  isFrame?: string
  isCache?: string
  perms?: string
  icon?: string
  remark?: string
  queryParam?: string
  activeMenu?: string
  children?: MenuListVo[]
}

/** POST /system/menu body */
export interface MenuCreateBo {
  menuName: string
  parentId: string
  orderNum: number
  path: string
  component?: string
  menuType: string
  visible: string
  status: string
  /** 必填: 后端 MenuCreateBo.is_frame 为非 Option String 且无 serde default,缺键即 400。0=是外链/1=否(Rust 语义,勿照抄 Vue 的 Y/N)。 */
  isFrame: string
  /** 必填: 同上,后端 is_cache 亦为非 Option String。0=缓存/1=不缓存。 */
  isCache: string
  perms?: string
  icon?: string
  remark?: string
  queryParam?: string
  activeMenu?: string
}

/** PUT /system/menu body */
export interface MenuUpdateBo extends MenuCreateBo {
  menuId: string
}

/**
 * GET /system/menu/roleMenuTreeselect/{roleId} 的树节点(后端 MenuTreeSelectVo)。
 * 注意与 MenuListVo 不同:这里是 id/label 归一化后的结构,且**按钮(menuType='F')也是节点**,
 * 由前端 buildPermissionTree 抽成父节点的按钮列。
 */
export interface MenuTreeSelectVo {
  id: string
  label: string
  parentId: string
  weight: number
  /** M=目录 C=菜单 F=按钮 */
  menuType: string
  /** 0=显示 1=隐藏 */
  visible: string
  /** 0=正常 1=停用 */
  status: string
  children?: MenuTreeSelectVo[]
}

/** 角色的菜单树 + 已勾选 id。checkedKeys 含后端存储的全部 id(包括半选父节点)。 */
export interface RoleMenuTreeSelectVo {
  checkedKeys: string[]
  menus: MenuTreeSelectVo[]
}

/**
 * 纯函数: 级联删除 URL(menu_ids 支持逗号分隔)。
 * 后端单删 delete_menu 对有子菜单的节点会 has_child 拒绝,
 * 非叶子节点必须走 cascade 接口(menu_route.rs:26-29)。
 */
export function cascadeDeleteUrl(menuIds: string): string {
  return `/system/menu/cascade/${menuIds}`
}

/** 树节点最小结构(MenuListVo 结构上兼容)。 */
export type MenuTreeNode = { menuId: string; children?: MenuTreeNode[] }

/**
 * 纯函数: 收集目标节点及全部子孙的 menuId。
 * 级联删除必须把子孙同批传入 —— 后端对齐 Java 排除语义后,
 * 子孙不在集合内仍会被 has_child_not_in 拒绝。
 */
export function collectMenuTreeIds(node: MenuTreeNode): string[] {
  return [node.menuId, ...(node.children ?? []).flatMap(collectMenuTreeIds)]
}

export const menuApi = {
  /**
   * 当前用户的动态路由菜单 (GET /system/menu/getRouters -> R<Vec<RouterVo>>)。
   * 登录后用于动态渲染 sidebar。RouterVo 树见 types.ts。
   */
  getRouters: () => apiClient.get<RouterVo[]>('/system/menu/getRouters'),
  /** 树形全量列表 (R<Vec<MenuListVo>>),非分页 */
  list: () => apiClient.get<MenuListVo[]>('/system/menu/list'),
  /** 详情 */
  getById: (id: string) => apiClient.get<MenuListVo>(`/system/menu/${id}`),
  /** 新建 */
  create: (data: MenuCreateBo) => apiClient.post<void>('/system/menu', data),
  /** 修改 */
  update: (data: MenuUpdateBo) => apiClient.put<void>('/system/menu', data),
  /** 删除单个(仅叶子节点;有子菜单的会被后端拒绝,请用 cascadeDelete) */
  delete: (id: string) => apiClient.delete<void>(`/system/menu/${id}`),
  /** 级联删除(连同全部子孙节点),menuIds 为逗号分隔,须含目标及全部子孙(见 collectMenuTreeIds) */
  cascadeDelete: (menuIds: string) => apiClient.delete<void>(cascadeDeleteUrl(menuIds)),
  /** 角色的菜单树 + 已勾选 id(分配权限用) */
  roleMenuTreeselect: (roleId: string) =>
    apiClient.get<RoleMenuTreeSelectVo>(
      `/system/menu/roleMenuTreeselect/${roleId}`
    ),
}
