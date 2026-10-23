<script lang="ts">
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
  } from '$lib/components/ui/dropdown-menu/index.js'
  import type { TabsStore } from '$lib/stores/tabs.svelte.js'
  import { flip } from 'svelte/animate'
  import { X, ChevronDown, RotateCw } from '@lucide/svelte'

  let {
    tabs,
    onRefresh,
  }: { tabs: TabsStore; onRefresh: (path: string) => void } = $props()

  const active = $derived(page.url.pathname)

  /** 导航并在落地后清空批量关闭护栏(store 临时屏蔽刚关掉的路径,防止
   *  陈旧 $effect 把它重新 open 出来)。 */
  function navTo(path: string) {
    goto(path).then(() => tabs.clearClosingGuard())
  }
  function activate(path: string) {
    if (path !== active) goto(path)
  }
  function closeTab(path: string) {
    const next = tabs.close(path)
    if (path === active && next !== active) navTo(next)
    else tabs.clearClosingGuard()
  }
  function closeOthers(path: string) {
    tabs.closeOthers(path)
    if (active !== path && !tabs.tabs.some((t) => t.path === active)) navTo(path)
    else tabs.clearClosingGuard()
  }
  function closeRight(path: string) {
    tabs.closeRight(path)
    if (!tabs.tabs.some((t) => t.path === active)) navTo(path)
    else tabs.clearClosingGuard()
  }
  function closeAll() {
    tabs.closeAll()
    navTo(tabs.tabs[0].path)
  }
</script>

<div data-testid="tabbar" class="bg-header flex items-center px-5 py-1">
  <div
    data-testid="tabbar-track"
    class="bg-card flex flex-1 items-center gap-1 overflow-x-auto rounded-xl px-2 py-1"
  >
    {#each tabs.tabs as tab (tab.path)}
      <div
        animate:flip={{ duration: 200 }}
        class="flex shrink-0 items-center rounded-md border px-2 py-1 text-sm {tab.path ===
        active
          ? 'bg-primary text-primary-foreground border-primary'
          : 'bg-muted/40 hover:bg-muted'}"
      >
        <button
          type="button"
          class="max-w-40 truncate"
          onclick={() => activate(tab.path)}>{tab.title}</button
        >
        {#if !tab.affix}
          <button
            type="button"
            class="ml-1 rounded hover:bg-black/10"
            aria-label="关闭"
            onclick={() => closeTab(tab.path)}
          >
            <X class="size-3.5" />
          </button>
        {/if}
      </div>
    {/each}

    <!-- 全局操作下拉(对齐 RuoYi):放最右边,操作针对当前激活 tab -->
    <div class="ml-auto shrink-0">
    <DropdownMenu>
      <DropdownMenuTrigger
        class="hover:bg-muted inline-flex size-7 items-center justify-center rounded-md"
        aria-label="标签操作"
      >
        <ChevronDown class="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onclick={() => onRefresh(active)}>
          <RotateCw class="mr-2 size-4" />刷新当前
        </DropdownMenuItem>
        <DropdownMenuItem onclick={() => closeTab(active)}
          >关闭当前</DropdownMenuItem
        >
        <DropdownMenuItem onclick={() => closeOthers(active)}
          >关闭其他</DropdownMenuItem
        >
        <DropdownMenuItem onclick={() => closeRight(active)}
          >关闭右侧</DropdownMenuItem
        >
        <DropdownMenuItem onclick={() => closeAll()}>全部关闭</DropdownMenuItem
        >
      </DropdownMenuContent>
    </DropdownMenu>
    </div>
  </div>
</div>
