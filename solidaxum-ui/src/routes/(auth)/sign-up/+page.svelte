<script lang="ts">
  import { goto } from '$app/navigation'
  import PasswordInput from '$lib/components/PasswordInput.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { LoaderCircle, UserPlus } from '@lucide/svelte'

  let isLoading = $state(false)

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault()
    isLoading = true
    setTimeout(async () => {
      isLoading = false
      await goto('/?notice=signup-not-implemented')
    }, 300)
  }
</script>

<Card.Root class="gap-4">
  <Card.Header>
    <Card.Title class="text-lg tracking-tight">注册</Card.Title>
    <Card.Description>请输入以下信息创建您的账户</Card.Description>
  </Card.Header>
  <Card.Content>
    <form class="grid gap-3" onsubmit={handleSubmit}>
      <div class="grid grid-cols-2 gap-3">
        <Field.Group>
          <Field.Field>
            <Field.Label>名</Field.Label>
            <Input placeholder="张三" />
          </Field.Field>
        </Field.Group>
        <Field.Group>
          <Field.Field>
            <Field.Label>姓</Field.Label>
            <Input placeholder="李四" />
          </Field.Field>
        </Field.Group>
      </div>
      <Field.Group>
        <Field.Field>
          <Field.Label>邮箱</Field.Label>
          <Input placeholder="name@example.com" type="email" />
        </Field.Field>
      </Field.Group>
      <Field.Group>
        <Field.Field>
          <Field.Label>密码</Field.Label>
          <PasswordInput placeholder="********" />
        </Field.Field>
      </Field.Group>
      <Field.Group>
        <Field.Field>
          <Field.Label>确认密码</Field.Label>
          <PasswordInput placeholder="********" />
        </Field.Field>
      </Field.Group>
      <Button class="mt-2" disabled={isLoading}>
        {#if isLoading}
          <LoaderCircle class="animate-spin" />
        {:else}
          <UserPlus />
        {/if}
        注册（即将推出）
      </Button>

      <p class="text-muted-foreground text-xs">
        注册功能尚未连接后端。请使用已有账户登录。
      </p>
    </form>
  </Card.Content>
  <Card.Footer>
    <p class="text-muted-foreground px-8 text-center text-sm">
      已有账户？{' '}
      <a href="/" class="hover:text-primary-emphasis underline underline-offset-4">
        登录
      </a>
    </p>
  </Card.Footer>
</Card.Root>
