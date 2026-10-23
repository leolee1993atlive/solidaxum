<script lang="ts">
  import { genApi } from '$lib/api/tool/gen.js'
  import type { DbTableVo } from '$lib/api/tool/gen.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { toast } from 'svelte-sonner'
  import { untrack } from 'svelte'

  let {
    open,
    dataName = '',
    onOpenChange,
    onImported,
  }: {
    open: boolean
    dataName?: string
    onOpenChange: (v: boolean) => void
    onImported: () => void
  } = $props()

  let rows = $state<DbTableVo[]>([])
  let selected = $state<Set<string>>(new Set())
  let loading = $state(false)
  let saving = $state(false)

  // Vue 的导入弹窗有独立筛选表单 + 分页(importTable.vue:3-41)。
  // 后端 DbTableQueryBo + db_meta.rs:59-99 的 LIMIT/OFFSET 一直支持,
  // 此前一次性拉 pageSize:100 是纯前端退化 —— 超 100 张表的库导不全。
  let tableName = $state('')
  let tableComment = $state('')
  let pageNum = $state(1)
  const PAGE_SIZE = 10
  let total = $state(0)

  const totalPages = $derived(Math.max(1, Math.ceil(total / PAGE_SIZE)))

  async function load() {
    loading = true
    try {
      const r = await genApi.dbList(
        { pageNum, pageSize: PAGE_SIZE },
        {
          dataName: dataName || undefined,
          tableName: tableName || undefined,
          tableComment: tableComment || undefined,
        }
      )
      rows = r.rows
      total = r.total
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '加载失败')
    } finally {
      loading = false
    }
  }

  function search() {
    pageNum = 1
    load()
  }
  function reset() {
    tableName = ''
    tableComment = ''
    pageNum = 1
    load()
  }
  function goPage(n: number) {
    pageNum = Math.min(Math.max(1, n), totalPages)
    load()
  }

  // 打开时重置全部状态并首次加载。
  // ⚠️ 选中态**跨页保留** —— Vue 同样保留(el-table 的 reserve-selection 语义),
  //    且导入是按表名提交的,翻页不该丢已勾的表。
  // 🔴 必须 untrack:load() 内部读了 pageNum/tableName/tableComment/dataName,
  //    不隔离的话它们会变成本 effect 的依赖 —— 之后 search()/goPage() 一改 pageNum
  //    就会重跑整个 effect,把筛选和选中态全清掉,页永远翻不动。
  //    本 effect 只该依赖 open。
  $effect(() => {
    if (!open) return
    untrack(() => {
      rows = []
      selected = new Set()
      tableName = ''
      tableComment = ''
      pageNum = 1
      total = 0
      load()
    })
  })

  function toggle(name: string, checked: boolean) {
    const next = new Set(selected)
    if (checked) next.add(name)
    else next.delete(name)
    selected = next
  }
  async function confirm() {
    if (selected.size === 0) {
      toast.error('请选择要导入的表')
      return
    }
    saving = true
    try {
      await genApi.importTable([...selected].join(','), dataName || undefined)
      toast.success('导入成功')
      onImported()
    } catch (e) {
      console.error(e)
      toast.error(e instanceof Error ? e.message : '导入失败')
    } finally {
      saving = false
    }
  }
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Content class="max-w-3xl">
    <Dialog.Header><Dialog.Title>导入表</Dialog.Title></Dialog.Header>

    <!-- 筛选表单(对齐 Vue importTable.vue:4-20) -->
    <form
      class="flex flex-wrap items-end gap-3"
      onsubmit={(e) => {
        e.preventDefault()
        search()
      }}
    >
      <label class="flex flex-col gap-1 text-sm">
        <span>表名称</span>
        <input
          class="border-input h-9 rounded-md border px-3"
          placeholder="请输入表名称"
          bind:value={tableName} />
      </label>
      <label class="flex flex-col gap-1 text-sm">
        <span>表描述</span>
        <input
          class="border-input h-9 rounded-md border px-3"
          placeholder="请输入表描述"
          bind:value={tableComment} />
      </label>
      <div class="ml-auto flex items-center gap-2 pb-0.5">
        <Button type="submit" size="sm">搜索</Button>
        <Button type="button" size="sm" variant="outline" onclick={reset}>重置</Button>
      </div>
    </form>

    <div class="max-h-[45vh] overflow-y-auto">
      {#if loading}
        <div class="text-muted-foreground py-8 text-center">加载中...</div>
      {:else if rows.length === 0}
        <div class="text-muted-foreground py-8 text-center">无可导入的表</div>
      {:else}
        <table class="w-full text-sm">
          <thead>
            <tr class="text-muted-foreground border-b">
              <th class="w-10 p-2"></th>
              <th class="p-2 text-left">表名称</th>
              <th class="p-2 text-left">表描述</th>
              <th class="p-2 text-left">创建时间</th>
              <th class="p-2 text-left">更新时间</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as t (t.tableName)}
              {@const name = t.tableName ?? ''}
              <!-- 整行可点切换勾选(Vue importTable.vue:26-29 的 @row-click) -->
              <tr
                class="hover:bg-muted/50 cursor-pointer border-b last:border-0"
                onclick={() => toggle(name, !selected.has(name))}
              >
                <td class="p-2" onclick={(e: Event) => e.stopPropagation()}>
                  <Checkbox
                    checked={selected.has(name)}
                    onCheckedChange={(v) => toggle(name, !!v)} />
                </td>
                <td class="p-2">{name}</td>
                <td class="text-muted-foreground p-2">{t.tableComment ?? '-'}</td>
                <td class="text-muted-foreground p-2">{t.createTime ?? '-'}</td>
                <td class="text-muted-foreground p-2">{t.updateTime ?? '-'}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    <!-- 分页条 -->
    <div class="text-muted-foreground flex items-center justify-between text-sm">
      <span>共 {total} 张表，已选 {selected.size} 张</span>
      <div class="flex items-center gap-2">
        <Button size="sm" variant="outline" disabled={pageNum <= 1} onclick={() => goPage(pageNum - 1)}>
          上一页
        </Button>
        <span>{pageNum} / {totalPages}</span>
        <Button
          size="sm"
          variant="outline"
          disabled={pageNum >= totalPages}
          onclick={() => goPage(pageNum + 1)}
        >
          下一页
        </Button>
      </div>
    </div>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => onOpenChange(false)}>取消</Button>
      <Button disabled={saving} onclick={confirm}>确定导入</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
