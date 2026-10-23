import { apiClient } from '$lib/api/client.js'

/** 三方账号绑定(对齐后端 SocialVo,`crates/system/src/social/model/social.rs`)。 */
export interface SocialVo {
  id: string
  userId: string
  authId?: string
  source?: string
  openId?: string
  userName?: string
  nickName?: string
  avatar?: string
  createTime?: string
}

export const socialApi = {
  /** 当前用户已绑定的三方账号 */
  list: () => apiClient.get<SocialVo[]>('/system/social/list'),
  /** 解绑。后端 DELETE /auth/unlock/{socialId}(router.rs:163-164) */
  unlock: (socialId: string) => apiClient.delete<void>(`/auth/unlock/${socialId}`),
}
