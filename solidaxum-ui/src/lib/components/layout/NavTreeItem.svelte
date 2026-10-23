<script lang="ts">
  import { page } from '$app/state'
  import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
  } from '$lib/components/ui/collapsible/index.js'
  import {
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarMenuSub,
    SidebarMenuSubButton,
    SidebarMenuSubItem,
    useSidebar,
  } from '$lib/components/ui/sidebar/index.js'
  import { ChevronRight } from '@lucide/svelte'
  import { hiddenRouteFor } from '$lib/stores/hidden-routes.js'
  import type { NavNode } from './data/types'
  import Self from './NavTreeItem.svelte'

  let { node, depth = 0 }: { node: NavNode; depth?: number } = $props()
  const sidebar = useSidebar()

  /**
   * 激活路径:隐藏页(不在菜单树)按其 meta.activeMenu 计入所属菜单,
   * 让对应目录保持展开、对应叶子高亮(对齐 Vue 原版 el-menu 的
   * default-active = route.meta.activeMenu 行为);普通页用当前路径。
   */
  const activePath = $derived(
    hiddenRouteFor(page.url.pathname)?.activeMenu ?? page.url.pathname
  )
  const active = $derived(nodeActive(node))
  let open = $state(false)
  let userToggled = $state(false)

  $effect.pre(() => {
    const pathname = activePath
    userToggled = false
    open = active
  })

  function nodeActive(n: NavNode): boolean {
    const here = activePath
    if (n.url) return here === n.url || here.split('?')[0] === n.url || here.startsWith(n.url + '/')
    return (n.items ?? []).some(nodeActive)
  }
  function handleNavClick() {
    sidebar.setOpenMobile(false)
  }
  function handleOpenChange(value: boolean) {
    userToggled = true
    open = value
  }
</script>

{#if !node.items}
  <!-- 叶子:depth>0 用 Sub 按钮,depth==0 用主按钮 -->
  {#if depth === 0}
    <SidebarMenuItem>
      <SidebarMenuButton isActive={nodeActive(node)} tooltipContent={node.title}>
        {#snippet child({ props })}
          <a {...props} href={node.url} onclick={handleNavClick}>
            {#if node.icon}<node.icon />{/if}
            <span>{node.title}</span>
          </a>
        {/snippet}
      </SidebarMenuButton>
    </SidebarMenuItem>
  {:else}
    <SidebarMenuSubItem>
      <SidebarMenuSubButton href={node.url} isActive={nodeActive(node)} onclick={handleNavClick}>
        {#if node.icon}<node.icon />{/if}
        <span>{node.title}</span>
      </SidebarMenuSubButton>
    </SidebarMenuSubItem>
  {/if}
{:else}
  <!-- 目录:可折叠;命中路径链默认展开,用户可手动折叠 -->
  <Collapsible open={open} onOpenChange={handleOpenChange} class="group/collapsible">
    <SidebarMenuItem>
      <CollapsibleTrigger>
        {#snippet child({ props })}
          <SidebarMenuButton {...props} tooltipContent={node.title}>
            {#if node.icon}<node.icon />{/if}
            <!-- 目录项的标题不是最后一个子元素(ChevronRight 在其后),
                 拿不到 shadcn 的 [&>span:last-child]:truncate,184px 定宽下
                 「日志管理」这类四字标题会折成两行,故显式截断。 -->
            <span class="truncate">{node.title}</span>
            <ChevronRight
              class="ms-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90"
            />
          </SidebarMenuButton>
        {/snippet}
      </CollapsibleTrigger>
      <CollapsibleContent>
        <SidebarMenuSub>
          {#each node.items as child (child.title)}
            <Self node={child} depth={depth + 1} />
          {/each}
        </SidebarMenuSub>
      </CollapsibleContent>
    </SidebarMenuItem>
  </Collapsible>
{/if}
