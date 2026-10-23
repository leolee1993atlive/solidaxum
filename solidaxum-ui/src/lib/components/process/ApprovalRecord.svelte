<script lang="ts">
  import { instanceApi } from '$lib/api/workflow/instance.js'
  import type { FlowHisTaskVo } from '$lib/api/workflow/instance.js'
  import FlowChart from '$lib/components/flow-designer/FlowChart.svelte'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { toast } from 'svelte-sonner'

  // 对齐 Vue approvalRecord.vue:两个 tab,默认落在流程图;后端返回 {list, instanceId}
  let { businessId }: { businessId: string } = $props()

  let records = $state<FlowHisTaskVo[]>([])
  let instanceId = $state<string | null>(null)
  let loading = $state(true)

  $effect(() => {
    void (async () => {
      loading = true
      try {
        const resp = await instanceApi.flowHisTaskList(businessId)
        records = resp.data?.list ?? []
        instanceId = resp.data?.instanceId ?? null
      } catch (e) {
        toast.error(e instanceof Error ? e.message : '加载审批记录失败')
      } finally {
        loading = false
      }
    })()
  })
</script>

{#if loading}
  <div class="text-muted-foreground py-6 text-center text-sm">加载中...</div>
{:else}
  <Tabs.Root value={instanceId ? 'chart' : 'records'}>
    <Tabs.List>
      <Tabs.Trigger value="chart" disabled={!instanceId}>流程图</Tabs.Trigger>
      <Tabs.Trigger value="records">审批记录</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="chart">
      {#if instanceId}
        <div class="h-[60vh] overflow-hidden rounded-md border">
          <FlowChart {instanceId} />
        </div>
      {/if}
    </Tabs.Content>
    <Tabs.Content value="records">
      {#if records.length === 0}
        <div class="text-muted-foreground py-6 text-center text-sm">
          暂无审批记录
        </div>
      {:else}
        <ol class="relative ml-3 border-l pl-6">
          {#each records as r (r.id)}
            <li class="mb-5">
              <span
                class="bg-primary absolute -left-1.5 mt-1.5 h-3 w-3 rounded-full"
              ></span>
              <div class="text-sm font-medium">{r.nodeName ?? '-'}</div>
              <div class="text-muted-foreground text-xs">
                {r.approver ?? '-'} · {r.createTime ?? '-'}
              </div>
              {#if r.message}
                <div class="bg-muted mt-1 rounded p-2 text-sm">{r.message}</div>
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
    </Tabs.Content>
  </Tabs.Root>
{/if}
