import { apiClient, TOKEN_KEY, getTokenClientId } from '$lib/api/client.js'
import { filenameFromDisposition } from './tool/gen-download.js'

/**
 * 从失败响应里取出后端文案。
 *
 * 这两个封装绕开了 apiClient(见下方各自的说明),所以也绕开了它的错误
 * 解包 —— 不主动读 body 的话,后端精心写的文案(如导出超过行数上限时的
 * 「请缩小查询范围」)会被丢掉,用户只看得到一个状态码。
 *
 * 错误体是 `R<()>` 信封(`{code, msg}`);但网关/代理返回的未必是 JSON,
 * 解析不出就退回状态码。用 `clone()` 以免占掉调用方可能还要读的 body。
 */
async function errorMessageFrom(
  resp: Response,
  fallbackPrefix: string
): Promise<string> {
  try {
    const body = (await resp.clone().json()) as {
      msg?: unknown
      message?: unknown
    }
    const msg = body.msg ?? body.message
    if (typeof msg === 'string' && msg.trim() !== '') return msg
  } catch {
    /* 非 JSON 响应(网关错误页等),退回状态码 */
  }
  return `${fallbackPrefix}: ${resp.status}`
}

/**
 * 带鉴权头取 blob 并触发浏览器下载。
 * 后端需要 Authorization 头,不能用裸 <a href> 直链(会 401)。
 * 用法: gen 的 zip 下载、oss 的文件下载、流程定义导出(POST)。
 *
 * ⚠️ 抛的是裸 Error,不经 apiClient —— 全局 toast 管不到它。
 *    调用方的 catch 里**必须自己 toast.error**,否则下载失败完全静默。
 */
export async function downloadBlob(
  endpoint: string,
  fallbackName: string,
  method: 'GET' | 'POST' = 'GET'
): Promise<void> {
  const base = apiClient.getBaseUrl()
  const token = typeof localStorage !== 'undefined' ? localStorage.getItem(TOKEN_KEY) ?? '' : ''
  const resp = await fetch(`${base}${endpoint}`, {
    method,
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!resp.ok) throw new Error(await errorMessageFrom(resp, '下载失败'))
  const blob = await resp.blob()
  const name = filenameFromDisposition(resp.headers.get('content-disposition') ?? '', fallbackName)
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  a.click()
  URL.revokeObjectURL(url)
}

/**
 * 筛选参数 → form-urlencoded body。剥掉 undefined/null/空串。
 * 空串必须剥:后端 Option<String> 收到 Some("") 不等于 None,会筛出零行。
 * 0 / false 要留:它们是有效筛选值(如 isFrame=0 表示外链)。
 */
export function toFormBody(
  extra?: Record<string, string | number | undefined | null>
): string {
  const params = new URLSearchParams()
  if (extra) {
    for (const [k, v] of Object.entries(extra)) {
      if (v !== undefined && v !== null && v !== '') params.append(k, String(v))
    }
  }
  return params.toString()
}

/**
 * 导出 xlsx。全站统一的「form-urlencoded body + blob 下载」封装。
 *
 * 为什么不能走 apiClient.post:它强制 JSON.parse 响应体并校验 code===200,
 * 二进制 blob 在那步就炸了。为什么不能走 downloadBlob:它不带 body,
 * 而这些端点用 Form<...> 提取筛选条件(不是 JSON —— 用 AppJson 会 400)。
 *
 * ⚠️ 抛的是裸 Error,不经 apiClient —— 全局 toast 管不到它。
 *    调用方的 catch 里**必须自己 toast.error**,否则导出失败完全静默。
 */
export async function exportBlob(
  endpoint: string,
  fallbackName: string,
  extra?: Record<string, string | number | undefined | null>
): Promise<void> {
  const base = apiClient.getBaseUrl()
  const token = typeof localStorage !== 'undefined' ? localStorage.getItem(TOKEN_KEY) ?? '' : ''
  const resp = await fetch(`${base}${endpoint}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      Authorization: `Bearer ${token}`,
      clientid: getTokenClientId(),
    },
    body: toFormBody(extra),
  })
  if (!resp.ok) throw new Error(await errorMessageFrom(resp, '导出失败'))
  const blob = await resp.blob()
  const name = filenameFromDisposition(
    resp.headers.get('content-disposition') ?? '',
    fallbackName
  )
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  a.click()
  URL.revokeObjectURL(url)
}
