<script lang="ts">
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { TOKEN_KEY } from '$lib/api/client.js'
  import { setupApi, markSetupCompleted } from '$lib/api/system/setup.js'
  import { APP_NAME } from '$lib/brand.js'
  import PasswordInput from '$lib/components/PasswordInput.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import {
    Building2,
    CheckCircle2,
    KeyRound,
    LoaderCircle,
    ShieldCheck,
    Users,
  } from '@lucide/svelte'
  import { toast } from 'svelte-sonner'

  /**
   * 首装系统初始化向导(独立于 (app)/(auth) 布局)。
   *
   * 进入条件:已登录且 sys_config.`sys.setup.completed` != true;
   * 后端键不存在 = 已完成(老系统),本页会直接弹回首页。
   * 完成动作为一个聚合端点:改管理员密码 + 写系统名称(sys.title)
   * + 写用户默认密码(sys.user.initPassword)。
   */
  let ready = $state(false)
  /** 0=欢迎 1=系统信息 2=管理员密码 3=用户默认密码 4=成功 */
  let step = $state(0)

  let systemName = $state('')
  let adminOld = $state('')
  let adminNew = $state('')
  let adminConfirm = $state('')
  let userInit = $state('')
  let userInitConfirm = $state('')
  let submitting = $state(false)

  const CONFIG_STEPS = [
    { title: '系统信息', icon: Building2 },
    { title: '管理员密码', icon: KeyRound },
    { title: '用户默认密码', icon: Users },
  ] as const

  const stepError = $derived.by(() => {
    if (step === 1) {
      if (!systemName.trim()) return '请填写系统名称'
      if (systemName.trim().length > 90) return '系统名称不能超过 90 字'
    }
    if (step === 2) {
      if (adminOld.length < 6 || adminOld.length > 50)
        return '当前密码为 6-50 位'
      if (adminNew.length < 6 || adminNew.length > 50) return '新密码为 6-50 位'
      if (adminNew !== adminConfirm) return '两次输入的新密码不一致'
      if (adminNew === adminOld) return '新密码不能与当前密码相同'
    }
    if (step === 3) {
      if (userInit.length < 6 || userInit.length > 50)
        return '默认密码为 6-50 位'
      if (userInit !== userInitConfirm) return '两次输入的默认密码不一致'
    }
    return ''
  })

  onMount(async () => {
    const hasToken =
      typeof localStorage !== 'undefined' && !!localStorage.getItem(TOKEN_KEY)
    if (!hasToken) {
      goto('/')
      return
    }
    try {
      const resp = await setupApi.status()
      // 已初始化(或探测失败)都不该停留在此页
      if (resp.data?.completed !== false) {
        goto('/')
        return
      }
    } catch {
      goto('/')
      return
    }
    ready = true
  })

  function next() {
    if (stepError) {
      toast.warning(stepError)
      return
    }
    step += 1
  }

  async function complete() {
    if (stepError || submitting) {
      if (stepError) toast.warning(stepError)
      return
    }
    submitting = true
    try {
      await setupApi.complete({
        systemName: systemName.trim(),
        adminOldPassword: adminOld,
        adminNewPassword: adminNew,
        userInitPassword: userInit,
      })
      markSetupCompleted()
      step = 4
    } catch {
      // 失败原因由 client 统一 toast(旧密码错误/重复初始化等),
      // 停在第 3 步让用户修正后重试
    } finally {
      submitting = false
    }
  }

  function enterSystem() {
    goto('/index')
  }
</script>

