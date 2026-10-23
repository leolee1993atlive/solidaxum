<script lang="ts">
  import type { Snippet } from 'svelte'
  import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { cn } from '$lib/utils.js'
  import { LoaderCircle } from '@lucide/svelte'

  interface Props {
    open: boolean
    onOpenChange: (open: boolean) => void
    title: string | Snippet
    desc: string | Snippet
    cancelBtnText?: string
    confirmText?: string | Snippet
    destructive?: boolean
    handleConfirm: () => void
    isLoading?: boolean
    disabled?: boolean
    class?: string
    children?: Snippet
  }

  let {
    open,
    onOpenChange,
    title,
    desc,
    cancelBtnText = '取消',
    confirmText = '继续',
    destructive = false,
    handleConfirm,
    isLoading = false,
    disabled = false,
    class: className,
    children,
  }: Props = $props()
</script>

<AlertDialog.Root {open} {onOpenChange}>
  <AlertDialog.Content class={cn(className)}>
    <AlertDialog.Header class="text-start">
      <AlertDialog.Title>
        {#if typeof title === 'string'}
          {title}
        {:else}
          {@render title()}
        {/if}
      </AlertDialog.Title>
      <AlertDialog.Description>
        {#if typeof desc === 'string'}
          {desc}
        {:else}
          {@render desc()}
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>

    {#if children}
      {@render children()}
    {/if}

    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={isLoading}>
        {cancelBtnText}
      </AlertDialog.Cancel>
      <Button
        variant={destructive ? 'destructive' : 'default'}
        onclick={handleConfirm}
        disabled={disabled || isLoading}
      >
        {#if isLoading}
          <LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
        {/if}
        {#if typeof confirmText === 'string'}
          {confirmText}
        {:else}
          {@render confirmText()}
        {/if}
      </Button>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
