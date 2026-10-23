<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar/index.js'
  import { useLayout } from '$lib/context/layout.svelte.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import NavTree from './NavTree.svelte'
  import { APP_ICON } from './data/sidebar-data'

  let layout = useLayout()
  const auth = useAuth()

  // 自定义系统图标(系统信息页上传)优先;未配置回退内置 LayoutGrid。
  // 系统标题不在这里 —— 顶栏折叠钮右侧展示(标题通常太长,侧栏放不下)。
  let iconUrl = $derived(auth.sysIcon)
</script>

<Sidebar.Root collapsible={layout.collapsible} variant={layout.variant}>
  <Sidebar.Header>
    <div
      class="flex h-12 min-w-0 items-center justify-center gap-2 px-2 transition-[gap] duration-200 ease-linear group-data-[collapsible=icon]:gap-0"
    >
      {#if iconUrl}
        <img
          src={iconUrl}
          alt="logo"
          class="size-8 shrink-0 rounded-lg object-contain"
        />
      {:else}
        <div
          class="bg-sidebar-primary text-sidebar-primary-foreground flex aspect-square size-8 shrink-0 items-center justify-center rounded-lg"
        >
          <APP_ICON class="size-4" />
        </div>
      {/if}
    </div>
  </Sidebar.Header>
  <Sidebar.Content>
    <NavTree nodes={auth.navTree} />
  </Sidebar.Content>
  <Sidebar.Rail />
</Sidebar.Root>
