<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '$lib/utils.js'

  interface Props {
    children?: Snippet
    fixed?: boolean
    fluid?: boolean
    class?: string
    [key: string]: unknown
  }

  let { fixed, class: className, fluid, children, ...props }: Props = $props()
</script>

<main
  data-layout={fixed ? 'fixed' : 'auto'}
  class={cn(
    'px-4 py-6',

    // If layout is fixed, make the main container flex and grow
    fixed && 'flex grow flex-col overflow-hidden',

    // If layout is not fluid, set the max-width
    !fluid && '@7xl/content:mx-auto @7xl/content:w-full @7xl/content:max-w-7xl',
    className
  )}
  {...props}
>
  {@render children?.()}
</main>
