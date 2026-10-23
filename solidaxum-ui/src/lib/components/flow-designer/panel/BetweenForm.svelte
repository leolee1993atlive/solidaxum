<script lang="ts">
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import * as RadioGroup from '$lib/components/ui/radio-group/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import {
    formatListeners,
    parseListeners,
    previousNodes,
  } from '$lib/flow/node-fields.js'
  import {
    formatNodeRatio,
    parseNodeRatio,
    RATIO_TYPE_OPTIONS,
    ratioForWay,
    validateNodeRatio,
    type CollaborativeWay,
    type NodeRatio,
    type RatioType,
  } from '$lib/flow/ratio.js'
  import type { FlowNode } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'
  import HandlerList from './HandlerList.svelte'
  import ListenerRows from './ListenerRows.svelte'
  import NodeCommonForm from './NodeCommonForm.svelte'

  let {
    store,
    node,
    readonly = false,
  }: { store: DesignerStore; node: FlowNode; readonly?: boolean } = $props()

  const json = $derived(node.data.json)
  const ratio = $derived(parseNodeRatio(json.nodeRatio))
  const ratioError = $derived(validateNodeRatio(ratio))
  const rejectTargets = $derived(
    previousNodes(store.nodes, store.edges, node.id)
  )
  const listenerRows = $derived(
    parseListeners(json.listenerType, json.listenerPath)
  )
  const ratioPlaceholder = $derived(
    RATIO_TYPE_OPTIONS.find((o) => o.value === ratio.type)?.placeholder ?? ''
  )
  // 存量的原始表达式不是可选项,只在当前值就是它时列出来供回显
  const ratioTypeOptions = $derived([
    ...RATIO_TYPE_OPTIONS,
    ...(ratio.type === 'raw'
      ? [{ value: 'raw', label: '原始表达式(引擎不支持)' }]
      : []),
  ])

  // bits-ui 把空串当作未选中,"不指定"用哨兵值表示,写回时还原成 null
  const NO_SKIP = '__none__'
  const rejectOptions = $derived([
    { value: NO_SKIP, label: '不指定' },
    ...rejectTargets.map((t) => ({
      value: t.id,
      label: t.data.json.nodeName || t.id,
    })),
    ...(json.anyNodeSkip &&
    !rejectTargets.some((t) => t.id === json.anyNodeSkip)
      ? [
          {
            value: json.anyNodeSkip,
            label: `${json.anyNodeSkip}(不在前置节点中)`,
          },
        ]
      : []),
  ])
  const labelOf = (
    opts: readonly { value: string; label: string }[],
    v: string
  ) => opts.find((o) => o.value === v)?.label ?? v

  function writeRatio(next: NodeRatio) {
    store.updateNodeJson(node.id, { nodeRatio: formatNodeRatio(next) })
  }
</script>

