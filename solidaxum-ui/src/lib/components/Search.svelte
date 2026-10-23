<script lang="ts">
  import { Button } from '$lib/components/ui/button/index.js'
  import { useSearch } from '$lib/context/search.svelte.js'
  import { cn } from '$lib/utils.js'
  import { SearchIcon } from '@lucide/svelte'

  interface Props {
    class?: string
    type?: string
    placeholder?: string
  }

  let {
    class: className = '',
    type = 'button',
    placeholder = '搜索...',
  }: Props = $props()
  const search = useSearch()
</script>

<Button
  variant="outline"
  class={cn(
    'group bg-muted/25 text-muted-foreground hover:bg-accent relative h-8 w-full flex-1 justify-start rounded-md text-sm font-normal shadow-none sm:w-40 sm:pe-12 md:flex-none lg:w-52 xl:w-64',
    className
  )}
  onclick={() => search.setOpen(true)}
>
  <SearchIcon
    aria-hidden="true"
    className="absolute start-1.5 top-1/2 -translate-y-1/2"
    size={16}
  />
  <span class="ms-4">{placeholder}</span>
  <kbd
    class="bg-muted group-hover:bg-accent pointer-events-none absolute end-[0.3rem] top-[0.3rem] hidden h-5 items-center gap-1 rounded border px-1.5 font-mono text-[10px] font-medium opacity-100 select-none sm:flex"
  >
    <span class="text-xs">⌘</span>K
  </kbd>
</Button>
