<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { cn, type WithElementRef } from '$lib/utils.js'
  import { Eye, EyeOff } from '@lucide/svelte'

  type Props = WithElementRef<Omit<HTMLInputAttributes, 'type'>>

  let {
    ref = $bindable(null),
    value = $bindable(),
    class: className,
    disabled,
    ...restProps
  }: Props = $props()

  let showPassword = $state(false)
</script>

<div class={cn('relative rounded-md', className)}>
  <Input
    bind:ref
    bind:value
    {disabled}
    type={showPassword ? 'text' : 'password'}
    class="pr-10"
  />
  <Button
    type="button"
    size="icon"
    variant="ghost"
    {disabled}
    class="text-muted-foreground absolute end-1 top-1/2 h-6 w-6 -translate-y-1/2 rounded-md"
    onclick={() => (showPassword = !showPassword)}
  >
    {#if showPassword}
      <Eye size={18} />
    {:else}
      <EyeOff size={18} />
    {/if}
    <span class="sr-only">{showPassword ? '隐藏密码' : '显示密码'}</span>
  </Button>
</div>
