import { apiClient } from '$lib/api/client.js'
import { toPageQuery } from '$lib/api/types.js'
import type { PageQuery } from '$lib/api/types.js'
import { downloadBlob } from '$lib/api/download.js'
/** 列表行 SysOssVo —— 对齐后端 /resource/oss/list (camelCase)。 */
export interface SysOssVo {
  ossId: string
  fileName: string
  originalName: string
  fileSuffix: string
  url: string
  service: string
  createByName?: string
  createTime?: string
}

/** POST /resource/oss/upload -> OssUploadVo */
export interface OssUploadVo {
  ossId: string
  url: string
  fileName: string
  originalName: string
  fileSuffix: string
  service: string
}


/** 可在列表里直接缩略展示的图片后缀(对齐 Vue oss/index.vue 的 checkFileSuffix)。 */
const IMAGE_SUFFIXES = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp'])

/** 后缀是否为图片。容忍前导点与大小写。 */
export function isImageSuffix(suffix?: string): boolean {
  if (!suffix) return false
  return IMAGE_SUFFIXES.has(suffix.replace(/^\./, '').toLowerCase())
}

export const ossApi = {
  /** 分页列表 */
  list: (q: PageQuery, extra?: Record<string, string | number | undefined | null>) =>
    apiClient.getList<SysOssVo>('/resource/oss/list', toPageQuery(q, extra)),
  /** multipart 上传: 字段名 file, 返回上传结果(含 url)。 */
  upload: (file: File) => {
    const fd = new FormData()
    fd.append('file', file)
    return apiClient.postForm<OssUploadVo>('/resource/oss/upload', fd)
  },
  /** 删除: 支持逗号分隔的多个 ossId。 */
  delete: (ossIds: string) => apiClient.delete<void>(`/resource/oss/${ossIds}`),
  /**
   * 下载文件。后端 `GET /resource/oss/download/{ossId}` 需要
   * Authorization + clientid 头,不能用裸 <a href> 直链。
   */
  download: (ossId: string, fileName: string) =>
    downloadBlob(`/resource/oss/download/${ossId}`, fileName),
}

