<script lang="ts">
  import { toast } from 'svelte-sonner'

  import { taskApi } from '$lib/api/workflow/task.js'
  import type { FlowNextNodeVo } from '$lib/api/workflow/task.js'
  import UserPicker from './UserPicker.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'

  let {
    open = $bindable(false),
    taskId,
    onDone,
  }: { open?: boolean; taskId: string; onDone: () => void } = $props()

  let message = $state('')
  let copyUserIds = $state<string[]>([])
  let nextNodes = $state<FlowNextNodeVo[]>([])
  let submitting = $state(false)

  // 打开时拉一次下一节点(关闭再开要重拉,节点可能随变量变化)
  $effect(() => {
    if (!open) return
    void (async () => {
      try {
        const resp = await taskApi.getNextNodeList(taskId)
        nextNodes = (resp.data ?? resp) as FlowNextNodeVo[]
      } catch {
        nextNodes = []
      }
    })()
  })

  async function submit() {
    submitting = true
    try {
      await taskApi.complete({
        taskId,
        message: message || undefined,
        flowCopyList: copyUserIds.map((id) => ({ userId: id })),
      })
      toast.success('提交成功')
      open = false
      message = ''
      copyUserIds = []
      onDone()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '提交失败')
    } finally {
      submitting = false
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-[560px]">
    <Dialog.Header><Dialog.Title>提交</Dialog.Title></Dialog.Header>
    <div class="grid gap-4 py-2">
      <div class="grid gap-2">
        <Label>审批意见</Label>
        <Textarea bind:value={message} rows={3} placeholder="请输入审批意见" />
      </div>
      <div class="grid gap-2">
        <Label>抄送人</Label>
        <UserPicker value={copyUserIds} onChange={(next) => { copyUserIds = next }} placeholder="可选，抄送给指定人员" />
      </div>
      <div class="grid gap-2">
        <Label>下一节点</Label>
        {#if nextNodes.length === 0}
          <span class="text-muted-foreground text-sm">无（提交后流程结束或由引擎自动流转）</span>
        {:else}
          <ul class="text-sm">
            {#each nextNodes as n (n.nodeCode)}
              <li>· {n.nodeName}（{n.nodeCode}）</li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (open = false)}>取消</Button>
      <Button disabled={submitting} onclick={submit}>确定提交</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
