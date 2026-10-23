import { apiClient } from '$lib/api/client.js'
import { exportBlob } from '$lib/api/download.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import type { TreeSelectNode } from '$lib/components/tree-select-types.js'

/** 列表行 / 详情 PostListVo —— 对齐后端 (camelCase)。 */
export interface PostListVo {
  postId: string
  deptId?: string
  postCode: string
  postName: string
  postSort: number
  status: string
  remark?: string
  deptName?: string
  postCategory?: string
  createTime?: string
}

/** POST /system/post body */
export interface PostCreateBo {
  deptId?: string
  postCode: string
  postName: string
  postCategory?: string
  postSort?: number
  status?: string
  remark?: string
}

/** PUT /system/post body */
export interface PostUpdateBo extends PostCreateBo {
  postId: string
}

export const postApi = {
  /** 分页列表 —— 后端 R<TableDataInfo<PostListVo>>,用 getList 解嵌套。 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<PostListVo>('/system/post/list', toPageQuery(q, extra)),
  /** 详情 */
  getById: (id: string) => apiClient.get<PostListVo>(`/system/post/${id}`),
  /** 新建 */
  create: (data: PostCreateBo) => apiClient.post<void>('/system/post', data),
  /** 修改 */
  update: (data: PostUpdateBo) => apiClient.put<void>('/system/post', data),
  /** 删除 */
  delete: (id: string) => apiClient.delete<void>(`/system/post/${id}`),
  /** 部门树,供左侧面板与弹窗部门选择使用 */
  deptTree: () => apiClient.get<TreeSelectNode[]>('/system/post/deptTree'),
  /** PO10 导出。后端 POST /system/post/export(post_handler.rs:215-222),会展开 belongDeptId 子树。 */
  export: (extra?: Record<string, string | number | undefined | null>) =>
    exportBlob('/system/post/export', `post_${Date.now()}.xlsx`, extra),
}