<div class="bg-background flex min-h-screen items-center justify-center p-4">
  <div class="w-full max-w-xl space-y-4">
    <div class="text-center">
      <div class="flex items-center justify-center gap-2">
        <ShieldCheck class="text-muted-foreground size-6" />
        <h1 class="text-xl font-semibold tracking-tight">
          {APP_NAME} · 系统初始化
        </h1>
      </div>
      <p class="text-muted-foreground mt-1 text-sm">
        首次部署完成后,请完成以下基础配置
      </p>
    </div>

    {#if !ready}
      <Card.Root>
        <Card.Content
          class="text-muted-foreground flex items-center justify-center gap-2 py-10"
        >
          <LoaderCircle class="size-5 animate-spin" /> 检查初始化状态…
        </Card.Content>
      </Card.Root>
    {:else if step === 0}
      <Card.Root>
        <Card.Header>
          <Card.Title>欢迎</Card.Title>
          <Card.Description>
            向导将引导你完成三项基础配置,全部保存在本系统数据库中:
          </Card.Description>
        </Card.Header>
        <Card.Content class="space-y-3">
          {#each CONFIG_STEPS as s, i}
            <div class="flex items-center gap-3 rounded-lg border p-3">
              <!-- runes 模式下组件本身即动态,无需 svelte:component -->
              <s.icon class="text-muted-foreground size-5" />
              <div>
                <p class="text-sm font-medium">{i + 1}. {s.title}</p>
              </div>
            </div>
          {/each}
          <p class="text-muted-foreground text-xs">
            提示:管理员初始账号 admin,初始密码为种子默认值(admin123),
            下一步将要求输入当前密码以完成更换。
          </p>
        </Card.Content>
        <Card.Footer class="justify-end">
          <Button onclick={() => (step = 1)}>开始配置</Button>
        </Card.Footer>
      </Card.Root>
    {:else if step < 4}
      <Card.Root>
        <Card.Header>
          <!-- 步骤指示器 -->
          <div
            class="flex items-center gap-2"
            aria-label="步骤 {step}/{CONFIG_STEPS.length}"
          >
            {#each CONFIG_STEPS as s, i}
              {#if i > 0}<div class="bg-border h-px flex-1"></div>{/if}
              <div class="flex items-center gap-1.5">
                <div
                  class="flex size-6 items-center justify-center rounded-full text-xs font-medium
                    {i + 1 === step
                    ? 'bg-primary text-primary-foreground'
                    : i + 1 < step
                      ? 'bg-primary/15 text-foreground'
                      : 'bg-muted text-muted-foreground'}"
                >
                  {#if i + 1 < step}<CheckCircle2 class="size-4" />{:else}{i +
                      1}{/if}
                </div>
                <span
                  class="text-xs {i + 1 === step
                    ? 'text-foreground font-medium'
                    : 'text-muted-foreground'}"
                >
                  {s.title}
                </span>
              </div>
            {/each}
          </div>
        </Card.Header>

        <Card.Content class="space-y-4">
          {#if step === 1}
            <Field.Field>
              <Field.Label for="systemName">系统名称</Field.Label>
              <Input
                id="systemName"
                bind:value={systemName}
                placeholder="例如:SolidAxum 管理平台"
                maxlength={90}
              />
              <Field.Description>
                将作为系统顶栏标题(保存于配置项
                sys.title),全站可见,后续可在「系统信息」页修改。
              </Field.Description>
            </Field.Field>
          {:else if step === 2}
            <Field.Field>
              <Field.Label for="adminOld">管理员当前密码</Field.Label>
              <PasswordInput
                id="adminOld"
                bind:value={adminOld}
                placeholder="初始密码为 admin123"
                autocomplete="current-password"
              />
            </Field.Field>
            <Field.Field>
              <Field.Label for="adminNew">新密码</Field.Label>
              <PasswordInput
                id="adminNew"
                bind:value={adminNew}
                placeholder="6-50 位,不能与当前密码相同"
                autocomplete="new-password"
              />
            </Field.Field>
            <Field.Field>
              <Field.Label for="adminConfirm">确认新密码</Field.Label>
              <PasswordInput
                id="adminConfirm"
                bind:value={adminConfirm}
                placeholder="再输入一次新密码"
                autocomplete="new-password"
              />
            </Field.Field>
          {:else if step === 3}
            <Field.Field>
              <Field.Label for="userInit">用户默认密码</Field.Label>
              <PasswordInput
                id="userInit"
                bind:value={userInit}
                placeholder="6-50 位"
              />
              <Field.Description>
                用于批量导入新账号的初始密码(配置项 sys.user.initPassword),
                用户首次登录后应自行修改。
              </Field.Description>
            </Field.Field>
            <Field.Field>
              <Field.Label for="userInitConfirm">确认默认密码</Field.Label>
              <PasswordInput
                id="userInitConfirm"
                bind:value={userInitConfirm}
                placeholder="再输入一次默认密码"
              />
            </Field.Field>
          {/if}
        </Card.Content>

        <Card.Footer class="justify-between">
          <Button variant="outline" onclick={() => (step -= 1)}>上一步</Button>
          {#if step < 3}
            <Button onclick={next}>下一步</Button>
          {:else}
            <Button onclick={complete} disabled={submitting}>
              {#if submitting}
                <LoaderCircle class="mr-2 size-4 animate-spin" />
              {/if}
              完成初始化
            </Button>
          {/if}
        </Card.Footer>
      </Card.Root>
    {:else}
      <Card.Root>
        <Card.Content class="space-y-4 py-8 text-center">
          <CheckCircle2 class="text-success-emphasis mx-auto size-12" />
          <div>
            <p class="font-medium">初始化完成</p>
            <p class="text-muted-foreground mt-1 text-sm">
              {systemName} · 管理员密码与用户默认密码已生效
            </p>
          </div>
        </Card.Content>
        <Card.Footer class="justify-center">
          <Button onclick={enterSystem}>进入系统</Button>
        </Card.Footer>
      </Card.Root>
    {/if}
  </div>
</div>
