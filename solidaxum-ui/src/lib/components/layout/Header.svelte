<script lang="ts">
  import type { Snippet } from 'svelte'
  import { Separator } from '$lib/components/ui/separator/index.js'
  import { SidebarTrigger } from '$lib/components/ui/sidebar/index.js'
  import { cn } from '$lib/utils.js'

  interface Props {
    children?: Snippet
    fixed?: boolean
    /** 顶栏标题(系统名称),显示在折叠钮右侧;空则不渲染。 */
    title?: string
    class?: string
    [key: string]: unknown
  }

  let {
    children,
    fixed = false,
    title = '',
    class: className = '',
    ...props
  }: Props = $props()

  let offset = $state(0)

  $effect(() => {
    const onScroll = () => {
      offset = document.body.scrollTop || document.documentElement.scrollTop
    }

    document.addEventListener('scroll', onScroll, { passive: true })

    return () => document.removeEventListener('scroll', onScroll)
  })
</script>

<header
  class={cn(
    'bg-header z-50 h-16 rounded-tl-[24px]',
    fixed && 'header-fixed peer/header sticky top-0 w-[inherit]',
    offset > 10 && fixed ? 'shadow' : 'shadow-none',
    className
  )}
  {...props}
>
  <div
    class={cn(
      'relative flex h-full items-center gap-3 p-4 sm:gap-4',
      offset > 10 &&
        fixed &&
        'after:bg-background/20 after:absolute after:inset-0 after:-z-10 after:backdrop-blur-lg'
    )}
  >
    <SidebarTrigger variant="outline" class="max-md:scale-125" />
    <Separator orientation="vertical" class="h-6" />
    {#if title}
      <h1
        class="text-header-foreground min-w-0 truncate text-base font-semibold tracking-tight"
      >
        {title}
      </h1>
    {/if}
    {#if children}
      <div class="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-4">
        {@render children()}
      </div>
    {/if}
  </div>
</header>
