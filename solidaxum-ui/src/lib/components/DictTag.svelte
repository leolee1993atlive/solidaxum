<script lang="ts">
  import type { DictDataVo } from '$lib/api/system/dict.js'
  import {
    resolveDictTagClass,
    resolveDictTagInlineStyle,
  } from '$lib/components/dict-tag.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { fetchDict, resolveDictClass, resolveDictCssClass, resolveDictLabel } from '$lib/hooks/use-dict.js'
  import { cn } from '$lib/utils.js'

  let {
    dictType,
    value,
    /** 测试注入：跳过网络。生产不传。 */
    fetcher,
  }: {
    dictType: string
    value: string | number | undefined | null
    fetcher?: (dictType: string) => Promise<DictDataVo[]>
  } = $props()

  let list = $state<DictDataVo[]>([])

  $effect(() => {
    // dictType 变化时重拉；fetchDict 自带内存缓存，重复调用不会重复请求。
    fetchDict(dictType, fetcher).then((l) => (list = l))
  })

  let label = $derived(resolveDictLabel(list, value))
  let listClass = $derived(resolveDictClass(list, value))
  let tagClass = $derived(resolveDictTagClass(listClass))
  // 旧库把 #fa8c16 这类 hex 直接写进 list_class(ae_rep_status 全族),
  // 语义类映射不命中 —— 走内联样式,观感对齐旧版 antd Tag。
  let tagStyle = $derived(resolveDictTagInlineStyle(listClass))
  let cssClass = $derived(resolveDictCssClass(list, value))
</script>

{#if tagStyle}
  <Badge variant="outline" class={cssClass} style={tagStyle}>{label}</Badge>
{:else if tagClass || cssClass}
  <Badge variant="outline" class={cn(tagClass, cssClass)}>{label}</Badge>
{:else}
  <span>{label}</span>
{/if}
