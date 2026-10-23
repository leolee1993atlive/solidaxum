<script lang="ts">
  import { sanitizeHtml } from '$lib/html/sanitize.js'
  import { cn } from '$lib/utils.js'

  let {
    html,
    class: className = '',
  }: {
    /** 未消毒的 HTML(直接来自数据库)。消毒在这里做,不回写库。 */
    html: string
    class?: string
  } = $props()

  const clean = $derived(sanitizeHtml(html ?? ''))
</script>

<!--
  全仓唯一允许 {@html} 的地方 —— 护栏见 src/lib/html/html-contract.test.ts。
  clean 必须来自 sanitizeHtml,不要在这里接任何别的字符串源。
-->
<div class={cn('rich-text', className)}>{@html clean}</div>
