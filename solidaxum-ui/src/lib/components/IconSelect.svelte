<script lang="ts">
  import * as Popover from '$lib/components/ui/popover/index.js'
  import * as ScrollArea from '$lib/components/ui/scroll-area/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { cn } from '$lib/utils.js'
  import { ChevronDown, X } from '@lucide/svelte'
  import { filterIconNames, resolveMenuIcon } from '$lib/menu/icon-map.js'

  let {
    value = '',
    disabled = false,
    onChange,
  }: {
    value?: string
    disabled?: boolean
    onChange?: (next: string) => void
  } = $props()

  let open = $state(false)
  let keyword = $state('')

  const visible = $derived(filterIconNames(keyword))
  // 当前值的预览组件。空值时 resolveMenuIcon 回落 LayoutGrid,故单独判空。
  const CurrentIcon = $derived(value ? resolveMenuIcon(value) : null)

  function pick(name: string) {
    onChange?.(name)
    open = false
    keyword = ''
  }

  function clear(e: MouseEvent) {
    e.stopPropagation()
    onChange?.('')
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger {disabled}>
    {#snippet child({ props })}
      <button
        {...props}
        type="button"
        class={cn(
          'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/50',
          'flex h-9 w-full items-center gap-2 rounded-md border px-3 py-1 text-sm',
          'shadow-xs outline-none focus-visible:ring-[3px]',
          disabled && 'cursor-not-allowed opacity-50'
        )}>
        {#if CurrentIcon}
          <CurrentIcon class="size-4 shrink-0" />
          <span class="truncate">{value}</span>
        {:else}
          <span class="text-muted-foreground truncate">请选择图标</span>
        {/if}
        <span class="ml-auto flex items-center gap-1">
          {#if value && !disabled}
            <!-- svelte-ignore a11y_consider_explicit_label -->
            <span
              role="button"
              tabindex="-1"
              title="清除图标"
              onclick={clear}
              onkeydown={(e) => e.key === 'Enter' && clear(e as unknown as MouseEvent)}>
              <X class="text-muted-foreground hover:text-foreground size-4" />
            </span>
          {/if}
          <ChevronDown class="size-4 opacity-50" />
        </span>
      </button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content class="w-[320px] p-2">
    <Input bind:value={keyword} placeholder="搜索图标" class="mb-2 h-8" />
    <ScrollArea.Root class="h-[240px]">
      {#if visible.length === 0}
        <p class="text-muted-foreground py-8 text-center text-sm">无匹配图标</p>
      {:else}
        <div class="grid grid-cols-5 gap-1 pr-2">
          {#each visible as name (name)}
            {@const Ico = resolveMenuIcon(name)}
            <button
              type="button"
              title={name}
              aria-label={name}
              aria-pressed={name === value}
              onclick={() => pick(name)}
              class={cn(
                'hover:bg-accent flex flex-col items-center gap-1 rounded-md p-2',
                name === value && 'bg-accent ring-ring ring-2'
              )}>
              <Ico class="size-5" />
              <span class="w-full truncate text-[10px] leading-tight">{name}</span>
            </button>
          {/each}
        </div>
      {/if}
    </ScrollArea.Root>
  </Popover.Content>
</Popover.Root>
