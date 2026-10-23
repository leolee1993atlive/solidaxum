<script lang="ts">
  import { isApiClientNotifiedError } from '$lib/api/client.js'
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte'
  import type { ValidationIssue } from '$lib/flow/validate.js'
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import { toast } from 'svelte-sonner'
  import Canvas from './Canvas.svelte'
  import Palette from './Palette.svelte'
  import Toolbar from './Toolbar.svelte'
  import PropertyPanel from './panel/PropertyPanel.svelte'
  import { setDesignerContext } from './state/context.js'
  import { DesignerStore } from './state/designer.svelte.js'

  let {
    definitionId,
    readonly = false,
    onClose,
  }: {
    definitionId: string
    readonly?: boolean
    onClose: () => void
  } = $props()

  const store = new DesignerStore()
  // 画布、边、面板、编辑层都从 context 取 store(state/context.ts)
  setDesignerContext(store)
  let confirmClose = $state(false)
  let warnings = $state<ValidationIssue[]>([])

  $effect(() => {
    // apiClient 已经 toast 过的错误不重复提示
    store.load(definitionId).catch((e) => {
      if (!isApiClientNotifiedError(e))
        toast.error(errorText(e, '加载流程定义失败'))
    })
  })

  function errorText(e: unknown, fallback: string) {
    return e instanceof Error && e.message ? e.message : fallback
  }

  /** 定位到第一条问题对应的元素,让用户能直接改 */
  function focusIssue(issue: ValidationIssue) {
    if (!issue.target.id) return
    if (issue.target.kind === 'node') store.selectNode(issue.target.id)
    else store.selectEdge(issue.target.id)
  }

  async function doSave() {
    try {
      await store.save()
      toast.success('保存成功')
    } catch (e) {
      if (!isApiClientNotifiedError(e)) toast.error(errorText(e, '保存失败'))
    }
  }

  /** error 拦住不存;warning 列出来让用户决定——老设计器允许保存半成品,这里也一样 */
  function save() {
    const issues = store.validate()
    const errors = issues.filter((i) => i.level === 'error')
    if (errors.length) {
      toast.error(errors[0].message)
      focusIssue(errors[0])
      return
    }
    if (issues.length) {
      warnings = issues
      return
    }
    void doSave()
  }

  function requestClose() {
    if (!readonly && store.isDirty()) {
      confirmClose = true
      return
    }
    onClose()
  }
</script>

<SvelteFlowProvider>
  <div class="flex h-full flex-col">
    <Toolbar {store} {readonly} onSave={save} onClose={requestClose} />
    <div class="flex min-h-0 flex-1">
      {#if !readonly}
        <Palette {store} />
      {/if}
      <div class="relative min-w-0 flex-1">
        {#if store.loading}
          <div
            class="text-muted-foreground absolute inset-0 z-10 flex items-center justify-center text-sm"
          >
            加载中…
          </div>
        {/if}
        <Canvas bind:nodes={store.nodes} bind:edges={store.edges} {readonly} />
      </div>
      <PropertyPanel {store} {readonly} />
    </div>
  </div>
</SvelteFlowProvider>

<ConfirmDialog
  open={confirmClose}
  onOpenChange={(v) => (confirmClose = v)}
  title="放弃未保存的修改?"
  desc="流程图有改动尚未保存,关闭后将丢失。"
  confirmText="放弃并关闭"
  destructive
  handleConfirm={() => {
    confirmClose = false
    onClose()
  }}
/>

<ConfirmDialog
  open={warnings.length > 0}
  onOpenChange={(v) => {
    if (!v) warnings = []
  }}
  title="流程有 {warnings.length} 处问题,仍要保存?"
  confirmText="仍然保存"
  handleConfirm={() => {
    warnings = []
    void doSave()
  }}
>
  {#snippet desc()}
    <ul class="max-h-60 list-disc space-y-1 overflow-auto pl-5 text-left">
      {#each warnings as w, i (i)}
        <li>
          <button
            type="button"
            class="text-left hover:underline"
            onclick={() => focusIssue(w)}
          >
            {w.message}
          </button>
        </li>
      {/each}
    </ul>
  {/snippet}
</ConfirmDialog>
