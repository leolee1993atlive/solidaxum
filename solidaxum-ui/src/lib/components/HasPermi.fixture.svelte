<script lang="ts">
  import { setAuthContext } from '$lib/context/auth.svelte.js'
  import HasPermi from './HasPermi.svelte'

  let { perms = [], perm }: { perms?: string[]; perm: string | string[] } = $props()

  const auth = setAuthContext()

  $effect(() => {
    auth.permissions = perms
  })
</script>

<HasPermi {perm}>
  <span data-testid="guarded">OK</span>
</HasPermi>
