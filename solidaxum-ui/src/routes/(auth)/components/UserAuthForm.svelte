<script lang="ts">
  import { goto } from '$app/navigation'
  import PasswordInput from '$lib/components/PasswordInput.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { authApi } from '$lib/api/auth.js'
  import { toast } from 'svelte-sonner'
  import { LoaderCircle, LogIn } from '@lucide/svelte'

  // 登录成功后跳首页 /index(与 RuoYi 一致)。/ 本身是登录页,不可作跳转目标。
  let { class: className = '', redirectTo = '/index' } = $props()

  const auth = useAuth()

  let username = $state('')
  let password = $state('')
  let code = $state('')
  let captchaUuid = $state('')
  let captchaImg = $state('')
  let captchaEnabled = $state(false)
  let clientId = $state(import.meta.env.VITE_CLIENT_ID || '')

  async function loadCaptcha() {
    try {
      const resp = await authApi.getCaptcha()
      if (resp.data) {
        captchaEnabled = resp.data.captchaEnabled
        captchaUuid = resp.data.uuid ?? ''
        captchaImg = resp.data.img ?? ''
      }
    } catch {
      // 验证码加载失败不阻塞,允许尝试登录
    }
  }

  $effect(() => {
    loadCaptcha()
  })

  async function handleSubmit(e: Event) {
    e.preventDefault()
    if (!username || !password) return
    const success = await auth.login({
      clientId,
      grantType: 'password',
      username,
      password,
      code: captchaEnabled ? code : undefined,
      uuid: captchaEnabled ? captchaUuid : undefined,
    })
    if (success) {
      toast.success('登录成功!')
      goto(redirectTo)
    }
  }
</script>

<form class="space-y-4 {className}" onsubmit={handleSubmit}>
  <Field.Field>
    <Field.Label for="username">用户名</Field.Label>
    <Input id="username" bind:value={username} placeholder="用户名" autocomplete="username" disabled={auth.isLoading} />
  </Field.Field>

  <Field.Field>
    <Field.Label for="password">密码</Field.Label>
    <PasswordInput id="password" bind:value={password} placeholder="密码" autocomplete="current-password" disabled={auth.isLoading} />
  </Field.Field>

  {#if captchaEnabled && captchaImg}
    <Field.Field>
      <Field.Label for="code">验证码</Field.Label>
      <div class="flex items-center gap-2">
        <Input id="code" bind:value={code} placeholder="验证码" disabled={auth.isLoading} class="flex-1" />
        <!-- img 是 base64 PNG,无 data-uri 前缀;点击刷新验证码,用 button 包裹以满足 a11y -->
        <button type="button" onclick={loadCaptcha} title="点击刷新验证码" class="shrink-0">
          <img src="data:image/png;base64,{captchaImg}" alt="点击刷新验证码" class="h-9 cursor-pointer rounded border" />
        </button>
      </div>
    </Field.Field>
  {/if}

  {#if auth.error}
    <p class="text-destructive text-sm">{auth.error}</p>
  {/if}

  <Button type="submit" class="w-full" disabled={auth.isLoading}>
    {#if auth.isLoading}
      <LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
    {:else}
      <LogIn class="mr-2 h-4 w-4" />
    {/if}
    登录
  </Button>
</form>
