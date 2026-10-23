<script lang="ts">
  import { designerApi } from '$lib/api/workflow/designer.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import {
    joinPermissionFlag,
    splitPermissionFlag,
  } from '$lib/flow/node-fields.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'
  import HandlerPicker from './HandlerPicker.svelte'

  /**
   * 办理人(permissionFlag)编辑:条目是 storageId(user:1 / role:2 / dept:3)或表达式(${initiator}),
   * 名称经 handler-feedback 回显,回显失败就只显示原始串(规格 §12 降级)。
   */
  let {
    store,
    node,
    readonly = false,
  }: { store: DesignerStore; node: FlowNode; readonly?: boolean } = $props()

  const ids = $derived(splitPermissionFlag(node.data.json.permissionFlag))
  let names = $state(new Map<string, string>())
  let draft = $state('')
  let pickerOpen = $state(false)

  // 只有带冒号前缀的才是 storageId,表达式项不去后端问名字
  $effect(() => {
    const unknown = ids.filter((id) => id.includes(':') && !names.has(id))
    if (unknown.length === 0) return
    designerApi
      .handlerFeedback(unknown)
      .then((resp) => {
        const next = new Map(names)
        for (const item of resp.data ?? [])
          next.set(item.storageId, item.handlerName)
        for (const id of unknown) if (!next.has(id)) next.set(id, '')
        names = next
      })
      .catch(() => {
        const next = new Map(names)
        for (const id of unknown) next.set(id, '')
        names = next
      })
  })

  function write(list: string[]) {
    store.updateNodeJson(node.id, { permissionFlag: joinPermissionFlag(list) })
  }

  function addDraft() {
    const v = draft.trim()
    if (!v || ids.includes(v)) return
    write([...ids, v])
    draft = ''
  }

  function onPicked(rows: { storageId: string; handlerName: string }[]) {
    // 弹窗只知道本次勾选的行的名字;预勾的存量条目名字是空串,不能拿它盖掉已回显的
    const next = new Map(names)
    for (const r of rows)
      if (r.handlerName) next.set(r.storageId, r.handlerName)
    names = next
    // 表达式项保留,storageId 以弹窗结果为准
    const expressions = ids.filter((id) => !id.includes(':'))
    write([...expressions, ...rows.map((r) => r.storageId)])
  }
</script>

<ul class="divide-y rounded-md border text-sm">
  {#each ids as id (id)}
    <li class="flex items-center gap-2 px-2 py-1.5">
      <span class="min-w-0 flex-1 truncate">
        {#if names.get(id)}
          {names.get(id)}
          <span class="text-muted-foreground text-xs">({id})</span>
        {:else}
          {id}
        {/if}
      </span>
      {#if !readonly}
        <Button
          variant="ghost"
          size="sm"
          aria-label="移除 {id}"
          onclick={() => write(ids.filter((x) => x !== id))}>✕</Button
        >
      {/if}
    </li>
  {:else}
    <li class="text-muted-foreground px-2 py-3 text-center">未设置办理人</li>
  {/each}
</ul>

{#if !readonly}
  <div class="mt-2 flex gap-2">
    <Input
      class="min-w-0 flex-1"
      placeholder={'手动输入 user:1 / role:2 / ${initiator}'}
      aria-label="办理人手动输入"
      bind:value={draft}
      onkeydown={(e) => e.key === 'Enter' && addDraft()}
    />
    <Button variant="outline" size="sm" onclick={addDraft}>添加</Button>
    <Button size="sm" onclick={() => (pickerOpen = true)}>选择</Button>
  </div>
  <HandlerPicker
    bind:open={pickerOpen}
    selected={ids.filter((id) => id.includes(':'))}
    onConfirm={onPicked}
  />
{/if}
