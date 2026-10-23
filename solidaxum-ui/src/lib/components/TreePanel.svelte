<script lang="ts">
  import { Button } from '$lib/components/ui/button/index.js'
  import { cn } from '$lib/utils.js'
  import { ChevronRight, ChevronDown, PanelLeftClose, PanelLeftOpen } from '@lucide/svelte'
  import type { TreeSelectNode } from './tree-select-types.js'
  import { filterTreeByLabel, collectExpandableIds } from './tree-panel-utils.js'

  let {
    title,
    placeholder = '请输入名称',
    nodes = [],
    selectedId,
    onNodeClick,
  }: {
    title: string
    placeholder?: string
    nodes?: TreeSelectNode[]
    selectedId?: string | undefined
    /** 点同一节点第二次 → 传 undefined(取消过滤),对齐 Vue「再点一次回到全部」的直觉 */
    onNodeClick?: (id: string | undefined) => void
  } = $props()

  let collapsed = $state(false)
  let keyword = $state('')
  let expandedIds = $state(new Set<string | number>())
  let autoExpanded = false

  const visibleNodes = $derived(filterTreeByLabel(nodes, keyword))

  // 数据首次到达时全展开一次;之后由用户手动控制,不再强行覆盖。
  $effect(() => {
    if (!autoExpanded && nodes.length > 0) {
      expandedIds = new Set(collectExpandableIds(nodes))
      autoExpanded = true
    }
  })

  function toggleExpanded(id: string | number) {
    const next = new Set(expandedIds)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    expandedIds = next
  }

  function clickNode(id: string | number) {
    onNodeClick?.(selectedId === id ? undefined : String(id))
  }
</script>

<div
  data-slot="content-card"
  class={cn('bg-card shrink-0 rounded-lg border', collapsed ? 'w-12' : 'w-60')}
>
  <div class="flex items-center justify-between border-b px-2 py-2">
    {#if !collapsed}<span class="text-sm font-medium">{title}</span>{/if}
    <Button
      variant="ghost"
      size="icon"
      class="size-7"
      title={collapsed ? '展开面板' : '折叠面板'}
      aria-label={collapsed ? '展开面板' : '折叠面板'}
      onclick={() => (collapsed = !collapsed)}
    >
      {#if collapsed}
        <PanelLeftOpen class="size-4" />
      {:else}
        <PanelLeftClose class="size-4" />
      {/if}
    </Button>
  </div>
  {#if !collapsed}
    <div class="p-2">
      <input
        class="border-input mb-2 h-8 w-full rounded-md border px-2 text-sm"
        {placeholder}
        aria-label={placeholder}
        bind:value={keyword}
      />
      <div class="max-h-[28rem] overflow-auto">
        {#if visibleNodes.length === 0}
          <div class="text-muted-foreground px-2 py-1.5 text-sm">暂无数据</div>
        {/if}
        {#each visibleNodes as node (node.id)}
          {@render treeNode(node, 0)}
        {/each}
      </div>
    </div>
  {/if}
</div>

{#snippet treeNode(node: TreeSelectNode, depth: number)}
  {@const hasChildren = !!node.children && node.children.length > 0}
  {@const expanded = expandedIds.has(node.id)}
  <div>
    <div
      class={cn(
        'flex w-full items-center rounded-sm text-sm',
        selectedId === node.id && 'bg-accent text-accent-foreground'
      )}
      style="padding-left: {depth * 12 + 4}px"
    >
      {#if hasChildren}
        <button
          type="button"
          class="inline-flex size-4 items-center justify-center border-0 bg-transparent p-0"
          aria-expanded={expanded}
          aria-label={expanded ? '折叠' : '展开'}
          onclick={() => toggleExpanded(node.id)}
        >
          {#if expanded}<ChevronDown class="size-4" />{:else}<ChevronRight class="size-4" />{/if}
        </button>
      {:else}
        <span class="size-4"></span>
      {/if}
      <button
        type="button"
        class="hover:bg-accent flex-1 truncate border-0 bg-transparent px-2 py-1.5 text-left"
        onclick={() => clickNode(node.id)}
      >
        {node.label}
      </button>
    </div>
    {#if hasChildren && expanded}
      {#each node.children ?? [] as child (child.id)}
        {@render treeNode(child, depth + 1)}
      {/each}
    {/if}
  </div>
{/snippet}
