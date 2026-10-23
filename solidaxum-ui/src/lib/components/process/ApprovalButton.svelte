<script lang="ts">
  import { toast } from 'svelte-sonner'

  import { taskApi, TASK_OPS, buildOperationBody } from '$lib/api/workflow/task.js'
  import UserPicker from './UserPicker.svelte'
  import SubmitVerify from './SubmitVerify.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'

  let { taskId, onDone }: { taskId: string; onDone: () => void } = $props()

  let submitOpen = $state(false)
  /** 'back' 退回 / 'termination' 终止 / 'transfer' 转办 / 'delegate' 委托 */
  let action = $state<null | 'back' | 'termination' | 'transfer' | 'delegate'>(null)
  let comment = $state('')
  let userIds = $state<string[]>([])
  let busy = $state(false)
  /** 可退回节点(getBackTaskNode),退回时必选其一,nodeCode 随 backProcess 提交。 */
  let backNodes = $state<{ nodeCode: string; nodeName: string }[]>([])
  let backNodeCode = $state('')
  let backNodesLoading = $state(false)

  const NEEDS_USER = new Set(['transfer', 'delegate'])

  async function openBack() {
    action = 'back'
    backNodes = []
    backNodeCode = ''
    backNodesLoading = true
    try {
      const res = await taskApi.getById(taskId)
      const nodeCode = res.data?.nodeCode
      if (!nodeCode) throw new Error('任务缺少节点编码')
      const nodesRes = await taskApi.getBackTaskNode(taskId, nodeCode)
      const nodes = nodesRes.data ?? []
      backNodes = nodes
      // 唯一候选直接预选,减少一步操作
      if (nodes.length === 1) backNodeCode = nodes[0].nodeCode
      if (nodes.length === 0) toast.error('没有可退回的节点')
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '可退回节点加载失败')
    } finally {
      backNodesLoading = false
    }
  }

  async function confirm() {
    if (!action) return
    if (NEEDS_USER.has(action) && userIds.length === 0) {
      toast.error('请选择人员')
      return
    }
    if (action === 'back' && !backNodeCode) {
      toast.error('请选择退回节点')
      return
    }
    busy = true
    try {
      if (action === 'back') {
        await taskApi.backProcess({
          taskId,
          nodeCode: backNodeCode,
          message: comment || undefined,
        })
      } else if (action === 'termination') {
        await taskApi.termination(taskId, comment || undefined)
      } else {
        const op = action === 'transfer' ? TASK_OPS.transfer : TASK_OPS.delegate
        await taskApi.operation(op, buildOperationBody(taskId, userIds, comment))
      }
      toast.success('操作成功')
      action = null
      comment = ''
      userIds = []
      backNodes = []
      backNodeCode = ''
      onDone()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '操作失败')
    } finally {
      busy = false
    }
  }
</script>

<div class="flex flex-wrap gap-2">
  <Button onclick={() => (submitOpen = true)}>同意</Button>
  <Button variant="secondary" onclick={openBack}>退回</Button>
  <Button variant="secondary" onclick={() => (action = 'transfer')}>转办</Button>
  <Button variant="secondary" onclick={() => (action = 'delegate')}>委托</Button>
  <Button variant="destructive" onclick={() => (action = 'termination')}>终止</Button>
</div>

<SubmitVerify bind:open={submitOpen} {taskId} {onDone} />

<Dialog.Root open={action !== null} onOpenChange={(v) => { if (!v) action = null }}>
  <Dialog.Content class="sm:max-w-[480px]">
    <Dialog.Header>
      <Dialog.Title>
        {action === 'back' ? '退回' : action === 'termination' ? '终止' : action === 'transfer' ? '转办' : '委托'}
      </Dialog.Title>
    </Dialog.Header>
    <div class="grid gap-4 py-2">
      {#if action === 'back'}
        <div class="grid gap-2">
          <Label>退回节点</Label>
          {#if backNodesLoading}
            <p class="text-muted-foreground text-sm">加载可退回节点…</p>
          {:else}
            <select
              class="rounded border px-2 py-1"
              bind:value={backNodeCode}
              disabled={backNodes.length === 0}>
              <option value="" disabled>请选择退回节点</option>
              {#each backNodes as n (n.nodeCode)}
                <option value={n.nodeCode}>{n.nodeName || n.nodeCode}</option>
              {/each}
            </select>
          {/if}
        </div>
      {/if}
      {#if action && NEEDS_USER.has(action)}
        <div class="grid gap-2">
          <Label>选择人员</Label>
          <UserPicker value={userIds} onChange={(next) => { userIds = next }} multiple={false} />
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
