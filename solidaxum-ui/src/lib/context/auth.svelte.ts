import { getContext, setContext } from 'svelte'
import { authApi } from '$lib/api/auth.js'
import { apiClient, TOKEN_KEY, CLIENT_ID_KEY } from '$lib/api/client.js'
import { menuApi } from '$lib/api/system/menu.js'
import { configApi } from '$lib/api/system/config.js'
import type { GetInfoVo, UserInfo, LoginBody } from '$lib/api/types.js'
import type { NavNode } from '$lib/components/layout/data/types.js'
import { routersToCacheMap } from '$lib/menu/cache-map.js'
import { routersToNavTree, routersToHiddenMap } from '$lib/menu/transform.js'
import { setCacheMap } from '$lib/stores/tab-cache.js'
import { setHiddenRouteMap } from '$lib/stores/hidden-routes.js'
import { toast } from 'svelte-sonner'
import {
  hasPermission as hasPermissionImpl,
  hasRole as hasRoleImpl,
} from './auth-helpers.js'

export { hasPermissionImpl as hasPermission, hasRoleImpl as hasRole }

export interface AuthUser extends UserInfo {}

class AuthState {
  user = $state<AuthUser | null>(null)
  roles = $state<string[]>([])
  permissions = $state<string[]>([])
  /** 后端动态菜单转换后的 NavNode[],供 sidebar/命令面板消费。 */
  navTree = $state<NavNode[]>([])
  /** 系统信息(系统信息页可配,存 sys_config)。title 空时消费者兜底 APP_NAME。 */
  sysTitle = $state('')
  sysIcon = $state('')
  isLoading = $state(false)
  error = $state<string | null>(null)

  isAuthenticated = $derived(this.user !== null)
  isAdmin = $derived(hasRoleImpl(this.roles, 'admin'))

  // 会话恢复去重:同一次加载并发调用共享同一个 Promise,避免重复 getInfo。
  private loadingPromise: Promise<boolean> | null = null

  /**
   * 确保用户信息已加载。有 token 但尚未加载时拉取 getInfo。
   * 由布局在 onMount 中 await 调用——**不能**在构造函数里发起(构造发生在
   * setContext 的组件初始化阶段,此时异步态变更不会被 Svelte 调度到跨组件消费者,
   * 会导致守卫永远停留在"加载中")。返回是否已认证。
   */
  ensureLoaded = async (): Promise<boolean> => {
    if (this.user) return true
    if (typeof localStorage === 'undefined' || !localStorage.getItem(TOKEN_KEY))
      return false
    if (!this.loadingPromise) {
      this.loadingPromise = (async () => {
        try {
          await this.loadUserInfo()
          return this.user !== null
        } catch {
          this.clearLocal()
          return false
        } finally {
          this.loadingPromise = null
        }
      })()
    }
    return this.loadingPromise
  }

  private async loadUserInfo(): Promise<void> {
    const resp = await apiClient.get<GetInfoVo>('/system/user/getInfo')
    if (resp.data) {
      this.user = resp.data.user
      this.roles = resp.data.roles ?? []
      this.permissions = resp.data.permissions ?? []
    }
    // 拉动态菜单(getRouters),失败不阻断登录(降级为空菜单)。
    try {
      const r = await menuApi.getRouters()
      const routers = r.data ?? []
      this.navTree = routersToNavTree(routers)
      setCacheMap(routersToCacheMap(routers))
      setHiddenRouteMap(routersToHiddenMap(routers))
    } catch {
      this.navTree = []
      setCacheMap({})
      setHiddenRouteMap({})
    }
    // 拉系统信息(标题/图标),失败安静降级为默认品牌(brand.ts APP_NAME)。
    try {
      const si = await configApi.sysInfo()
      this.sysTitle = si.data?.title ?? ''
      this.sysIcon = si.data?.icon ?? ''
    } catch {
      this.sysTitle = ''
      this.sysIcon = ''
    }
  }

  private clearLocal() {
    this.user = null
    this.roles = []
    this.permissions = []
    this.navTree = []
    this.sysTitle = ''
    this.sysIcon = ''
    setCacheMap({})
    setHiddenRouteMap({})
    if (typeof localStorage !== 'undefined') {
      localStorage.removeItem(TOKEN_KEY)
      localStorage.removeItem(CLIENT_ID_KEY)
    }
    if (typeof sessionStorage !== 'undefined') {
      sessionStorage.removeItem('axum_tabs')
      for (const key of Object.keys(sessionStorage)) {
        if (key.startsWith('crud:') || key.startsWith('scroll:')) {
          sessionStorage.removeItem(key)
        }
      }
    }
  }

  login = async (body: LoginBody): Promise<boolean> => {
    this.isLoading = true
    this.error = null
    try {
      const resp = await authApi.login(body)
      const data = resp.data
      if (!data?.access_token) return false
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem(TOKEN_KEY, data.access_token)
        localStorage.setItem(CLIENT_ID_KEY, data.client_id)
      }
      await this.loadUserInfo()
      return true
    } catch (err) {
      this.error = err instanceof Error ? err.message : '登录失败'
      return false
    } finally {
      this.isLoading = false
    }
  }

  logout = async (): Promise<void> => {
    try {
      await authApi.logout()
    } catch {
      // 忽略:即使后端 401 也要本地登出
    }
    this.clearLocal()
    toast.success('登出成功')
  }

  hasPermission = (perm: string): boolean =>
    hasPermissionImpl(this.permissions, perm)
  hasRole = (role: string): boolean => hasRoleImpl(this.roles, role)
}

const AUTH_CONTEXT_KEY = Symbol.for('scn-auth')

export function setAuthContext() {
  return setContext(AUTH_CONTEXT_KEY, new AuthState())
}

export function useAuth() {
  const ctx = getContext<AuthState>(AUTH_CONTEXT_KEY)
  if (!ctx) throw new Error('useAuth must be used within an AuthProvider')
  return ctx
}
