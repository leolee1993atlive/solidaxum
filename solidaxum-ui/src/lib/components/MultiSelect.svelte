<script lang="ts">
  import * as Popover from '$lib/components/ui/popover/index.js'
  import * as ScrollArea from '$lib/components/ui/scroll-area/index.js'
  import { Checkbox } from '$lib/components/ui/checkbox/index.js'
  import { cn } from '$lib/utils.js'
  import { ChevronDown } from '@lucide/svelte'

  // 组件内部类型:不 export(Svelte 实例 script 不做类型导出),调用方按结构字面量传即可。
  type MultiSelectOption = {
    label: string
    value: string
    /** 停用项:可见但不可选(对齐 Vue :disabled="item.status == '1'") */
    disabled?: boolean
  }

  let {
    options = [],
    values = [],
    placeholder = '请选择',
    searchPlaceholder = '搜索',
    disabled = false,
    onChange,
  }: {
    options?: MultiSelectOption[]
    values?: string[]
    placeholder?: string
    searchPlaceholder?: string
    disabled?: boolean
    onChange?: (next: string[]) => void
  } = $props()

  let open = $state(false)
  let keyword = $state('')

  const selectedLabels = $derived(
    values
      .map((v) => options.find((o) => o.value === v)?.label)
      .filter((l): l is string => !!l)
      .join(', ')
  )

  const visibleOptions = $derived(
    keyword.trim() === ''
      ? options
      : options.filter((o) => o.label.includes(keyword.trim()))
  )

  function toggle(option: MultiSelectOption) {
    if (option.disabled) return
    const next = values.includes(option.value)
      ? values.filter((v) => v !== option.value)
      : [...values, option.value]
    onChange?.(next)
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger {disabled}>
    {#snippet child({ props })}
      <button
        type="button"
        {...props}
        class={cn(
          'border-input ring-offset-background focus-visible:ring-ring flex h-9 w-full items-center justify-between rounded-md border bg-transparent px-3 py-2 text-sm shadow-sm transition-colors focus-visible:ring-1 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
          values.length === 0 && 'text-muted-foreground'
        )}
      >
        <span class="truncate">{selectedLabels || placeholder}</span>
        <ChevronDown class="h-4 w-4 opacity-50" />
      </button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content class="w-72 p-0" align="start">
    <div class="border-b p-2">
      <input
        class="border-input h-8 w-full rounded-md border px-2 text-sm"
        placeholder={searchPlaceholder}
        bind:value={keyword}
      />
    </div>
    <ScrollArea.Root class="h-56 w-full">
      <div class="p-2">
        {#if visibleOptions.length === 0}
          <div class="text-muted-foreground px-2 py-1.5 text-sm">暂无数据</div>
        {/if}
        {#each visibleOptions as option (option.value)}
          <!-- 用 label 包 Checkbox: getByLabelText 能取到,点击 label 即切换 -->
          <label
            class={cn(
              'hover:bg-accent flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm',
              option.disabled && 'cursor-not-allowed opacity-50'
            )}
          >
            <Checkbox
              aria-label={option.label}
              checked={values.includes(option.value)}
              disabled={option.disabled}
              onCheckedChange={() => toggle(option)}
            />
            <span class="truncate">{option.label}</span>
          </label>
        {/each}
      </div>
    </ScrollArea.Root>
  </Popover.Content>
</Popover.Root>
