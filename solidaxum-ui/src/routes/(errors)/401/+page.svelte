<script lang="ts">
  import { goto } from '$app/navigation'
  import { Button } from '$lib/components/ui/button/index.js'
</script>

<div class="h-svh">
  <div
    class="m-auto flex h-full w-full flex-col items-center justify-center gap-2"
  >
    <h1 class="text-[7rem] leading-tight font-bold">401</h1>
    <span class="font-medium">未授权访问</span>
    <p class="text-muted-foreground text-center">
      请使用正确的凭据登录 <br /> 以访问此资源。
    </p>
    <div class="mt-6 flex gap-4">
      <Button variant="outline" onclick={() => window.history.back()}>
        返回
      </Button>
      <Button onclick={() => goto('/')}>回到首页</Button>
    </div>
  </div>
</div>
