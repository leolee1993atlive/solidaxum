<script lang="ts">
  import type { HTMLThAttributes } from 'svelte/elements'
  import { cn, type WithElementRef } from '$lib/utils.js'

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithElementRef<HTMLThAttributes> = $props()
</script>

<th
  bind:this={ref}
  data-slot="table-head"
  class={cn(
    // 表头列间的短竖线是青绿的皮层特征,写在 teal.css 里
    // (.theme-teal [data-slot='table-head']),不在这里给所有皮肤都画上。
    'text-foreground h-10 bg-clip-padding px-2 text-center align-middle font-medium whitespace-nowrap [&:has([role=checkbox])]:pe-0',
    className
  )}
  {...restProps}
>
  {@render children?.()}
</th>
