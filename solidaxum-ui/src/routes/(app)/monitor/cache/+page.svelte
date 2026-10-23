<script lang="ts">
  import { onMount } from 'svelte'

  import { cacheApi, commandStatsSeries, parseMemoryBytes, cacheOverview } from '$lib/api/monitor/cache.js'
  import type { CacheInfo } from '$lib/api/monitor/cache.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import CommandRoseChart from '$lib/components/charts/CommandRoseChart.svelte'
  import MemoryGaugeChart from '$lib/components/charts/MemoryGaugeChart.svelte'

  // ---- detail state ----
  // 缓存监控是详情页(非列表),加载一次单对象即可。
  let data = $state<CacheInfo | null>(null)
  let loading = $state(false)

  async function load() {
    loading = true
    try {
      const resp = await cacheApi.get()
      data = resp.data ?? null
    } catch {
      // apiClient 已经 toast 错误,这里不重复提示
    } finally {
      loading = false
    }
  }

  onMount(() => {
    load()
  })

  // CA2:全量交给玫瑰图,不再截断(此前 TOP_N=10 会丢命令)。
  let commandSeries = $derived(commandStatsSeries(data?.commandStats ?? []))

  // CA3:内存信息面板。
  // ⚠️ 有意偏离 Vue:Vue 用 parseFloat(used_memory_human) 配 max=1000
  //    ("1.05M" → 1.05,指针永远贴零)。改用真实字节数,
  //    上限取 maxmemory(未配置时为 "0"),回落 used_memory_peak。
  //    中央文本仍是 used_memory_human,用户看到的数字与 Vue 一致。
  let memUsed = $derived(parseMemoryBytes(data?.info?.used_memory))
  let memMax = $derived(
    parseMemoryBytes(data?.info?.maxmemory) ||
      parseMemoryBytes(data?.info?.used_memory_peak),
  )
  let memLabel = $derived(data?.info?.used_memory_human ?? '-')

  // CA4:12 项精选字段 + 中文标签 + 值转换(对齐 Vue index.vue:15-108)。
  let overview = $derived(cacheOverview(data))
</script>

<div class="space-y-4 p-4 md:p-6">
  {#if loading}
    <Card.Root>
      <Card.Content class="text-muted-foreground py-8 text-center">加载中...</Card.Content>
    </Card.Root>
  {:else if data}
    <!-- 缓存概览 (CA4) -->
    <Card.Root>
      <Card.Header>
        <Card.Title>缓存概览</Card.Title>
      </Card.Header>
      <Card.Content>
        <dl class="grid gap-x-8 gap-y-2 sm:grid-cols-2 lg:grid-cols-4">
          {#each overview as item (item.label)}
            <div class="flex justify-between border-b py-1.5 text-sm">
              <dt class="text-muted-foreground">{item.label}</dt>
              <dd class="font-medium">{item.value}</dd>
            </div>
          {/each}
        </dl>
      </Card.Content>
    </Card.Root>

    <div class="grid gap-4 lg:grid-cols-2">
      <!-- 命令统计 (CA2:ECharts 玫瑰图,全量不截断) -->
      <Card.Root>
        <Card.Header>
          <Card.Title>命令统计</Card.Title>
        </Card.Header>
        <Card.Content>
          {#if commandSeries.length > 0}
            <CommandRoseChart data={commandSeries} />
          {:else}
            <p class="text-muted-foreground py-6 text-center">暂无数据</p>
          {/if}
        </Card.Content>
      </Card.Root>

      <!-- 内存信息 (CA3) -->
      <Card.Root>
        <Card.Header>
          <Card.Title>内存信息</Card.Title>
        </Card.Header>
        <Card.Content>
          {#if memUsed > 0}
            <MemoryGaugeChart used={memUsed} max={memMax} label={memLabel} />
          {:else}
            <p class="text-muted-foreground py-6 text-center">暂无数据</p>
          {/if}
        </Card.Content>
      </Card.Root>
    </div>

  {:else}
    <Card.Root>
      <Card.Content class="text-muted-foreground py-8 text-center">暂无数据</Card.Content>
    </Card.Root>
  {/if}
</div>
