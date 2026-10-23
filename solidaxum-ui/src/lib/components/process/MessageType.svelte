<script lang="ts" module>
  /**
   * 催办消息类型选择弹窗 —— 对齐 Vue components/Process/MessageType.vue。
   *
   * ⚠️ **后端现状**:
   * 后端 FlowUrgeTaskBo(dto.rs:708-713) 只有 taskId + message,**没有 messageType 字段**。
   * 三个 checkbox(站内信/邮件/短信)的值当前**不会被后端消费**——多发键被 serde 静默忽略
   * (未开 deny_unknown_fields)。UI 照 Vue 做以保持 parity,但别以为勾「邮件/短信」真的会发。
   * 消息内容进 message 字段(后端支持)。messageType 尚待后端支持。
   */
</script>

<script lang="ts">
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import * as Dialog from '$lib/components/ui/dialog/index.js'

  let {
    open = $bindable(false),
    title = '提示',
    loading = false,
    onConfirm,
    onCancel,
  }: {
    open?: boolean
    title?: string
    loading?: boolean
    onConfirm: (payload: { messageType: string[]; message: string }) => void
    onCancel?: () => void
  } = $props()

  // 站内信默认勾选(Vue 站内信 value='1' 且 disabled)。
  let messageType = $state<string[]>(['1'])
  let message = $state('')

  // 弹窗打开/关闭时重置表单(Vue open() 里 reset())。
  $effect(() => {
    if (open) {
      messageType = ['1']
      message = ''
    }
  })

  // Vue rules: message 必填。空则禁用确认按钮。
  let canSubmit = $derived(message.trim().length > 0)

  function handleConfirm() {
    if (!canSubmit || loading) return
    onConfirm({ messageType: [...messageType], message: message.trim() })
  }

  function handleCancel() {
    onCancel?.()
    open = false
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-[560px]">
    <Dialog.Header>
      <Dialog.Title>{title}</Dialog.Title>
    </Dialog.Header>
    <div class="space-y-4 py-2">
      <div class="flex flex-col gap-2">
        <Label>消息提醒</Label>
        <div class="flex items-center gap-4">
          <div class="flex items-center gap-2">
            <Checkbox
              id="mt-site"
              checked={messageType.includes('1')}
              disabled
              onCheckedChange={(v) => {
                if (v) messageType = [...new Set([...messageType, '1'])]
              }} />
            <Label for="mt-site" class="cursor-default font-normal text-muted-foreground">站内信</Label>
          </div>
          <div class="flex items-center gap-2">
            <Checkbox
              id="mt-mail"
              checked={messageType.includes('2')}
              onCheckedChange={(v) => {
                messageType = v
                  ? [...new Set([...messageType, '2'])]
                  : messageType.filter((x) => x !== '2')
              }} />
            <Label for="mt-mail" class="cursor-pointer font-normal">邮件</Label>
          </div>
          <div class="flex items-center gap-2">
            <Checkbox
              id="mt-sms"
              checked={messageType.includes('3')}
              onCheckedChange={(v) => {
                messageType = v
                  ? [...new Set([...messageType, '3'])]
                  : messageType.filter((x) => x !== '3')
              }} />
            <Label for="mt-sms" class="cursor-pointer font-normal">短信</Label>
          </div>
        </div>
        <p class="text-muted-foreground text-xs">站内信默认开启;邮件/短信需后端支持(当前未消费)。</p>
      </div>
      <div class="flex flex-col gap-2">
        <Label for="mt-message">消息内容</Label>
        <Textarea
          id="mt-message"
          bind:value={message}
          placeholder="请输入消息内容"
          rows={3} />
      </div>
    </div>
    <Dialog.Footer>
      <Button variant="outline" onclick={handleCancel} disabled={loading}>取消</Button>
      <Button onclick={handleConfirm} disabled={!canSubmit || loading}>
        {loading ? '发送中...' : '确认'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
