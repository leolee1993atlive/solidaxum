<script lang="ts">
  import { fly } from 'svelte/transition'
  import { SegmentedControl } from '$lib/components/crud/index.js'
  import MultiSelect from '$lib/components/MultiSelect.svelte'
  import TreeSelect from '$lib/components/TreeSelect.svelte'
  import UserPicker from '$lib/components/process/UserPicker.svelte'
  import type {
    QueryField,
    QueryOption,
  } from '$lib/components/query-form-types.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { fetchDict } from '$lib/hooks/use-dict.js'
  import { Search, RotateCcw, ChevronDown, ChevronUp } from '@lucide/svelte'

  let {
    fields = [],
    onSearch,
    onReset,
    initialModel = {},
  }: {
    fields?: QueryField[]
    onSearch?: (model: Record<string, string>) => void
    onReset?: () => void
    /** 初始值(如创建时间默认近 3 月);键与 model 同(beginKey/endKey 等)。 */
    initialModel?: Record<string, string>
  } = $props()

  let model = $state<Record<string, string>>({ ...initialModel })

  // 初值里的裸日期补默认时刻(起 00:00:00 / 止 23:59:59),与 submit 同语义。
  for (const f of fields) {
    if (f.type !== 'dateRange') continue
    const beginKey = f.beginKey ?? 'beginTime'
    const endKey = f.endKey ?? 'endTime'
    if (model[beginKey]) model[beginKey] = withTime(model[beginKey], false)
    if (model[endKey]) model[endKey] = withTime(model[endKey], true)
  }

  // ---- 收起/展开(对齐青绿列表页):超过 3 个字段时收起只显示前三个,
  // 查询栏保持一行,不再把页面撑出多行。隐藏字段的已填值仍随搜索提交,
  // 重置仍清空全部(与 vben 行为一致)。
  const COLLAPSED_COUNT = 3
  let expanded = $state(false)
  const collapsible = $derived(fields.length > COLLAPSED_COUNT)
  const visibleFields = $derived(
    collapsible && !expanded ? fields.slice(0, COLLAPSED_COUNT) : fields
  )

  // userSelect / multiSelect 的选中态需要单独存数组
  // (model 里是逗号分隔串,见各自 onChange 注释)。
  let userValues = $state<Record<string, string[]>>({})
  let multiValues = $state<Record<string, string[]>>({})

  let fieldOptions = $state<Record<string, QueryOption[]>>({})

  $effect(() => {
    for (const f of fields) {
      if ((f.type === 'select' || f.type === 'multiSelect') && f.dictType) {
        fetchDict(f.dictType).then((list) => {
          fieldOptions[f.key] = list.map((d) => ({
            label: d.dictLabel,
            value: d.dictValue,
          }))
        })
      }
    }
  })

  function optionsFor(
    f: Extract<QueryField, { type: 'select' | 'multiSelect' }>
  ): QueryOption[] {
    if (f.options) return f.options
    return fieldOptions[f.key] ?? []
  }

  /** 触发器显示文案:'' = 全部(空选项),取不到 label 时回显原值。 */
  function selectLabel(
    f: Extract<QueryField, { type: 'select' }>,
    current: string | undefined
  ): string {
    if (!current) return '全部'
    return optionsFor(f).find((o) => o.value === current)?.label ?? current
  }

  /**
   * 日期范围值补足时分秒。
   * 🔴 后端五处 repo(operlog/logininfo/gen_table/role/user)的日期范围解析都是
   *    `NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S")`,且写成 `if let Ok(dt) = ...`
   *    —— **解析失败就跳过整个筛选**,不报错。只发 'YYYY-MM-DD' 等于筛选没生效。
   *    Vue 侧一直是发满的:value-format="YYYY-MM-DD HH:mm:ss"
   *    + :default-time="[00:00:00, 23:59:59]"。
   * 起止两端的默认时刻逐字对齐 Vue 的 default-time。
   */
  function withTime(value: string, end: boolean): string {
    if (value === '' || value.includes(' ')) return value
    return `${value} ${end ? '23:59:59' : '00:00:00'}`
  }

  function submit(e: Event) {
    e.preventDefault()
    const beginKeys = new Set<string>()
    const endKeys = new Set<string>()
    for (const f of fields) {
      if (f.type === 'dateRange') {
        beginKeys.add(f.beginKey ?? 'beginTime')
        endKeys.add(f.endKey ?? 'endTime')
      }
    }
    const filtered: Record<string, string> = {}
    for (const [k, v] of Object.entries(model)) {
      const isDateKey = beginKeys.has(k) || endKeys.has(k)
      // 空的日期键直接丢弃:后端 Option<String> 收到 Some("") 不等于 None。
      if (v === '' && isDateKey) continue
      filtered[k] = isDateKey ? withTime(v, endKeys.has(k)) : v
    }
    onSearch?.(filtered)
  }
  function reset() {
    model = {}
    // userValues/multiValues 不清就会出现「UI 还显示已选、model 已空」的视觉与数据不同步。
    userValues = {}
    multiValues = {}
    onReset?.()
  }
</script>

<!-- 固定四列栅格。搜索/重置格用 col-start-4 钉在最后一行第 4 格,
     空格因此只出现在它左侧,最右端恒为按钮。
     每个字段包一层 display:contents 的 wrapper 承载展开过渡(in:fly),
     wrapper 对栅格布局完全透明。 -->
