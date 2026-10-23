import { apiClient } from '$lib/api/client.js'

/**
 * 流程分类 CategoryVo —— 对齐后端 (camelCase)。
 * 分类为树形结构,后端返回 R<Vec<CategoryVo>> 已组装好 children。
 */
export interface CategoryVo {
  categoryId: string
  parentId: string
  parentName?: string
  ancestors?: string
  categoryName: string
  orderNum: number
  createTime?: string
  children?: CategoryVo[]
}

/**
 * 分类树节点 CategoryTreeVo(`flow/src/model/dto.rs:785-792`)。
 * 形状与 `TreeSelectNode` 一致,可直接喂 TreePanel / TreeSelect。
 */
export interface CategoryTreeVo {
  id: string
  parentId: string
  label: string
  weight: number
  children?: CategoryTreeVo[]
}

/** POST /workflow/category body */
export interface CategoryCreateBo {
  categoryName: string
  parentId: string
  orderNum: number
}

/** PUT /workflow/category body */
export interface CategoryEditBo extends CategoryCreateBo {
  categoryId: string
}

export const categoryApi = {
  /**
   * 树形全量列表 (R<Vec<CategoryVo>>),非分页;categoryName 为可选模糊过滤。
   * 后端 CategoryQueryBo 支持 category_handler.rs:24-39。
   * 注意:apiClient.get 第二参是 Record<string,string>,undefined 会被 URLSearchParams
   * 序列化成字符串 "undefined",所以调用方要么不传、要么传空串之外的值。
   */
  list: (categoryName?: string) =>
    categoryName
      ? apiClient.get<CategoryVo[]>('/workflow/category/list', { categoryName })
      : apiClient.get<CategoryVo[]>('/workflow/category/list'),
  /** 分类树(TreeSelectNode 形状),供左树与弹窗 tree-select 共用 */
  categoryTree: () =>
    apiClient.get<CategoryTreeVo[]>('/workflow/category/categoryTree'),
  /** 详情 */
  getById: (id: string) => apiClient.get<CategoryVo>(`/workflow/category/${id}`),
  /** 新建 */
  create: (data: CategoryCreateBo) => apiClient.post<void>('/workflow/category', data),
  /** 修改 */
  update: (data: CategoryEditBo) => apiClient.put<void>('/workflow/category', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/workflow/category/${id}`),
}
