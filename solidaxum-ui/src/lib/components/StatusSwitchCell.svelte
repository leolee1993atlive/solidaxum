<script lang="ts">
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import StatusSwitch from '$lib/components/StatusSwitch.svelte'
  import { toast } from 'svelte-sonner'

  let {
    value,
    checkedValue,
    uncheckedValue,
    confirmText,
    onConfirm,
    onDone,
    successMessage = '操作成功',
    disabled = false,
    label = '状态',
  }: {
    value: string | undefined | null
    checkedValue: string
    uncheckedValue: string
    /** 确认框正文。对齐 Vue 的 modal.confirm 文案。 */
    confirmText: string
    /** 真正的切换请求。抛错(reject)即视为失败;resolve 的 payload 被忽略。 */
    onConfirm: (next: string) => Promise<unknown>
    /** 成功后的收尾(通常是 crudTable 刷新)。 */
    onDone?: () => void
    successMessage?: string
    disabled?: boolean
    label?: string
  } = $props()

  let open = $state(false)
  let confirming = $state(false)
  let settle: { ok: () => void; fail: (e?: unknown) => void } | null = null

  // StatusSwitch 通过 Promise 感知用户在确认框里的选择：
  // 取消 → reject → 开关回滚；确认且请求成功 → resolve → 保持翻转。
  function ask(next: string): Promise<void> {
    return new Promise<void>((ok, fail) => {
      settle = { ok, fail }
      pending = next
      open = true
    })
  }

  let pending = $state<string | null>(null)

  // ⚠️ 请求完成前不关框:ConfirmDialog 收到 isLoading=true 后,
  // 确认/取消按钮都会 disabled(ConfirmDialog.svelte:63,69),
  // 拦住「请求进行中又点一次确定」的双击穿透。对齐 Vue modal 行为。
  async function confirm() {
    const next = pending
    if (next === null || !settle) return
    confirming = true
    try {
      await onConfirm(next)
      toast.success(successMessage)
      settle.ok()
      onDone?.()
      open = false
    } catch (e) {
      toast.error(e instanceof Error ? e.message : '操作失败')
      settle.fail(e)
      open = false
    } finally {
      settle = null
      pending = null
      confirming = false
    }
  }

  function dismiss() {
    if (confirming) return // 请求中不允许取消,避免半成品状态
    open = false
    settle?.fail(new Error('cancelled'))
    settle = null
    pending = null
  }
</script>

<StatusSwitch {value} {checkedValue} {uncheckedValue} {disabled} {label} onChange={ask} />

<ConfirmDialog
  {open}
  onOpenChange={(o) => { if (!o) dismiss() }}
  title="确认"
  desc={confirmText}
  confirmText="确定"
  isLoading={confirming}
  handleConfirm={confirm}
/>
