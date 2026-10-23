/**
 * 业务表单页跳转 URL 构造 —— 对齐 Vue 的 workflowCommon.routerJump，
 * 但吸收 Vue 路由与 SvelteKit 路由的三处形状差异。
 *
 * Vue:    router.push({ path: formPath, query: { id, type, taskId } })
 *         formPath 来自 DB(flow_definition.form_path)，实测六条全为
 *         '/workflow/leaveEdit/index' —— Vue 的 views/xxx/index.vue 风格。
 *
 * Svelte: 路由是 /workflow/leaveEdit/[[id]]，页面读 params.id + ?mode= + ?taskId=
 *         (见 routes/(app)/workflow/leaveEdit/[[id]]/+page.svelte)。
 *
 * 于是要做三件事：剥掉 '/index'、把 businessId 挪到路径段、type 改名 mode。
 * 直接照 Vue 抄会落到 /workflow/leaveEdit/index，[[id]] 匹配成 id='index'，
 * 随后 getById('index') 报错 —— 这类错误不会在编译期暴露，故以单测锁定。
 */
export type FormJumpType = 'view' | 'approval' | 'update' | 'add'

export interface FormJumpArgs {
  /** 来自 FlowTaskVo.formPath / FlowHisTaskVo.formPath。 */
  formPath?: string | null
  /** 业务主键(雪花 id，全程字符串)。 */
  businessId?: string | null
  type: FormJumpType
  /** approval 时必带。 */
  taskId?: string | null
  /** 返回路径。leaveEdit 的 back() 默认硬跳 /demo/leave，带上它才能回原页。 */
  from?: string | null
}

/** 返回可直接 goto() 的 URL；参数不足时返回 null（调用方据此禁用入口）。 */
export function buildFormUrl(args: FormJumpArgs): string | null {
  const { formPath, businessId, type, taskId, from } = args
  if (!formPath || !businessId) return null

  // '/workflow/leaveEdit/index' → '/workflow/leaveEdit'；顺带吃掉尾斜杠。
  const base = formPath.replace(/\/index\/?$/, '').replace(/\/$/, '')

  const qs = new URLSearchParams({ mode: type })
  if (taskId) qs.set('taskId', taskId)
  if (from) qs.set('from', from)

  return `${base}/${businessId}?${qs.toString()}`
}