<form class="grid w-full grid-cols-4 gap-x-4 gap-y-3" onsubmit={submit}>
  {#each visibleFields as f (f.key)}
    <div class="contents" in:fly={{ y: -6, duration: 160 }}>
      {#if f.type === 'dateRange'}
        {@const beginKey = f.beginKey ?? 'beginTime'}
        {@const endKey = f.endKey ?? 'endTime'}
        <div
          class="col-span-2 flex items-center gap-2 text-sm"
          data-field-type="dateRange"
        >
          <span class="w-20 shrink-0 text-right font-medium">{f.label}</span>
          <!-- 单框区间观感:一个描边容器装起止两个无边框日期输入,中间 ~ 分隔。 -->
          <div
            class="border-input flex h-9 min-w-0 flex-1 items-center rounded-md border"
          >
            <input
              type="date"
              class="h-full min-w-0 flex-1 rounded-l-md bg-transparent px-2"
              aria-label="{f.label} 起"
              value={model[beginKey] ?? ''}
              onchange={(e: Event & { currentTarget: HTMLInputElement }) => {
                model[beginKey] = e.currentTarget.value
              }}
            />
            <span class="text-muted-foreground px-1 select-none">~</span>
            <input
              type="date"
              class="h-full min-w-0 flex-1 rounded-r-md bg-transparent px-2"
              aria-label="{f.label} 止"
              value={model[endKey] ?? ''}
              onchange={(e: Event & { currentTarget: HTMLInputElement }) => {
                model[endKey] = e.currentTarget.value
              }}
            />
          </div>
        </div>
      {:else if f.type === 'treeSelect'}
        <div class="flex items-center gap-2 text-sm">
          <span class="w-20 shrink-0 text-right font-medium">{f.label}</span>
          <div class="min-w-0 flex-1">
            <TreeSelect
              treeData={f.treeData}
              value={model[f.key]}
              placeholder={f.placeholder}
              onChange={(id) => {
                model[f.key] = String(id)
              }}
            />
          </div>
        </div>
      {:else if f.type === 'userSelect'}
        <div class="flex items-center gap-2 text-sm">
          <span class="w-20 shrink-0 text-right font-medium">{f.label}</span>
          <div class="min-w-0 flex-1">
            <UserPicker
              value={userValues[f.key] ?? []}
              placeholder={f.placeholder ?? '请选择申请人'}
              onChange={(next) => {
                userValues[f.key] = next
                // 后端 create_by_ids 约定为逗号分隔字符串,
                // 不是数组重复键 —— toPageQuery 只发 Record<string,string>。
                model[f.key] = next.join(',')
              }}
            />
          </div>
        </div>
      {:else if f.type === 'multiSelect'}
        <div class="flex items-center gap-2 text-sm" data-field-type="multiSelect">
          <span class="w-20 shrink-0 text-right font-medium">{f.label}</span>
          <div class="min-w-0 flex-1">
            <MultiSelect
              options={optionsFor(f)}
              values={multiValues[f.key] ?? []}
              placeholder={f.placeholder ?? '全部'}
              onChange={(next) => {
                multiValues[f.key] = next
                // 多选随搜索提交为逗号串(对齐旧版 vben 多选传参)。
                model[f.key] = next.join(',')
              }}
            />
          </div>
        </div>
      {:else if f.type === 'segmented'}
        <div class="flex items-center gap-2 text-sm" data-field-type="segmented">
          <span class="w-20 shrink-0 text-right font-medium">{f.label}</span>
          <div class="min-w-0 flex-1">
            <SegmentedControl
              options={f.options}
              value={model[f.key] ?? f.options[0]?.value ?? ''}
              size="sm"
              onchange={(v) => {
                model[f.key] = v
              }}
            />
          </div>
        </div>
      {:else}
        <label for={f.key} class="flex items-center gap-2 text-sm">
          <span class="w-20 shrink-0 text-right font-medium">{f.label}</span>
          {#if f.type === 'select'}
            <!-- __all__ 哨兵对应「全部」(model 存 ''),避免空串在 bits-ui 里被当作未选。 -->
            <Select.Root
              type="single"
              value={model[f.key] || '__all__'}
              onValueChange={(v) => {
                model[f.key] = v === '__all__' ? '' : v
              }}
            >
              <Select.Trigger id={f.key} class="min-w-0 flex-1">
                <span class="truncate">
                  {selectLabel(f, model[f.key])}
                </span>
              </Select.Trigger>
              <Select.Content>
                <Select.Item value="__all__" label="全部" />
                {#each optionsFor(f) as opt (opt.value)}
                  <Select.Item value={opt.value} label={opt.label} />
                {/each}
              </Select.Content>
            </Select.Root>
          {:else}
            <input
              id={f.key}
              class="border-input h-9 min-w-0 flex-1 rounded-md border px-3"
              placeholder={('placeholder' in f ? f.placeholder : undefined) ??
                f.label}
              bind:value={model[f.key]}
            />
          {/if}
        </label>
      {/if}
    </div>
  {/each}
  <!-- col-start-4:CSS 自动布局会把它推到「最后一行第 4 格」。
       实测(规格 §5.3):3 字段占满一行;4 字段时独占次行;5/6 字段时
       空格落在它左侧。 -->
  <div
    data-testid="query-actions"
    class="col-start-4 flex items-center justify-end gap-2"
  >
    <Button type="submit" size="sm"><Search class="mr-1 h-4 w-4" />搜索</Button>
    <Button type="button" variant="outline" size="sm" onclick={reset}
      ><RotateCcw class="mr-1 h-4 w-4" />重置</Button
    >
    {#if collapsible}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onclick={() => (expanded = !expanded)}
      >
        {expanded ? '收起' : '展开'}
        {#if expanded}
          <ChevronUp class="mr-1 h-4 w-4" />
        {:else}
          <ChevronDown class="mr-1 h-4 w-4" />
        {/if}
      </Button>
    {/if}
  </div>
</form>
