<script lang="ts">
  import { genApi, sortPreviewNames } from '$lib/api/tool/gen.js'
  import type { GenPreview } from '$lib/api/tool/gen.js'
  import { highlightCode } from './code-highlight.js'
  import './code-highlight.css'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { Copy } from '@lucide/svelte'
  import { toast } from 'svelte-sonner'

  let {
    open,
    tableId,
    onOpenChange,
  }: {
    open: boolean
    tableId: string | null
    onOpenChange: (v: boolean) => void
  } = $props()

  let preview = $state<GenPreview>({})
  let loading = $state(false)
  // 后端返回的是无序 HashMap,必须自己排序,否则默认 tab 每次可能不一样。
  const names = $derived(sortPreviewNames(Object.keys(preview)))

  async function load() {
    if (tableId == null) return
    preview = {}
    loading = true
    try {
      const r = await genApi.preview(tableId)
      preview = r.data ?? {}
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '预览加载失败')
    } finally {
      loading = false
    }
  }
  $effect(() => {
    if (open && tableId != null) load()
  })

  async function copy(code: string) {
    try {
      await navigator.clipboard.writeText(code)
      toast.success('已复制')
    } catch {
      toast.error('复制失败')
    }
  }
  // 模板名可能是完整路径，取末段作 tab 标签
  function shortName(n: string): string {
    return n.split(/[\\/]/).pop() ?? n
  }
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Content class="max-w-4xl">
    <Dialog.Header><Dialog.Title>代码预览</Dialog.Title></Dialog.Header>
    {#if loading}
      <div class="text-muted-foreground py-8 text-center">加载中...</div>
    {:else if names.length === 0}
      <div class="text-muted-foreground py-8 text-center">无预览内容</div>
    {:else}
      <Tabs.Root value={names[0]}>
        <Tabs.List class="max-w-full flex-wrap overflow-x-auto">
          {#each names as n}<Tabs.Trigger value={n}>{shortName(n)}</Tabs.Trigger
            >{/each}
        </Tabs.List>
        {#each names as n}
          <Tabs.Content value={n}>
            <div class="flex justify-end pb-2">
              <Button
                variant="outline"
                size="sm"
                onclick={() => copy(preview[n])}
              >
                <Copy class="mr-2 h-4 w-4" />复制
              </Button>
            </div>
            <pre
              class="bg-muted max-h-[55vh] overflow-auto rounded-md p-3 text-xs"><code
                class="hljs">{@html highlightCode(preview[n], n)}</code
              ></pre>
          </Tabs.Content>
        {/each}
      </Tabs.Root>
    {/if}
  </Dialog.Content>
</Dialog.Root>
