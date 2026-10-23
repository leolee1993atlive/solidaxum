<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { TOKEN_KEY } from '$lib/api/client.js'
  import Logo from '$lib/components/Logo.svelte'
  import { APP_NAME } from '$lib/brand.js'
  import { useAuth } from '$lib/context/auth.svelte.js'

  let { children } = $props()

  const auth = useAuth()

  onMount(async () => {
    // 已持有 token(如刷新页面)则校验并离开登录页,进入首页 /index。
    // token 失效时 ensureLoaded 返回 false,留在登录页。
    if (typeof localStorage !== 'undefined' && localStorage.getItem(TOKEN_KEY)) {
      const ok = await auth.ensureLoaded()
      if (ok) goto('/index')
    }
  })
</script>

  <svelte:head>
  <title>登录 - {APP_NAME}</title>
  </svelte:head>

  <div class="container grid h-svh max-w-none items-center justify-center">
  <div
    class="mx-auto flex w-full flex-col justify-center space-y-2 py-8 sm:w-[480px] sm:p-8"
  >
    <div class="mb-4 flex items-center justify-center">
      <Logo class="me-2" />
      <h1 class="text-xl font-medium">{APP_NAME}</h1>
    </div>
    {@render children()}
  </div>
</div>
