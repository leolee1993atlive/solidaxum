<script lang="ts">
  import { goto } from '$app/navigation'
  import Button from '$lib/components/ui/button/button.svelte'
</script>

<div class="h-svh">
  <div
    class="m-auto flex h-full w-full flex-col items-center justify-center gap-2"
  >
    <h1 class="text-[7rem] leading-tight font-bold">500</h1>
    <span class="font-medium">糟糕！出错了 {`:')`}</span>
    <p class="text-muted-foreground text-center">
      给您带来不便，我们深表歉意。 <br />
      请稍后再试。
    </p>
    <div class="mt-6 flex gap-4">
      <Button variant="outline" onclick={() => window.history.back()}>
        返回
      </Button>
      <Button onclick={() => goto('/')}>回到首页</Button>
    </div>
  </div>
</div>
