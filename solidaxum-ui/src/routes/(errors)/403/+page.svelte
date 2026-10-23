<script lang="ts">
  import { goto } from '$app/navigation'
  import Button from '$lib/components/ui/button/button.svelte'
</script>

<div class="h-svh">
  <div
    class="m-auto flex h-full w-full flex-col items-center justify-center gap-2"
  >
    <h1 class="text-[7rem] leading-tight font-bold">403</h1>
    <span class="font-medium">禁止访问</span>
    <p class="text-muted-foreground text-center">
      您没有必要的权限 <br />
      查看此资源。
    </p>
    <div class="mt-6 flex gap-4">
      <Button variant="outline" onclick={() => window.history.back()}>
        返回
      </Button>
      <Button onclick={() => goto('/')}>回到首页</Button>
    </div>
  </div>
</div>
