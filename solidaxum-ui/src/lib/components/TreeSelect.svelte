<script lang="ts">
  import * as Popover from '$lib/components/ui/popover/index.js'
  import * as ScrollArea from '$lib/components/ui/scroll-area/index.js'
  import { cn } from '$lib/utils.js'
  import { ChevronRight, ChevronDown } from '@lucide/svelte'
  import type { TreeSelectNode } from './tree-select-types.js'
  import { findNodeLabel, pruneSubtrees } from './tree-select-utils.js'

  let {
    treeData = [],
    value = $bindable<string | number | undefined>(undefined),
    placeholder = '请选择',
    excludeIds = [],
    rootOption,
    disabled = false,
    onChange,
  }: {
    treeData?: TreeSelectNode[]
    value?: string | number
    placeholder?: string
    /** 排除这些节点及其全部子孙(防循环挂载)。 */
    excludeIds?: (string | number)[]
    /** 可选的合成根节点,如菜单的 {id: 0, label: '主类目'}。 */
    rootOption?: { id: string | number; label: string }
    disabled?: boolean
    onChange?: (id: string | number) => void
  } = $props()

  let open = $state(false)
  let expandedIds = $state(new Set<string | number>())

  const visibleNodes = $derived(pruneSubtrees(treeData, excludeIds))
  const displayLabel = $derived(
    findNodeLabel(visibleNodes, value) ?? placeholder
  )

  function selectNode(id: string | number) {
    value = id
    onChange?.(id)
    open = false
  }

  function toggleExpanded(id: string | number) {
    const next = new Set(expandedIds)
    if (next.has(id)) {
      next.delete(id)
    } else {
      next.add(id)
    }
    expandedIds = next
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger {disabled}>
    {#snippet child({ props })}
      <button
        type="button"
        {...props}
        class={cn(
          'border-input ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex h-9 w-full items-center justify-between rounded-md border bg-transparent px-3 py-2 text-sm shadow-sm transition-colors focus-visible:ring-1 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
          !value && 'text-muted-foreground'
        )}
      >
        <span class="truncate">{displayLabel}</span>
        <ChevronDown class="h-4 w-4 opacity-50" />
      </button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content class="w-72 p-0" align="start">
    <ScrollArea.Root class="h-64 w-full">
      <div class="p-2">
        {#if rootOption}
          <button
            type="button"
            class={cn(
              'hover:bg-accent hover:text-accent-foreground flex w-full items-center rounded-sm px-2 py-1.5 text-left text-sm',
              value === rootOption.id && 'bg-accent text-accent-foreground'
            )}
            onclick={() => selectNode(rootOption.id)}
          >
            {rootOption.label}
          </button>
        {/if}
        {#if visibleNodes.length === 0 && !rootOption}
          <div class="text-muted-foreground px-2 py-1.5 text-sm">暂无数据</div>
        {/if}
        {#each visibleNodes as node (node.id)}
          {@render treeNode(node, 0)}
        {/each}
      </div>
    </ScrollArea.Root>
  </Popover.Content>
</Popover.Root>

{#snippet treeNode(node: TreeSelectNode, depth: number)}
  {@const hasChildren = !!node.children && node.children.length > 0}
  {@const expanded = expandedIds.has(node.id)}
  <div>
    <div
      class={cn(
        'flex w-full items-center rounded-sm text-sm',
        value === node.id && 'bg-accent text-accent-foreground',
        !node.disabled && 'hover:bg-accent hover:text-accent-foreground'
      )}
      style="padding-left: {depth * 16 + 8}px"
    >
      {#if hasChildren}
        <button
          type="button"
          class="inline-flex size-4 items-center justify-center border-0 bg-transparent p-0"
          aria-expanded={expanded}
          aria-label={expanded ? '折叠' : '展开'}
          onclick={(e) => {
            e.stopPropagation()
            toggleExpanded(node.id)
          }}
        >
          {#if expanded}
            <ChevronDown class="size-4" />
          {:else}
            <ChevronRight class="size-4" />
          {/if}
        </button>
      {:else}
        <span class="size-4"></span>
      {/if}
      <button
        type="button"
        class="flex-1 border-0 bg-transparent px-2 py-1.5 text-left disabled:pointer-events-none disabled:opacity-50"
        disabled={node.disabled}
        onclick={() => selectNode(node.id)}
      >
        <span class="truncate">{node.label}</span>
      </button>
    </div>
    {#if hasChildren && expanded}
      <div>
        {#each node.children ?? [] as child (child.id)}
          {@render treeNode(child, depth + 1)}
        {/each}
      </div>
    {/if}
  </div>
{/snippet}
