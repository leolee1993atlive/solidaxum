<script lang="ts">
  import { goto } from '$app/navigation'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { LoaderCircle, Mail } from '@lucide/svelte'

  let isLoading = $state(false)

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault()
    isLoading = true
    setTimeout(async () => {
      isLoading = false
      await goto('/?notice=reset-not-implemented')
    }, 300)
  }
</script>

<Card.Root class="gap-4">
  <Card.Header>
    <Card.Title class="text-lg tracking-tight">忘记密码</Card.Title>
    <Card.Description>
      请输入您的邮箱地址，我们将发送重置密码链接
    </Card.Description>
  </Card.Header>
  <Card.Content>
    <form class="grid gap-3" onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label>邮箱</Field.Label>
          <Input placeholder="name@example.com" type="email" />
        </Field.Field>
      </Field.Group>
      <Button class="mt-2" disabled={isLoading}>
        {#if isLoading}
          <LoaderCircle class="animate-spin" />
        {:else}
          <Mail />
        {/if}
        发送重置链接（即将推出）
      </Button>

      <p class="text-muted-foreground text-xs">密码重置功能尚未连接后端。</p>
    </form>
  </Card.Content>
  <Card.Footer>
    <p class="text-muted-foreground px-8 text-center text-sm">
      记得密码？{' '}
      <a href="/" class="hover:text-primary-emphasis underline underline-offset-4">
        登录
      </a>
    </p>
  </Card.Footer>
</Card.Root>
