<script lang="ts">
  import { onMount } from 'svelte'
  import {
    buildChannelSaveBody,
    messageChannelApi,
    type ChannelRow,
  } from '$lib/api/system/message-channel.js'
  import HasPermi from '$lib/components/HasPermi.svelte'
  import Main from '$lib/components/layout/Main.svelte'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import {
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
  } from '$lib/components/ui/card/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { toast } from 'svelte-sonner'

  type EditableChannel = ChannelRow & { remark: string }

  let rows = $state<EditableChannel[]>([])
  let loading = $state(true)
  let saving = $state<string | null>(null)
  let testing = $state<string | null>(null)

  async function load() {
    loading = true
    try {
      const resp = await messageChannelApi.list()
      rows = (resp.data ?? []).map((r) => ({
        ...r,
        // 密钥回读是掩码,编辑表单留空表示不改;真实值不由前端回填。
        clientSecret: '',
        agentSecret: '',
        remark: r.remark ?? '',
      }))
    } catch {
      // apiClient 已 toast
    } finally {
      loading = false
    }
  }

  onMount(load)

  function sourceLabel(source: ChannelRow['source']) {
    if (source === 'db') return '数据库配置'
    if (source === 'env') return '环境变量'
    return '未配置'
  }

  async function save(row: EditableChannel) {
    saving = row.channel
    try {
      await messageChannelApi.save(row.channel, buildChannelSaveBody(row))
      toast.success(`${row.channelName} 配置已保存`)
      await load()
    } catch {
      // apiClient 已 toast
    } finally {
      saving = null
    }
  }

  async function test(row: EditableChannel) {
    testing = row.channel
    try {
      const resp = await messageChannelApi.test(row.channel)
      toast.success(resp.data?.message ?? '连接成功')
    } catch {
      // apiClient 已 toast(未配置 400 含中文原因)
    } finally {
      testing = null
    }
  }
</script>

<Main class="flex flex-1 flex-col gap-4 sm:gap-6">
  <div class="flex flex-col gap-1">
    <div class="text-lg font-semibold tracking-tight">消息渠道</div>
    <p class="text-muted-foreground text-sm">
      钉钉 / 企业微信 /
      飞书企业应用配置。密钥仅以密文落库，页面永远只显示掩码；留空表示保留旧值。
    </p>
  </div>

  {#if loading}
    <p class="text-muted-foreground text-sm">加载中…</p>
  {:else if rows.length === 0}
    <p class="text-muted-foreground text-sm">暂无可配置渠道。</p>
  {:else}
    <div class="grid gap-4 lg:grid-cols-3">
      {#each rows as row (row.channel)}
        <Card>
          <CardHeader>
            <div class="flex items-start justify-between gap-2">
              <div class="flex flex-col gap-1">
                <CardTitle>{row.channelName}</CardTitle>
                <CardDescription>{row.channel}</CardDescription>
              </div>
              <div class="flex flex-wrap items-center gap-1">
                <Badge variant={row.configured ? 'default' : 'outline'}>
                  {row.configured ? '已配置' : '未配置'}
                </Badge>
                <Badge variant="secondary">{sourceLabel(row.source)}</Badge>
              </div>
            </div>
          </CardHeader>

          <CardContent class="flex flex-col gap-3">
            <label class="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                class="size-4 rounded border"
                bind:checked={row.enabled}
                disabled={saving === row.channel}
              />
              启用该渠道
            </label>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-clientId`}>ClientId</Label>
              <Input
                id={`${row.channel}-clientId`}
                bind:value={row.clientId}
                placeholder="AppKey / CorpId / AppId"
              />
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-clientSecret`}>ClientSecret</Label>
              <Input
                id={`${row.channel}-clientSecret`}
                type="password"
                bind:value={row.clientSecret}
                placeholder={row.configured
                  ? '已配置，留空保留原值'
                  : '请输入 ClientSecret'}
              />
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-agentId`}>AgentId</Label>
              <Input
                id={`${row.channel}-agentId`}
                bind:value={row.agentId}
                placeholder="企业微信应用 AgentId（企微必填）"
              />
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-agentSecret`}>AgentSecret</Label>
              <Input
                id={`${row.channel}-agentSecret`}
                type="password"
                bind:value={row.agentSecret}
                placeholder="可选，留空保留原值"
              />
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-baseUrl`}>API 基址</Label>
              <Input
                id={`${row.channel}-baseUrl`}
                bind:value={row.baseUrl}
                placeholder="留空使用公有云地址"
              />
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-domain`}>域名</Label>
              <Input
                id={`${row.channel}-domain`}
                bind:value={row.domain}
                placeholder="可选"
              />
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for={`${row.channel}-remark`}>备注</Label>
              <Textarea
                id={`${row.channel}-remark`}
                bind:value={row.remark}
                rows={2}
                placeholder="可选"
              />
            </div>
          </CardContent>

          <CardFooter class="flex flex-wrap gap-2">
            <HasPermi perm="system:messageChannel:edit">
              <Button
                size="sm"
                onclick={() => save(row)}
                disabled={saving === row.channel}
              >
                {saving === row.channel ? '保存中…' : '保存'}
              </Button>
            </HasPermi>
            <HasPermi perm="system:messageChannel:test">
              <Button
                size="sm"
                variant="outline"
                onclick={() => test(row)}
                disabled={testing === row.channel}
              >
                {testing === row.channel ? '测试中…' : '连接测试'}
              </Button>
            </HasPermi>
          </CardFooter>
        </Card>
      {/each}
    </div>
  {/if}
</Main>
