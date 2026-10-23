// 契约类型 —— 对齐 solidaxum 后端 (crates/core/src/vo/{response,page}.rs)

/** 单值响应 R<T> -> { code, msg, data? }。
 *  msg 是上游字段;message 可选——部分后端壳用 message。 */
export interface ApiResponse<T> {
  code: number
  msg: string
  message?: string
  data?: T
}

/** 分页内层 TableDataInfo<T> (嵌套在 ApiResponse.data 中)
 *
 *  后端的 total 默认走**上限计数**(1 万):不到上限时就是精确值,超出时
 *  total 等于上限并置 totalCapped —— 此时 total 是**下界**,不是真实总数。
 *  两个字段都可选:老后端不返回它们,视为「未触顶」。 */
export interface TableData<T> {
  code: number
  msg: string
  rows: T[]
  total: number
  totalCapped?: boolean
  /** 触顶时的表级估算行数(不精确、不认筛选条件),仅供展示量级。 */
  totalApprox?: number
}

/** client.getList<T>() 解构后返回给视图的扁平结果 */
export interface ListResult<T> {
  rows: T[]
  total: number
  totalCapped?: boolean
  totalApprox?: number
}

/** 分页请求 PageQuery (camelCase) */
export interface PageQuery {
  pageNum: number
  pageSize: number
  orderByColumn?: string
  isAsc?: string
}

/** 把 PageQuery + 额外过滤序列化为 query string record,跳过 undefined/null/空串。 */
export function toPageQuery(
  base: PageQuery,
  extra?: Record<string, string | number | undefined | null>
): Record<string, string> {
  const r: Record<string, string> = {
    pageNum: String(base.pageNum),
    pageSize: String(base.pageSize),
  }
  if (base.orderByColumn) r.orderByColumn = base.orderByColumn
  if (base.isAsc) r.isAsc = base.isAsc
  if (extra) {
    for (const [k, v] of Object.entries(extra)) {
      if (v === undefined || v === null) continue
      const s = String(v)
      if (s.trim() === '') continue
      r[k] = s
    }
  }
  return r
}

// ---- Auth 类型 ----

/** POST /auth/login body (camelCase) */
export interface LoginBody {
  clientId: string
  grantType: 'password'
  code?: string
  uuid?: string
  username?: string
  password?: string
}

/** LoginVo —— 唯一 snake_case 响应 */
export interface LoginVo {
  access_token: string
  expire_in: number
  client_id: string
  refresh_token?: string
  refresh_expire_in?: number
  scope?: string
  openid?: string
}

/** GET /auth/code -> CaptchaVo (camelCase) */
export interface CaptchaVo {
  captchaEnabled: boolean
  uuid?: string
  img?: string
}

/**
 * getInfo 返回的角色对象(对齐 Java SysUserVo.roles / Rust RoleListVo)。
 * 下拉身份区只用到这两个字段,故声明窄类型;完整角色类型见
 * `$lib/api/system/role.ts` 的 RoleListVo。
 * roleId 是字符串:后端 stringify_ids 会把任何以 Id 结尾的键转为字符串。
 */
export interface UserRoleBrief {
  roleId: string
  roleName: string
}

/**
 * GET /system/user/getInfo -> { user, roles, permissions }
 *
 * 字段逐条对齐后端 `user_handler.rs:432-451` 手工拼的 json!,**不多不少**。
 * ⚠️ 勿加 `[k: string]: unknown` —— 索引签名会让任何拼错的字段名通过类型检查。
 *    `auth.user?.username`(正确写法是 userName)就是这么在 ProfileDropdown 里
 *    恒取到 undefined、UI 常年显示兜底串 'User' 的。字段不够照后端补,别开口子。
 * `deptName`/`roles` 由后端翻译写入(dept_id -> dept_name;按 user_id 查角色),
 * 与顶层 `GetInfoVo.roles`(角色 key 字符串数组)不是一回事,不可互相顶替。
 */
export interface UserInfo {
  userId: string
  deptId?: string
  deptName?: string
  userName: string
  nickName?: string
  userType?: string
  email?: string
  phoneNumber?: string
  gender?: string
  /** 头像 OSS ID(原始值) */
  avatar?: string
  /** 头像 URL(后端翻译后) */
  avatarUrl?: string
  status?: string
  loginIp?: string
  loginDate?: string
  remark?: string
  roles?: UserRoleBrief[]
  createTime?: string
  updateTime?: string
}
export interface GetInfoVo {
  user: UserInfo
  roles: string[]
  permissions: string[]
}

/** GET /system/menu/getRouters -> RouterVo 树 */
export interface RouterMeta {
  title: string
  icon?: string
  noCache?: boolean
  link?: string
  activeMenu?: string
}
export interface RouterVo {
  name?: string
  path: string
  hidden?: boolean
  redirect?: string
  component?: string
  query?: string
  alwaysShow?: boolean
  meta: RouterMeta
  children?: RouterVo[]
}
