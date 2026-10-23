<script lang="ts">
  import { goto } from '$app/navigation'
  import * as Command from '$lib/components/ui/command/index.js'
  import { ScrollArea } from '$lib/components/ui/scroll-area/index.js'
  import { useSearch } from '$lib/context/search.svelte.js'
  import { useAuth } from '$lib/context/auth.svelte.js'
  import { flattenNavLinks } from '$lib/menu/transform.js'
  import { useTheme } from '$lib/context/theme.svelte.js'
  import { COLOR_THEMES } from '$lib/themes/registry.js'
  import { Laptop, Moon, Sun } from '@lucide/svelte'

  const search = useSearch()
  const theme = useTheme()
  const auth = useAuth()

  const navLinks = $derived(flattenNavLinks(auth.navTree))

  function runCommand(command: () => void) {
    search.setOpen(false)
    command()
  }
</script>

<Command.Dialog open={search.open} onOpenChange={search.setOpen}>
  <Command.Input placeholder="输入命令或搜索..." />
  <Command.List>
    <ScrollArea type="hover" class="h-72 pe-1">
      <Command.Empty>未找到结果。</Command.Empty>
      {#each navLinks as navItem (navItem.url)}
        <Command.Item
          value={`${navItem.title} ${navItem.url}`}
          onSelect={() => runCommand(() => goto(navItem.url))}
        >
          <span>{navItem.title}</span>
        </Command.Item>
      {/each}
      <Command.Separator />
      <Command.Group heading="颜色主题">
        {#each COLOR_THEMES as ct}
          <Command.Item
            onSelect={() => runCommand(() => theme.setColorTheme(ct.name))}
          >
            <span>{ct.label}</span>
          </Command.Item>
        {/each}
      </Command.Group>
      <Command.Group heading="外观模式">
        <Command.Item
          onSelect={() => runCommand(() => theme.setTheme('light'))}
        >
          <Sun class="mr-2 size-4" /> <span>浅色</span>
        </Command.Item>
        <Command.Item onSelect={() => runCommand(() => theme.setTheme('dark'))}>
          <Moon class="mr-2 size-4 scale-90" /> <span>深色</span>
        </Command.Item>
        <Command.Item
          onSelect={() => runCommand(() => theme.setTheme('system'))}
        >
          <Laptop class="mr-2 size-4" /> <span>跟随系统</span>
        </Command.Item>
      </Command.Group>
    </ScrollArea>
  </Command.List>
</Command.Dialog>