<Tabs.Root value="base">
  <Tabs.List class="w-full">
    <Tabs.Trigger value="base">基础</Tabs.Trigger>
    <Tabs.Trigger value="handler">办理人</Tabs.Trigger>
    <Tabs.Trigger value="listener">监听器</Tabs.Trigger>
  </Tabs.List>

  <Tabs.Content value="base" class="space-y-4">
    <NodeCommonForm {store} {node} {readonly} />

    <Field.Field>
      <Field.Label>协作方式</Field.Label>
      <RadioGroup.Root
        value={ratio.way}
        disabled={readonly}
        onValueChange={(v) => writeRatio(ratioForWay(v as CollaborativeWay))}
        class="flex gap-4"
      >
        <div class="flex items-center gap-2">
          <RadioGroup.Item value="1" id="fd-way-1" />
          <Label for="fd-way-1" class="font-normal" title="只需一个人审批"
            >或签</Label
          >
        </div>
        <div class="flex items-center gap-2">
          <RadioGroup.Item value="2" id="fd-way-2" />
          <Label
            for="fd-way-2"
            class="font-normal"
            title="部分办理人审批,建议选用户">票签</Label
          >
        </div>
        <div class="flex items-center gap-2">
          <RadioGroup.Item value="3" id="fd-way-3" />
          <Label
            for="fd-way-3"
            class="font-normal"
            title="所有办理人都要审批,建议选用户">会签</Label
          >
        </div>
      </RadioGroup.Root>
    </Field.Field>

    {#if ratio.way === '2'}
      <Field.Field>
        <Field.Label for="fd-ratio-value">票签策略</Field.Label>
        <div class="flex gap-2">
          <Select.Root
            type="single"
            value={ratio.type}
            disabled={readonly}
            onValueChange={(v) =>
              writeRatio({ ...ratio, type: v as RatioType, value: '' })}
          >
            <Select.Trigger class="w-32 shrink-0" aria-label="策略类型">
              {labelOf(ratioTypeOptions, ratio.type)}
            </Select.Trigger>
            <Select.Content>
              {#each ratioTypeOptions as o (o.value)}
                <Select.Item value={o.value} label={o.label} />
              {/each}
            </Select.Content>
          </Select.Root>
          <Input
            id="fd-ratio-value"
            class="min-w-0 flex-1"
            placeholder={ratioPlaceholder}
            value={ratio.value}
            disabled={readonly}
            oninput={(e) =>
              writeRatio({ ...ratio, value: e.currentTarget.value })}
          />
        </div>
        {#if ratioError}<Field.Error>{ratioError}</Field.Error>{/if}
      </Field.Field>
    {/if}

    <Field.Field>
      <Field.Label for="fd-any-skip">驳回到指定节点</Field.Label>
      <Select.Root
        type="single"
        value={json.anyNodeSkip ?? NO_SKIP}
        disabled={readonly}
        onValueChange={(v) =>
          store.updateNodeJson(node.id, {
            anyNodeSkip: v === NO_SKIP ? null : v,
          })}
      >
        <Select.Trigger id="fd-any-skip" class="w-full">
          {labelOf(rejectOptions, json.anyNodeSkip ?? NO_SKIP)}
        </Select.Trigger>
        <Select.Content>
          {#each rejectOptions as o (o.value)}
            <Select.Item value={o.value} label={o.label} />
          {/each}
        </Select.Content>
      </Select.Root>
      {#if ratio.way === '2'}
        <Field.Description>票签建议指定驳回节点</Field.Description>
      {/if}
    </Field.Field>

    <Field.Field>
      <Field.Label>表单</Field.Label>
      <RadioGroup.Root
        value={json.formCustom === 'Y' ? 'Y' : 'N'}
        disabled={readonly}
        onValueChange={(v) => store.updateNodeJson(node.id, { formCustom: v })}
        class="flex gap-4"
      >
        <div class="flex items-center gap-2">
          <RadioGroup.Item value="N" id="fd-form-n" />
          <Label for="fd-form-n" class="font-normal">路径表单</Label>
        </div>
        <div class="flex items-center gap-2">
          <RadioGroup.Item value="Y" id="fd-form-y" />
          <Label for="fd-form-y" class="font-normal">自定义表单</Label>
        </div>
      </RadioGroup.Root>
      <Input
        aria-label={json.formCustom === 'Y' ? '表单唯一标识' : '表单路径'}
        placeholder={json.formCustom === 'Y'
          ? '表单唯一标识'
          : '表单路径,如 /workflow/leaveEdit/index'}
        value={json.formPath ?? ''}
        disabled={readonly}
        oninput={(e) =>
          store.updateNodeJson(node.id, {
            formPath: e.currentTarget.value || null,
          })}
      />
    </Field.Field>
  </Tabs.Content>

  <Tabs.Content value="handler">
    <HandlerList {store} {node} {readonly} />
  </Tabs.Content>

  <Tabs.Content value="listener">
    <ListenerRows
      rows={listenerRows}
      {readonly}
      onChange={(next) => store.updateNodeJson(node.id, formatListeners(next))}
    />
  </Tabs.Content>
</Tabs.Root>
