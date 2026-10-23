<script lang="ts">
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import { cn } from '$lib/utils.js'
  import { ChevronRight, ChevronDown, EyeOff, Ban } from '@lucide/svelte'
  import type { PermissionNode } from './permission-tree.js'
  import {
    allSelectableIds,
    isAllSelected,
    nodeCheckState,
    toggleButton,
    toggleNode,
  } from './permission-tree.js'

  let {
    nodes = [],
    checked = new Set<string>(),
    linked = true,
    showButtonColumn = true,
    onChange,
    onLinkedChange,
  }: {
    nodes?: PermissionNode[]
    checked?: ReadonlySet<string>
    /** true = 父子联动(对齐后端 menuCheckStrictly / deptCheckStrictly) */
    linked?: boolean
    /** 数据权限树没有按钮权限,关掉列头 */
    showButtonColumn?: boolean
    onChange?: (next: Set<string>) => void
    onLinkedChange?: (next: boolean) => void
  } = $props()

  let expandAll = $state(true)
  let collapsedIds = $state(new Set<string>())

  const allSelected = $derived(isAllSelected(nodes, checked))

  function isExpanded(id: string): boolean {
    return expandAll ? !collapsedIds.has(id) : collapsedIds.has(id)
  }

  function toggleExpanded(id: string) {
    const next = new Set(collapsedIds)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    collapsedIds = next
  }

  function handleExpandAll(next: boolean) {
    expandAll = next
    collapsedIds = new Set()
  }

  function handleSelectAll(next: boolean) {
    onChange?.(next ? new Set(allSelectableIds(nodes)) : new Set())
  }

  function handleNode(id: string, next: boolean) {
    onChange?.(toggleNode(nodes, checked, id, next, { linked }))
  }

  function handleButton(id: string, next: boolean) {
    onChange?.(toggleButton(nodes, checked, id, next, { linked }))
  }
</script>

<div class="space-y-2">
  <div class="flex flex-wrap items-center gap-4 text-sm">
    <label class="flex items-center gap-2">
      <Checkbox
        aria-label="展开/折叠"
        checked={expandAll}
        onCheckedChange={(v) => handleExpandAll(v === true)}
      />
      展开/折叠
    </label>
    <label class="flex items-center gap-2">
      <Checkbox
        aria-label="全选/全不选"
        checked={allSelected}
        onCheckedChange={(v) => handleSelectAll(v === true)}
      />
      全选/全不选
    </label>
    <label class="flex items-center gap-2">
      <Checkbox
        aria-label="父子联动"
        checked={linked}
        onCheckedChange={(v) => onLinkedChange?.(v === true)}
      />
      父子联动
    </label>
  </div>

  <div class="rounded-md border">
    {#if showButtonColumn}
      <div class="text-muted-foreground flex items-center border-b px-2 py-1.5 text-xs">
        <span class="flex-1">菜单名称</span>
        <span class="w-1/2">按钮权限</span>
      </div>
    {/if}
    <div class="max-h-80 overflow-auto p-2">
      {#if nodes.length === 0}
        <div class="text-muted-foreground px-2 py-1.5 text-sm">加载中,请稍候</div>
      {/if}
      {#each nodes as node (node.id)}
        {@render treeRow(node, 0)}
      {/each}
    </div>
  </div>
</div>

{#snippet treeRow(node: PermissionNode, depth: number)}
  {@const state = nodeCheckState(node, checked)}
  {@const hasChildren = node.children.length > 0}
  {@const expanded = isExpanded(node.id)}
  <div>
    <div class="flex items-center gap-1 py-0.5 text-sm" style="padding-left: {depth * 14}px">
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
      <label class={cn('flex flex-1 items-center gap-2', node.disabled && 'opacity-50')}>
        <Checkbox
          aria-label={node.label}
          checked={state === 'checked'}
          indeterminate={state === 'indeterminate'}
          disabled={node.disabled}
          onCheckedChange={(v) => handleNode(node.id, v === true)}
        />
        <span class="truncate">{node.label}</span>
        {#if node.hidden}
          <EyeOff class="text-muted-foreground size-3.5" aria-label="隐藏" />
        {/if}
        {#if node.disabled}
          <Ban class="text-muted-foreground size-3.5" aria-label="停用" />
        {/if}
      </label>
      {#if node.buttons.length > 0}
        <div class="flex w-1/2 flex-wrap items-center gap-3">
          {#each node.buttons as button (button.id)}
            <label class={cn('flex items-center gap-1.5', button.disabled && 'opacity-50')}>
              <Checkbox
                aria-label={button.label}
                checked={checked.has(button.id)}
                disabled={button.disabled}
                onCheckedChange={(v) => handleButton(button.id, v === true)}
              />
              <span class="truncate text-xs">{button.label}</span>
            </label>
          {/each}
        </div>
      {/if}
    </div>
    {#if hasChildren && expanded}
      <!-- 业务树展开高度动画(grid-rows 技法,节点不卸载 DOM) -->
      <div class="grid transition-[grid-template-rows] duration-200" style="grid-template-rows: 1fr">
        <div class="overflow-hidden min-h-0">
          {#each node.children as child (child.id)}
            {@render treeRow(child, depth + 1)}
          {/each}
        </div>
      </div>
    {/if}
  </div>
{/snippet}
