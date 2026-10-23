<script lang="ts">
  import { onMount } from 'svelte'
  import { fly } from 'svelte/transition'
  import { goto, beforeNavigate, afterNavigate } from '$app/navigation'
  import { page } from '$app/state'
  import { tweened } from 'svelte/motion'
  import { cubicOut } from 'svelte/easing'
  import { LoaderCircle } from '@lucide/svelte'
  import { TOKEN_KEY } from '$lib/api/client.js'
  import ConnectionAlert from '$lib/components/ConnectionAlert.svelte'
  import AppSidebar from '$lib/components/layout/AppSidebar.svelte'
  import Header from '$lib/components/layout/Header.svelte'
  import TabBar from '$lib/components/layout/TabBar.svelte'
  import { APP_NAME } from '$lib/components/layout/data/sidebar-data'
  import {
    SidebarProvider,
    SidebarInset,
  } from '$lib/components/ui/sidebar/index.js'
  import NotificationBell from '$lib/components/NotificationBell.svelte'
  import ProfileDropdown from '$lib/components/ProfileDropdown.svelte'
  import Search from '$lib/components/Search.svelte'
  import SearchProvider from '$lib/context/SearchProvider.svelte'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { setLayoutContext } from '$lib/context/layout.svelte.js'
  import { pathToTitle } from '$lib/menu/transform.js'
  import { isCacheable } from '$lib/stores/tab-cache.js'
  import { TabsStore } from '$lib/stores/tabs.svelte.js'

  let { children } = $props()

  const auth = useAuth()
  // 顶栏标题:系统信息页配置优先,未配置回退构建期品牌名
  const headerTitle = $derived(auth.sysTitle || APP_NAME)
  setLayoutContext()
  const tabs = new TabsStore()
  let refreshToken = $state(0)

  // 导航变化 → 开/激活 tab
  $effect(() => {
    const path = page.url.pathname
    const title = pathToTitle(auth.navTree, path) ?? path
    tabs.open(path, title, isCacheable(path))
  })

  // 滚动位置 best-effort 恢复(仅 cacheable 页)
  beforeNavigate(({ from }) => {
    if (from && isCacheable(from.url.pathname)) {
      sessionStorage.setItem(
        `scroll:${from.url.pathname}`,
        String(window.scrollY)
      )
    }
  })
  afterNavigate(({ to }) => {
    if (to && isCacheable(to.url.pathname)) {
      const y = Number(sessionStorage.getItem(`scroll:${to.url.pathname}`) ?? 0)
      requestAnimationFrame(() => window.scrollTo(0, y))
    }
  })

  function refresh(path: string) {
    if (path !== page.url.pathname) {
      goto(path).then(() => (refreshToken += 1))
    } else {
      refreshToken += 1
    }
  }

  // 守卫状态用**本组件本地** $state 驱动,在 onMount 里 await 会话恢复后落定。
  // 不依赖跨组件读取 auth context 的 $state/$derived(那条链在会话恢复的异步态
  // 变更时不会重渲染,会永远卡在"加载中")。
  let status = $state<'checking' | 'authed' | 'guest'>('checking')


  // ---- 路由切换进度条(tweened 驱动,beforeNavigate 爬升/afterNavigate 到顶淡出)
  const navProgress = tweened(0, { duration: 250, easing: cubicOut })
  let navInProgress = $state(false)
  beforeNavigate(() => {
    navInProgress = true
    void navProgress.set(0.15)
    // 爬升:分两步到 60%,不一下到顶(到顶意味着"完成了")
    setTimeout(() => void navProgress.set(0.5), 150)
  })
  afterNavigate(() => {
    void navProgress.set(1).then(() => {
      navInProgress = false
      void navProgress.set(0, { duration: 0 })
    })
  })

  onMount(async () => {
    const hasToken =
      typeof localStorage !== 'undefined' && !!localStorage.getItem(TOKEN_KEY)
    if (!hasToken) {
      status = 'guest'
      goto('/') // 登录页真实 URL 是 /((auth) 路由组不进 URL)
      return
    }
    const ok = await auth.ensureLoaded()
    status = ok ? 'authed' : 'guest'
    if (!ok) goto('/')
  })

  const pageTitle = $derived(pathToTitle(auth.navTree, page.url.pathname))
  const docTitle = $derived(pageTitle ? `${pageTitle} - ${APP_NAME}` : APP_NAME)

  // 列表页视口适配(pageFit):这些路由把外层高度锁定为视口、内容区
  // 禁止整页滚动,配合 CrudTableShell 的 fit 表格内部滚动(对齐青绿列表页)。
  // 只按路径启用——其余长页面保持原有整页滚动。
  const FIT_HEIGHT_PATHS = new Set(['/demo/leave'])
  const pageFit = $derived(FIT_HEIGHT_PATHS.has(page.url.pathname))
</script>

<svelte:head>
  <title>{docTitle}</title>
</svelte:head>

<!-- 路由进度条(2px 顶部) -->
{#if navInProgress}
  <div
    class="bg-primary fixed top-0 left-0 z-[100] h-0.5 transition-[opacity]"
    style="width: {Math.round($navProgress * 100)}%"
    aria-hidden="true"
  ></div>
{/if}

{#if status === 'checking'}
  <div class="flex h-screen items-center justify-center gap-2">
    <LoaderCircle class="text-muted-foreground size-5 animate-spin" />
    <span class="text-muted-foreground text-sm">加载中...</span>
  </div>
{:else if status === 'authed'}
  <SearchProvider>
    <ConnectionAlert />
    <div
      class="bg-background {pageFit
        ? 'flex h-svh flex-col overflow-hidden'
        : 'min-h-svh'}"
      style="background-image: var(--gradient-shell)"
    >
      <SidebarProvider
        style="--sidebar-width: 184px"
        class={pageFit ? 'h-svh overflow-hidden' : ''}
      >
        <AppSidebar />
        <SidebarInset
          class={pageFit
            ? 'min-h-0 min-w-0 bg-transparent'
            : 'min-w-0 bg-transparent'}
        >
          <Header title={headerTitle}>
            <Search />
            <NotificationBell />
            <ProfileDropdown />
          </Header>
          <TabBar {tabs} onRefresh={refresh} />
          <div
            data-testid="app-content"
            class="bg-background flex-1 {pageFit
              ? 'flex min-h-0 flex-col overflow-hidden'
              : ''}"
            style="background-image: var(--gradient-content)"
          >
            {#key `${page.url.pathname}::${refreshToken}`}
              <!-- 路由切换淡入(只进不出;reduced-motion 由 layout.css 全局兜底) -->
              <div in:fly={{ y: 8, duration: 200 }}>
                {@render children()}
              </div>
            {/key}
          </div>
        </SidebarInset>
      </SidebarProvider>
    </div>
  </SearchProvider>
{/if}
