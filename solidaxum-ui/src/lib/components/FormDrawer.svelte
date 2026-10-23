<script lang="ts">
  import type { Snippet } from 'svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Sheet from '$lib/components/ui/sheet/index.js'
  import { cn } from '$lib/utils.js'
  import { LoaderCircle } from '@lucide/svelte'

  interface Props {
    open: boolean
    onOpenChange: (open: boolean) => void
    title: string
    /** 标题下的辅助说明(可选) */
    description?: string
    loading?: boolean
    onConfirm?: () => void | Promise<void>
    confirmText?: string
    cancelText?: string
    /** 宽度档位(风格平价 §A 规格):xl=标准单列表单(默认),2xl=双列栅格 CRUD,4xl=只读详情/时间线。 */
    size?: 'xl' | '2xl' | '4xl'
    /** 只读详情变体:隐藏保存钮,取消钮文案变「关闭」。 */
    readonly?: boolean
    /** 覆盖宽度等 Sheet.Content 类(size 之外的非常规覆盖逃生口)。 */
    class?: string
    children?: Snippet
  }

  let {
    open,
    onOpenChange,
    title,
    description,
    loading = false,
    onConfirm,
    confirmText = '保存',
    cancelText = '取消',
    size = 'xl',
    readonly = false,
    class: className,
    children,
  }: Props = $props()

  const SIZE_CLASS = {
    xl: 'sm:max-w-xl',
    '2xl': 'sm:max-w-2xl',
    '4xl': 'sm:max-w-4xl',
  } as const
</script>

<Sheet.Root {open} {onOpenChange}>
  <Sheet.Content class={cn('flex flex-col', SIZE_CLASS[size], className)}>
    <Sheet.Header class="text-start">
      <Sheet.Title>{title}</Sheet.Title>
      {#if description}
        <Sheet.Description>{description}</Sheet.Description>
      {/if}
    </Sheet.Header>
    <div class="flex-1 space-y-4 overflow-y-auto px-4 py-4">
      {@render children?.()}
    </div>
    <Sheet.Footer class="flex-row justify-end gap-2 border-t px-4 py-3">
      <Button
        variant="outline"
        onclick={() => onOpenChange(false)}
        disabled={loading}
      >
        {readonly ? '关闭' : cancelText}
      </Button>
      {#if !readonly}
        <Button onclick={onConfirm} disabled={loading}>
          {#if loading}
            <LoaderCircle class="size-4 animate-spin" />
          {/if}
          {loading ? '提交中...' : confirmText}
        </Button>
      {/if}
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>
