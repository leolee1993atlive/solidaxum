<script lang="ts">
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'

  // 所有节点共用的两个字段。节点编码是节点 id,改它要连带改边与引用,
  // 所以不做逐字符写回:失焦/回车时整体提交给 store.renameNodeCode。
  let {
    store,
    node,
    readonly = false,
    showName = true,
  }: {
    store: DesignerStore
    node: FlowNode
    readonly?: boolean
    showName?: boolean
  } = $props()

  // 面板按 {#key node.id} 重建,这里只需要初值;编码提交后 node.id 本身会变
  // svelte-ignore state_referenced_locally
  let codeDraft = $state(node.id)
  let codeError = $state<string | null>(null)

  function commitCode() {
    const error = store.renameNodeCode(node.id, codeDraft)
    codeError = error
    if (error) codeDraft = node.id
  }
</script>

<Field.Field>
  <Field.Label for="fd-node-code">节点编码</Field.Label>
  <Input
    id="fd-node-code"
    bind:value={codeDraft}
    disabled={readonly}
    onblur={commitCode}
    onkeydown={(e) => e.key === 'Enter' && commitCode()}
  />
  {#if codeError}<Field.Error>{codeError}</Field.Error>{/if}
</Field.Field>

{#if showName}
  <Field.Field>
    <Field.Label for="fd-node-name">节点名称</Field.Label>
    <Input
      id="fd-node-name"
      value={node.data.json.nodeName ?? ''}
      disabled={readonly}
      oninput={(e) =>
        store.updateNodeJson(node.id, { nodeName: e.currentTarget.value })}
    />
  </Field.Field>
{/if}
