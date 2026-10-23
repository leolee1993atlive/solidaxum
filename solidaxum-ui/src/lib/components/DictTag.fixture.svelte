<script lang="ts">
  import type { DictDataVo } from '$lib/api/system/dict.js'
  import DictTag from './DictTag.svelte'

  let { dictType, value, rows }: { dictType: string; value: string | number | undefined | null; rows: DictDataVo[] } =
    $props()
</script>

<div data-testid="host">
  <DictTag {dictType} {value} fetcher={async () => rows} />
</div>
