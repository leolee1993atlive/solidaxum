import { apiClient } from './client'
import type { CaptchaVo, LoginBody, LoginVo } from './types'

export const authApi = {
  /** GET /auth/code -> { captchaEnabled, uuid?, img? } */
  getCaptcha: () => apiClient.get<CaptchaVo>('/auth/code'),

  /** POST /auth/login, body camelCase, 响应 LoginVo (snake_case) */
  login: (data: LoginBody) => apiClient.post<LoginVo>('/auth/login', data),

  /** POST /auth/logout */
  logout: () => apiClient.post<void>('/auth/logout', null),
}
