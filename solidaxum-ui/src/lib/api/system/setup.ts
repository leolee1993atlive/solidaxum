import { apiClient } from '$lib/api/client.js'

/** GET /system/setup/status 返回体。 */
export interface SetupStatusVo {
  /** false = 首装未初始化,登录后应强导向导 */
  completed: boolean
}

/** POST /system/setup/complete 入参(全部必填,长度规则与后端对齐)。 */
export interface SetupCompleteBo {
  /** 系统名称 → sys.title(顶栏标题,全站可见),1-90 字 */
  systemName: string
  /** 管理员当前密码(首装即种子默认 admin123),6-50 位 */
  adminOldPassword: string
  adminNewPassword: string
  /** 用户默认密码 → sys.user.initPassword(批量导入新账号初始密码),6-50 位 */
  userInitPassword: string
}

const setupApi = {
  status() {
    return apiClient.get<SetupStatusVo>('/system/setup/status')
  },
  complete(bo: SetupCompleteBo) {
    return apiClient.post<void>('/system/setup/complete', bo)
  },
}

/**
 * 本会话是否已知"未完成初始化"(避免 (app) 布局每次挂载都打探测接口;
 * 一旦发现未完成就记住,直到向导完成把它清掉)。
 */
let knownIncomplete = false

/**
 * 登录成功后与 (app) 布局挂载时调用:未完成初始化 → true,调用方应
 * goto('/setup')。探测失败按"已完成"处理 —— 不让初始化探测故障
 * 把已初始化系统的用户挡在门外。
 */
export async function shouldRedirectToSetup(): Promise<boolean> {
  if (knownIncomplete) return true
  try {
    const resp = await setupApi.status()
    if (resp.data?.completed === false) {
      knownIncomplete = true
      return true
    }
    return false
  } catch {
    return false
  }
}

/** 向导成功完成后调用,清掉本会话的"未完成"记忆。 */
export function markSetupCompleted() {
  knownIncomplete = false
}

export { setupApi }
