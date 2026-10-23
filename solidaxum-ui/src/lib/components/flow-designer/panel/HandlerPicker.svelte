<script lang="ts">
  import {
    designerApi,
    type HandlerAuthRow,
  } from '$lib/api/workflow/designer.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'

  /**
   * 办理人选择弹窗:tab 来自 handler-type(用户/角色/部门),列表来自 handler-result 分页。
   * 后端不接搜索/分组参数(规格 §9.2),所以只有翻页。跨 tab、跨页的勾选按 storageId 累计。
   */
  let {
    open = $bindable(false),
    selected = [],
    onConfirm,
  }: {
    open?: boolean
    /** 已选的 storageId,打开时预勾 */
    selected?: string[]
    onConfirm: (rows: { storageId: string; handlerName: string }[]) => void
  } = $props()

  const PAGE_SIZE = 10
  let types = $state<string[]>([])
  let active = $state('')
  let rows = $state<HandlerAuthRow[]>([])
  let total = $state(0)
  let pageNum = $state(1)
  let loading = $state(false)
  let picked = $state(new Map<string, string>())

  const pages = $derived(Math.max(1, Math.ceil(total / PAGE_SIZE)))

  // 失败时 apiClient 已经 toast 过,这里只保证不留未处理的 rejection
  async function loadTypes() {
    if (types.length) return
    try {
      const resp = await designerApi.handlerType()
      types = resp.data ?? []
      active = types[0] ?? ''
    } catch {
      types = []
    }
  }

  async function loadPage() {
    if (!active) return
    loading = true
    try {
      const resp = await designerApi.handlerResult({
        handlerType: active,
        pageNum,
        pageSize: PAGE_SIZE,
        status: '0',
      })
      rows = resp.data?.handlerAuths.rows ?? []
      total = resp.data?.handlerAuths.total ?? 0
    } catch {
      rows = []
      total = 0
    } finally {
      loading = false
    }
  }

  $effect(() => {
    if (!open) return
    picked = new Map(selected.map((id) => [id, '']))
    pageNum = 1
    loadTypes().then(loadPage)
  })

  function switchTab(t: string) {
    active = t
    pageNum = 1
    loadPage()
  }

  function toggle(row: HandlerAuthRow, checked: boolean) {
    const next = new Map(picked)
    if (checked) next.set(row.storageId, row.handlerName)
    else next.delete(row.storageId)
    picked = next
  }

  function confirm() {
    onConfirm(
      [...picked].map(([storageId, handlerName]) => ({
        storageId,
        handlerName,
      }))
    )
    open = false
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-2xl">
    <Dialog.Header><Dialog.Title>选择办理人</Dialog.Title></Dialog.Header>
    {#if types.length}
      <Tabs.Root value={active} onValueChange={switchTab}>
        <Tabs.List>
          {#each types as t (t)}<Tabs.Trigger value={t}>{t}</Tabs.Trigger
            >{/each}
        </Tabs.List>
      </Tabs.Root>
    {/if}
    <div class="max-h-80 overflow-auto rounded-md border">
      <table class="w-full text-sm">
        <thead class="bg-muted/50 text-muted-foreground text-xs">
          <tr>
            <th class="w-8 p-2"></th>
            <th class="p-2 text-left">名称</th>
            <th class="p-2 text-left">编码</th>
            <th class="p-2 text-left">分组</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as row (row.storageId)}
            <tr class="border-t">
              <td class="p-2">
                <Checkbox
                  aria-label={row.handlerName}
                  checked={picked.has(row.storageId)}
                  onCheckedChange={(v) => toggle(row, v === true)}
                />
              </td>
              <td class="p-2">{row.handlerName}</td>
              <td class="text-muted-foreground p-2">{row.handlerCode}</td>
              <td class="text-muted-foreground p-2">{row.groupName ?? ''}</td>
            </tr>
          {:else}
            <tr>
              <td colspan="4" class="text-muted-foreground p-4 text-center">
                {loading ? '加载中…' : '没有数据'}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="flex items-center justify-between text-sm">
      <span class="text-muted-foreground">
        已选 {picked.size} 项 · 第 {pageNum}/{pages} 页,共 {total} 条
      </span>
      <span class="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={pageNum <= 1 || loading}
          onclick={() => {
            pageNum -= 1
            loadPage()
          }}>上一页</Button
        >
        <Button
          variant="outline"
          size="sm"
          disabled={pageNum >= pages || loading}
          onclick={() => {
            pageNum += 1
            loadPage()
          }}>下一页</Button
        >
      </span>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (open = false)}>取消</Button>
      <Button onclick={confirm}>确定</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
