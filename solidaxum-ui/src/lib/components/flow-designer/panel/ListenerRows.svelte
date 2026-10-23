<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { LISTENER_TYPES, type ListenerRow } from '$lib/flow/node-fields.js'
  import { loadListenerNames } from './options.js'

  // 监听器行:类型下拉 + 路径输入(后端 listener-list 给的名字作 datalist 建议,也可手输类路径)。
  // 刚加的空行还没内容,写回时会被拼串规则丢掉,所以行列表以本地草稿为准:
  // 只有当 store 里的行与本地"已填的行"对不上(换了节点、外部改动)才用 props 覆盖。
  // 比较时两边都只看已填的行:存量拼串带尾分隔符("start," / "@@")会解析出全空行,
  // 拿它跟过滤后的本地行硬比永远不等,effect 会一直互相覆盖。
  let {
    rows,
    readonly = false,
    onChange,
  }: {
    rows: ListenerRow[]
    readonly?: boolean
    onChange: (rows: ListenerRow[]) => void
  } = $props()

  let local = $state<ListenerRow[]>([])
  let names = $state<string[]>([])

  const filled = (list: ListenerRow[]) =>
    list.filter((r) => r.listenerType.trim() || r.listenerPath.trim())

  $effect(() => {
    if (JSON.stringify(filled(local)) !== JSON.stringify(filled(rows))) {
      local = rows.map((r) => ({ ...r }))
    }
  })

  onMount(() => {
    loadListenerNames().then((list) => (names = list))
  })

  function set(next: ListenerRow[]) {
    local = next
    onChange(next)
  }

  function patch(i: number, part: Partial<ListenerRow>) {
    set(local.map((r, idx) => (idx === i ? { ...r, ...part } : r)))
  }
</script>

<div class="space-y-2">
  {#each local as row, i (i)}
    <div class="flex items-center gap-2">
      <!-- 空串在 bits-ui 里就是未选中,触发器显示占位"类型" -->
      <Select.Root
        type="single"
        value={row.listenerType}
        disabled={readonly}
        onValueChange={(v) => patch(i, { listenerType: v })}
      >
        <Select.Trigger class="w-24 shrink-0" aria-label="监听器类型">
          {LISTENER_TYPES.find((t) => t.value === row.listenerType)?.label ??
            '类型'}
        </Select.Trigger>
        <Select.Content>
          {#each LISTENER_TYPES as t (t.value)}
            <Select.Item value={t.value} label={t.label} />
          {/each}
        </Select.Content>
      </Select.Root>
      <Input
        class="min-w-0 flex-1"
        list="fd-listener-names"
        aria-label="监听器路径"
        placeholder="监听器名称或类路径"
        value={row.listenerPath}
        disabled={readonly}
        oninput={(e) => patch(i, { listenerPath: e.currentTarget.value })}
      />
      {#if !readonly}
        <Button
          variant="ghost"
          size="sm"
          aria-label="删除监听器"
          onclick={() => set(local.filter((_, idx) => idx !== i))}>✕</Button
        >
      {/if}
    </div>
  {/each}
  {#if !readonly}
    <Button
      variant="outline"
      size="sm"
      onclick={() => set([...local, { listenerType: '', listenerPath: '' }])}
      >+ 监听器</Button
    >
  {/if}
  <datalist id="fd-listener-names">
    {#each names as n (n)}<option value={n}></option>{/each}
  </datalist>
</div>
