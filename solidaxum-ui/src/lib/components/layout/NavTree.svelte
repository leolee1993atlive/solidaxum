<script lang="ts">
  import { SidebarGroup, SidebarMenu } from '$lib/components/ui/sidebar/index.js'
  import NavTreeItem from './NavTreeItem.svelte'
  import type { NavNode } from './data/types'

  let { nodes }: { nodes: NavNode[] } = $props()
</script>

<SidebarGroup>
  <SidebarMenu>
    {#each nodes as node (node.title)}
      <NavTreeItem {node} depth={0} />
    {/each}
  </SidebarMenu>
</SidebarGroup>
