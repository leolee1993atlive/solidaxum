/**
 * system 主数据四页的表单校验 —— 逐条移植 Vue 的 `rules`,抽成纯函数便于单测。
 *
 * 为什么不内联在页面里:Vue 的 rules 是这些字段唯一的防线(后端 validate 更宽松,
 * 例如 dept 的 phone/email 只有长度限制、没有格式校验),写错了不会有任何信号。
 * 抽出来 + 单测钉住,才不会在下一次改表单时被顺手删掉。
 */

/** RuoYi 全站统一的手机号正则(Vue user/dept 两页同款)。 */
export const PHONE_RE = /^1[3456789][0-9]\d{8}$/
/** el-form `type: 'email'` 的等价实现。 */
export const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
/** 用户密码禁用字符(Vue: 不能包含 < > " ' \ |)。 */
export const PWD_ILLEGAL_RE = /[<>"'|\\]/

export type DeptFormLike = {
  parentId?: string
  deptName?: string
  orderNum?: number
  phone?: string
  email?: string
}

/** 对齐 Vue dept/index.vue:248-266。返回空数组表示通过。 */
export function deptFormIssues(form: DeptFormLike): string[] {
  const issues: string[] = []
  if (!(form.parentId ?? '').trim()) issues.push('上级部门不能为空')
  if (!(form.deptName ?? '').trim()) issues.push('部门名称不能为空')
  if (form.orderNum === undefined || form.orderNum === null || Number.isNaN(form.orderNum)) {
    issues.push('显示排序不能为空')
  }
  const email = (form.email ?? '').trim()
  if (email && !EMAIL_RE.test(email)) issues.push('请输入正确的邮箱地址')
  const phone = (form.phone ?? '').trim()
  if (phone && !PHONE_RE.test(phone)) issues.push('请输入正确的手机号码')
  return issues
}

export type UserFormLike = {
  userName?: string
  nickName?: string
  password?: string
  email?: string
  phoneNumber?: string
  roleIds?: string[]
}

/**
 * 对齐 Vue user/index.vue:551-591。
 * @param opts.isEdit 编辑态不校验密码(表单里没有这个字段)
 * @param opts.editingSelf 编辑自己时归属部门/角色/岗位整块隐藏,故不校验 roleIds
 */
export function userFormIssues(
  form: UserFormLike,
  opts: { isEdit: boolean; editingSelf?: boolean }
): string[] {
  const issues: string[] = []
  const userName = (form.userName ?? '').trim()
  if (!userName) {
    issues.push('用户名称不能为空')
  } else if (userName.length < 2 || userName.length > 20) {
    issues.push('用户名称长度必须介于 2 和 20 之间')
  }
  if (!(form.nickName ?? '').trim()) issues.push('用户昵称不能为空')

  if (!opts.isEdit) {
    const pwd = form.password ?? ''
    if (!pwd) {
      issues.push('用户密码不能为空')
    } else {
      if (pwd.length < 5 || pwd.length > 20) {
        issues.push('用户密码长度必须介于 5 和 20 之间')
      }
      if (PWD_ILLEGAL_RE.test(pwd)) {
        issues.push('密码不能包含非法字符：< > " \' \ |')
      }
    }
  }

  const email = (form.email ?? '').trim()
  if (email && !EMAIL_RE.test(email)) issues.push('请输入正确的邮箱地址')
  const phone = (form.phoneNumber ?? '').trim()
  if (phone && !PHONE_RE.test(phone)) issues.push('请输入正确的手机号码')

  // 🔴 Vue 的 roleIds required 是唯一防线:MultiSelect 可以被取消勾选到空,
  // 后端 UserCreateBo.role_ids 无 #[validate],空数组会静默存成"没有任何角色的用户"。
  if (!opts.editingSelf && (form.roleIds ?? []).length === 0) {
    issues.push('用户角色不能为空')
  }
  return issues
}

/** 对齐 Vue handleResetPwd 的 inputPattern(/^.{5,20}$/) + inputValidator。 */
export function resetPwdIssues(pwd: string): string[] {
  const issues: string[] = []
  if (pwd.length < 5 || pwd.length > 20) {
    issues.push('用户密码长度必须介于 5 和 20 之间')
  }
  if (PWD_ILLEGAL_RE.test(pwd)) {
    issues.push('密码不能包含非法字符：< > " \' \ |')
  }
  return issues
}
