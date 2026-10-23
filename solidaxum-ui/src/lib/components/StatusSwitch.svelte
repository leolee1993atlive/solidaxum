<script lang="ts">
  import { cn } from '$lib/utils.js'

  let {
    value,
    checkedValue,
    uncheckedValue,
    onChange,
    disabled = false,
    label = '状态',
  }: {
    /** 当前值(后端原码,如 '0'/'1' 或 'Y'/'N')。 */
    value: string | undefined | null
    /** 视为「开」的值。 */
    checkedValue: string
    /** 视为「关」的值。 */
    uncheckedValue: string
    /** 切换回调。抛错即视为失败,组件自动回滚视觉状态。 */
    onChange: (next: string) => void | Promise<void>
    disabled?: boolean
    label?: string
  } = $props()

  // 乐观视觉状态。null = 跟随 props.value。
  let optimistic = $state<string | null>(null)
  let pending = $state(false)

  // 上一次从 props 看到的值。普通变量而非 $state:只在 effect 内读写,
  // 不需要它自身触发重算(否则 effect 会自我依赖)。初值留 undefined,
  // 由 effect 的首次运行播种 —— 不在这里写 `= value`,那样会在闭包外读响应式
  // prop,触发 svelte 的 state_referenced_locally 告警。
  let lastSeen: string | null | undefined

  // 服务端送来**任何**新值(列表 refetch 落地)即丢弃乐观值,交还 props 的话语权。
  //
  // 为什么必须清:乐观值一旦长期挂着,就会永久遮蔽 props.value。oss-config 的
  // 「唯一默认桶」是典型受害场景 —— 把 A 设为默认(A 挂上乐观值 'Y')后再把 B
  // 设为默认,后端会把 A 置回 'N',但 A 这一格仍显示打开,界面上出现两个默认桶,
  // 正好破坏该页要保证的唯一性。
  //
  // 正常路径不会闪:refetch 后 props.value 恰好等于乐观值,清掉后渲染结果不变。
  // 首次运行时 lastSeen 为 undefined、optimistic 必为 null,清一次是空操作。
  $effect(() => {
    const v = value
    if (v !== lastSeen) {
      lastSeen = v
      optimistic = null
    }
  })

  let current = $derived(optimistic ?? value ?? uncheckedValue)
  let checked = $derived(current === checkedValue)

  async function toggle() {
    if (disabled || pending) return
    const next = checked ? uncheckedValue : checkedValue
    optimistic = next
    pending = true
    try {
      await onChange(next)
      // 成功: 暂留乐观值,等 refetch 把 props.value 送来后由上面的 $effect 清掉。
    } catch {
      optimistic = null // 回滚
    } finally {
      pending = false
    }
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={toggle}
  class={cn(
    'focus-visible:ring-ring/50 inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent transition-colors outline-none focus-visible:ring-[3px]',
    checked ? 'bg-primary' : 'bg-input',
    (disabled || pending) && 'cursor-not-allowed opacity-50'
  )}
>
  <span
    class={cn(
      'bg-background pointer-events-none block size-4 rounded-full shadow transition-transform',
      checked ? 'translate-x-4' : 'translate-x-0.5'
    )}
  ></span>
</button>
