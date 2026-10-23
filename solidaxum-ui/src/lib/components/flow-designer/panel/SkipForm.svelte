<script lang="ts">
  import * as Field from '$lib/components/ui/field/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import {
    COMPARE_OPS,
    formatSkipCondition,
    parseSkipCondition,
    validateSkipCondition,
    type SkipCondition,
  } from '$lib/flow/condition.js'
  import type { SkipEdge, SkipType } from '$lib/flow/types.js'
  import type { DesignerStore } from '../state/designer.svelte.js'

  let {
    store,
    edge,
    readonly = false,
  }: { store: DesignerStore; edge: SkipEdge; readonly?: boolean } = $props()

  const cond = $derived(parseSkipCondition(edge.data.json.skipCondition))
  const error = $derived(validateSkipCondition(cond))
  const skipType = $derived(edge.data.json.skipType ?? 'PASS')

  // 存量数据里的 NONE / 原始表达式不是可选项,只在当前值就是它时列出来供回显
  const skipTypeOptions = $derived([
    { value: 'PASS', label: '审批通过' },
    { value: 'REJECT', label: '退回' },
    ...(skipType === 'NONE' ? [{ value: 'NONE', label: '无(NONE)' }] : []),
  ])
  const condKindOptions = $derived([
    { value: 'none', label: '无条件' },
    { value: 'compare', label: '比较条件' },
    { value: 'spel', label: 'spel 表达式' },
    ...(cond.kind === 'raw'
      ? [{ value: 'raw', label: '原始表达式(引擎不支持)' }]
      : []),
  ])
  const labelOf = (
    opts: readonly { value: string; label: string }[],
    v: string
  ) => opts.find((o) => o.value === v)?.label ?? v

  function write(next: SkipCondition) {
    store.updateEdgeJson(edge.id, { skipCondition: formatSkipCondition(next) })
  }

  /** 切换条件方式;比较 ↔ spel 之间尽量保留已填内容 */
  function setKind(kind: string) {
    if (kind === 'none') write({ kind: 'none' })
    else if (kind === 'spel') {
      write({
        kind: 'spel',
        expr: cond.kind === 'compare' ? `#${cond.variable}` : '',
      })
    } else if (kind === 'compare') {
      write({ kind: 'compare', op: 'eq', variable: '', value: '' })
    }
  }
</script>

<Field.Field>
  <Field.Label for="fd-skip-name">跳转名称</Field.Label>
  <Input
    id="fd-skip-name"
    value={edge.data.json.skipName ?? ''}
    disabled={readonly}
    oninput={(e) =>
      store.updateEdgeJson(edge.id, { skipName: e.currentTarget.value })}
  />
</Field.Field>

<Field.Field>
  <Field.Label for="fd-skip-type">跳转类型</Field.Label>
  <Select.Root
    type="single"
    value={skipType}
    disabled={readonly}
    onValueChange={(v) =>
      store.updateEdgeJson(edge.id, { skipType: v as SkipType })}
  >
    <Select.Trigger id="fd-skip-type" class="w-full">
      {labelOf(skipTypeOptions, skipType)}
    </Select.Trigger>
    <Select.Content>
      {#each skipTypeOptions as o (o.value)}
        <Select.Item value={o.value} label={o.label} />
      {/each}
    </Select.Content>
  </Select.Root>
</Field.Field>

<Field.Field>
  <Field.Label for="fd-skip-cond-kind">跳转条件</Field.Label>
  <Select.Root
    type="single"
    value={cond.kind}
    disabled={readonly}
    onValueChange={setKind}
  >
    <Select.Trigger id="fd-skip-cond-kind" class="w-full">
      {labelOf(condKindOptions, cond.kind)}
    </Select.Trigger>
    <Select.Content>
      {#each condKindOptions as o (o.value)}
        <Select.Item value={o.value} label={o.label} />
      {/each}
    </Select.Content>
  </Select.Root>

  {#if cond.kind === 'compare'}
    <div class="flex gap-2">
      <Input
        class="min-w-0 flex-1"
        placeholder="变量名"
        aria-label="条件变量"
        value={cond.variable}
        disabled={readonly}
        oninput={(e) => write({ ...cond, variable: e.currentTarget.value })}
      />
      <Select.Root
        type="single"
        value={cond.op}
        disabled={readonly}
        onValueChange={(v) => write({ ...cond, op: v as typeof cond.op })}
      >
        <Select.Trigger class="w-28 shrink-0" aria-label="比较方式">
          {labelOf(COMPARE_OPS, cond.op)}
        </Select.Trigger>
        <Select.Content>
          {#each COMPARE_OPS as op (op.value)}
            <Select.Item value={op.value} label={op.label} />
          {/each}
        </Select.Content>
      </Select.Root>
      <Input
        class="min-w-0 flex-1"
        placeholder="比较值"
        aria-label="比较值"
        value={cond.value}
        disabled={readonly}
        oninput={(e) => write({ ...cond, value: e.currentTarget.value })}
      />
    </div>
  {:else if cond.kind === 'spel'}
    <Textarea
      aria-label="spel 表达式"
      placeholder={'如 #leaveDays > 2 && #type == "annual"'}
      value={cond.expr}
      disabled={readonly}
      oninput={(e) => write({ kind: 'spel', expr: e.currentTarget.value })}
    />
  {:else if cond.kind === 'raw'}
    <Textarea
      aria-label="原始表达式"
      value={cond.raw}
      disabled={readonly}
      oninput={(e) => write({ kind: 'raw', raw: e.currentTarget.value })}
    />
  {/if}
  {#if error}<Field.Error>{error}</Field.Error>{/if}
</Field.Field>
