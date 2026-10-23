import { connectionStore } from '$lib/stores/connection.js'
import { toast } from 'svelte-sonner'
import type { ApiResponse, TableData, ListResult } from './types'
import {
  TOKEN_KEY,
  CLIENT_ID_KEY,
  DEFAULT_CLIENT_ID,
  LOGIN_ENDPOINT,
} from '$lib/brand.js'

export { TOKEN_KEY, CLIENT_ID_KEY }

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? '/api'
const USE_MOCK_API = import.meta.env.VITE_USE_MOCK_API === 'true'

export class ApiClientNotifiedError extends Error {
  override readonly name = 'ApiClientNotifiedError'
  constructor(message: string) {
    super(message)
  }
}

export function isApiClientNotifiedError(
  e: unknown
): e is ApiClientNotifiedError {
  return e instanceof ApiClientNotifiedError
}

/**
 * 单次请求的行为开关。
 *
 * `silent`:抑制**本次**失败的全局 toast(仍照常抛 `ApiClientNotifiedError`,调用方
 * 自己汇总报告)。用于「后端无批量端点、前端循环逐条发」的场景 —— 否则失败 N 条
 * 就刷 N 个红 toast(见 taskApi.urgeMany)。会话过期(401)不受影响,一律提示。
 */
export type RequestOptions = {
  silent?: boolean
  /** 额外请求头,与默认头合并(同名覆盖默认值)。 */
  headers?: Record<string, string>
}

/** 读当前会话 token。SSR / 无 localStorage 环境返回 null。 */
export function getToken(): string | null {
  if (typeof localStorage === 'undefined') return null
  return localStorage.getItem(TOKEN_KEY)
}

export function getTokenClientId(): string {
  if (typeof localStorage === 'undefined') return DEFAULT_CLIENT_ID
  return localStorage.getItem(CLIENT_ID_KEY) || DEFAULT_CLIENT_ID
}

/** 清空会话痕迹(token + clientid)。 */
export function clearToken() {
  if (typeof localStorage !== 'undefined') {
    localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(CLIENT_ID_KEY)
  }
}

function redirectToLogin() {
  if (typeof window !== 'undefined') {
    toast.error('会话已过期，请重新登录')
    // 登录页位于路由根 / (route group (auth) 不参与 URL)。
    window.location.href = '/?session=expired'
  }
}

function failWithToast(
  endpoint: string,
  message: string,
  silent = false
): never {
  // 登录接口的错误由调用方处理，不全局 toast（端点经 brand 覆写点导出）
  if (!silent && !endpoint.startsWith(LOGIN_ENDPOINT)) toast.error(message)
  throw new ApiClientNotifiedError(message)
}

function endpointPath(endpoint: string): string {
  const i = endpoint.indexOf('?')
  return i === -1 ? endpoint : endpoint.slice(0, i)
}

class ApiClient {
  private baseUrl: string
  private useMock: boolean

  constructor(baseUrl: string, useMock: boolean) {
    this.baseUrl = baseUrl
    this.useMock = useMock
  }

  getBaseUrl(): string {
    return this.baseUrl
  }

  private buildHeaders(init?: HeadersInit): Record<string, string> {
    const h: Record<string, string> = {
      'Content-Type': 'application/json',
      clientid: getTokenClientId(),
    }
    const token = getToken()
    if (token) h['Authorization'] = `Bearer ${token}`
    const provided = init as Record<string, string> | undefined
    if (provided) Object.assign(h, provided)
    return h
  }

  /** request 内部: 若 body 是 FormData,去掉默认 Content-Type(让浏览器设 boundary)。 */
  private mergeHeaders(init: RequestInit): Record<string, string> {
    const h = this.buildHeaders(init.headers)
    if (init.body instanceof FormData) {
      delete h['Content-Type']
    }
    return h
  }

  private async request<T>(
    endpoint: string,
    init: RequestInit = {},
    opts: RequestOptions = {}
  ): Promise<ApiResponse<T>> {
    if (this.useMock) {
      return { code: 200, msg: 'Success', data: {} as T }
    }
    const url = `${this.baseUrl}${endpoint}`
    const path = endpointPath(endpoint)
    try {
      const response = await fetch(url, {
        ...init,
        headers: this.mergeHeaders(init),
      })
      connectionStore.markConnected()

      if (response.status === 401) {
        clearToken()
        redirectToLogin()
        throw new ApiClientNotifiedError('未授权')
      }

      const bodyText = await response.text()
      let parsed: ApiResponse<T> | null = null
      try {
        parsed = bodyText ? (JSON.parse(bodyText) as ApiResponse<T>) : null
      } catch {
        parsed = null
      }

      if (!response.ok) {
        const msg =
          parsed?.msg ||
          parsed?.message ||
          bodyText.slice(0, 200) ||
          `请求失败（HTTP ${response.status}）`
        failWithToast(path, msg, opts.silent)
      }
      if (!parsed) {
        failWithToast(path, '服务器返回了空响应', opts.silent)
      }
      if (parsed.code !== 200) {
        failWithToast(
          path,
          parsed.msg || parsed.message || `业务错误（${parsed.code}）`,
          opts.silent
        )
      }
      return parsed
    } catch (error) {
      if (isApiClientNotifiedError(error)) throw error
      if (error instanceof TypeError && /fetch/i.test(error.message)) {
        connectionStore.markDisconnected()
        failWithToast(
          path,
          '无法连接到服务器，请检查后端服务是否运行',
          opts.silent
        )
      }
      throw error
    }
  }

  async get<T>(
    endpoint: string,
    params?: Record<string, string>
  ): Promise<ApiResponse<T>> {
    const qs = params ? `?${new URLSearchParams(params)}` : ''
    return this.request<T>(`${endpoint}${qs}`, { method: 'GET' })
  }

  async post<T>(
    endpoint: string,
    data: unknown,
    opts?: RequestOptions
  ): Promise<ApiResponse<T>> {
    return this.request<T>(
      endpoint,
      { method: 'POST', body: JSON.stringify(data), headers: opts?.headers },
      opts
    )
  }

  /**
   * multipart/form-data 上传: 不设 Content-Type(让浏览器自动带 boundary),
   * 仍注入 clientid + Authorization(mergeHeaders 检测 FormData 跳过 Content-Type)。
   */
  async postForm<T>(
    endpoint: string,
    formData: FormData
  ): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'POST', body: formData })
  }

  async put<T>(endpoint: string, data: unknown): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, {
      method: 'PUT',
      body: JSON.stringify(data),
    })
  }

  async delete<T>(endpoint: string): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'DELETE' })
  }

  /** 列表接口：解析嵌套 R<TableDataInfo<T>>，返回扁平 {rows, total}。 */
  async getList<T>(
    endpoint: string,
    params?: Record<string, string>
  ): Promise<ListResult<T>> {
    const qs = params ? `?${new URLSearchParams(params)}` : ''
    const resp = await this.request<TableData<T>>(`${endpoint}${qs}`, {
      method: 'GET',
    })
    const data = resp.data
    if (!data) return { rows: [], total: 0 }
    return {
      rows: data.rows ?? [],
      total: data.total ?? 0,
      // 计数口径原样透传:丢在这里的话,视图只能看到一个"恰好 10000"的
      // 总数,分不清是真有一万条还是被上限截断了。
      totalCapped: data.totalCapped,
      totalApprox: data.totalApprox,
    }
  }
}

export const apiClient = new ApiClient(API_BASE_URL, USE_MOCK_API)
