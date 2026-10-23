<script lang="ts">
  import { goto } from '$app/navigation'
  import Button from '$lib/components/ui/button/button.svelte'
</script>

<div class="h-svh">
  <div
    class="m-auto flex h-full w-full flex-col items-center justify-center gap-2"
  >
    <h1 class="text-[7rem] leading-tight font-bold">404</h1>
    <span class="font-medium">糟糕！页面未找到！</span>
    <p class="text-muted-foreground text-center">
      您要查找的页面 <br />
      似乎不存在或已被删除。
    </p>
    <div class="mt-6 flex gap-4">
      <Button variant="outline" onclick={() => window.history.back()}>
        返回
      </Button>
      <Button onclick={() => goto('/')}>回到首页</Button>
    </div>
  </div>
</div>
