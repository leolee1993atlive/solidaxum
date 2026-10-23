// 契约层
export {
  apiClient,
  ApiClientNotifiedError,
  isApiClientNotifiedError,
  TOKEN_KEY,
  CLIENT_ID_KEY,
} from './client'
export type * from './types'
export { toPageQuery } from './types'

// 领域 API (随 M2 逐步补)
export { authApi } from './auth'
