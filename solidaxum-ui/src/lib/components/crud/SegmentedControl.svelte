<script lang="ts">
  import { cn } from '$lib/utils.js'

  /**
   * 分段单选控件(对齐旧版 antd RadioGroup button 实心钮:
   * 列表页「全部/待提交/返回重报」类状态过滤、查询行内分段)。
   * 键盘可达:原生 button 组,选中态 aria-pressed。
   */
  export type SegmentedOption = { label: string; value: string }

  let {
    options,
    value,
    onchange,
    size = 'default',
  }: {
    options: SegmentedOption[]
    value: string
    onchange?: (value: string) => void
    size?: 'sm' | 'default'
  } = $props()

  function select(option: SegmentedOption) {
    if (option.value === value) return
    onchange?.(option.value)
  }
</script>

<div
  role="group"
  data-slot="segmented-control"
  class="border-input bg-muted/40 inline-flex items-center rounded-md border p-0.5"
>
  {#each options as option (option.value)}
    <button
      type="button"
      aria-pressed={option.value === value}
      class={cn(
        'rounded-[5px] px-3 font-medium whitespace-nowrap transition-colors',
        size === 'sm' ? 'h-7 text-xs' : 'h-8 text-sm',
        option.value === value
          ? 'bg-primary text-primary-foreground shadow-sm'
          : 'text-muted-foreground hover:text-foreground'
      )}
      onclick={() => select(option)}
    >
      {option.label}
    </button>
  {/each}
</div>
