<script lang="ts">
  import { toast } from 'svelte-sonner'

  import { taskApi, TASK_OPS, buildOperationBody } from '$lib/api/workflow/task.js'
  import UserPicker from './UserPicker.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'

  let { taskId, onDone }: { taskId: string; onDone: () => void } = $props()

  /** 对齐 Vue processMeddle.vue 的四项干预 */
  type Meddle = 'assignee' | 'transfer' | 'addSign' | 'termination'

  let action = $state<Meddle | null>(null)
  let userIds = $state<string[]>([])
  let comment = $state('')
  let busy = $state(false)

  const TITLES: Record<Meddle, string> = {
    assignee: '改派',
    transfer: '转办',
    addSign: '加签',
    termination: '终止',
  }
  const NEEDS_USER = new Set<Meddle>(['assignee', 'transfer', 'addSign'])

  async function confirm() {
    if (!action) return
    if (NEEDS_USER.has(action) && userIds.length === 0) {
      toast.error('请选择人员')
      return
    }
    busy = true
    try {
      if (action === 'assignee') {
        // 改派走独立端点:把任务的办理人换成指定用户
        await taskApi.updateAssignee(userIds[0], [taskId])
      } else if (action === 'transfer') {
        await taskApi.operation(TASK_OPS.transfer, buildOperationBody(taskId, userIds, comment))
      } else if (action === 'addSign') {
        await taskApi.operation(TASK_OPS.addSign, buildOperationBody(taskId, userIds, comment))
      } else {
        await taskApi.termination(taskId, comment || undefined)
      }
      toast.success('操作成功')
      action = null
      userIds = []
      comment = ''
      onDone()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '操作失败')
    } finally {
      busy = false
    }
  }
</script>

<div class="flex flex-wrap gap-1">
  {#each Object.keys(TITLES) as k (k)}
    <Button
      size="sm"
      variant={k === 'termination' ? 'destructive' : 'ghost'}
      onclick={() => (action = k as Meddle)}
    >
      {TITLES[k as Meddle]}
    </Button>
  {/each}
</div>

<Dialog.Root open={action !== null} onOpenChange={(v) => { if (!v) action = null }}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>{action ? TITLES[action] : ''}</Dialog.Title>
    </Dialog.Header>
    <div class="grid gap-4 py-2">
      {#if action && NEEDS_USER.has(action)}
        <div class="grid gap-2">
          <Label>{action === 'addSign' ? '加签人员（可多选）' : '选择人员'}</Label>
          <UserPicker value={userIds} onChange={(next) => { userIds = next }} multiple={action === 'addSign'} />
        </div>
      {/if}
      <div class="grid gap-2">
        <Label>意见</Label>
        <Textarea bind:value={comment} rows={3} />
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (action = null)}>取消</Button>
      <Button disabled={busy} onclick={confirm}>确定</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
