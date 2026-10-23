<script lang="ts">
  import * as Card from '$lib/components/ui/card/index.js'
  import UserAuthForm from './components/UserAuthForm.svelte'

  let redirect = $state<string | undefined>(undefined)
</script>

<Card.Root class="gap-4">
  <Card.Header>
    <Card.Title class="text-lg tracking-tight">登录</Card.Title>
    <Card.Description>
      请输入您的用户名和密码
    </Card.Description>
  </Card.Header>
  <Card.Content>
    <UserAuthForm redirectTo={redirect} />
  </Card.Content>
  <Card.Footer>
    <a href="/forgot-password" class="ms-auto inline-block text-sm underline"> 忘记密码？ </a>
  </Card.Footer>
</Card.Root>
