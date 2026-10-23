import { apiClient } from '$lib/api/client.js'

/** 个人信息(对齐后端 ProfileUserVo,`crates/system/src/user/model/user.rs:205-220`)。 */
export interface ProfileUserVo {
  userId: string
  deptId?: string
  userName: string
  nickName: string
  userType?: string
  email?: string
  phoneNumber?: string
  gender?: string
  /** 头像 OSS ID(原始值) */
  avatar?: string
  /** 头像 URL(后端翻译后) */
  avatarUrl?: string
  loginIp?: string
  loginDate?: string
  createTime?: string
  deptName?: string
}

/** GET /system/user/profile 的响应(`user.rs:199-203`)。 */
export interface ProfileVo {
  user: ProfileUserVo
  roleGroup: string
  postGroup: string
}

/** PUT /system/user/profile 的可改字段(头像单独走上传后回写 avatar)。 */
export interface ProfileUpdateBody {
  nickName?: string
  email?: string
  phoneNumber?: string
  gender?: string
  avatar?: string
}

export interface PasswordForm {
  oldPassword: string
  newPassword: string
  confirmPassword: string
}

/** 新密码非法字符,逐字抄自 Vue resetPwd.vue:48-51 */
const ILLEGAL_PWD_CHARS = /[<>"'\\|]/

/**
 * 密码表单的问题列表(空数组 = 通过)。文案逐条抄自 Vue `resetPwd.vue:39-60`,
 * 不要自拟 —— 后端 `SysUserPasswordBo` 只校验 6-50 长度,前端这套更严的规则
 * 是 RuoYi 的既有约定。
 */
export function passwordIssues(form: PasswordForm): string[] {
  const issues: string[] = []
  if (!form.oldPassword) issues.push('旧密码不能为空')
  if (!form.newPassword) {
    issues.push('新密码不能为空')
  } else {
    if (form.newPassword.length < 6 || form.newPassword.length > 20) {
      issues.push('长度在 6 到 20 个字符')
    }
    if (ILLEGAL_PWD_CHARS.test(form.newPassword)) {
      issues.push('不能包含非法字符：< > " \' \\ |')
    }
  }
  if (!form.confirmPassword) {
    issues.push('确认密码不能为空')
  } else if (form.newPassword && form.confirmPassword !== form.newPassword) {
    issues.push('两次输入的密码不一致')
  }
  return issues
}

export const profileApi = {
  /** 当前登录用户的个人信息 */
  get: () => apiClient.get<ProfileVo>('/system/user/profile'),
  /** 修改个人信息 */
  update: (body: ProfileUpdateBody) =>
    apiClient.put<void>('/system/user/profile', body),
  /** 修改密码。后端字段名以 `SysUserPasswordBo`(user.rs:238-243)为准 */
  updatePwd: (oldPassword: string, newPassword: string) =>
    apiClient.put<void>('/system/user/profile/updatePwd', {
      oldPassword,
      newPassword,
    }),
}
