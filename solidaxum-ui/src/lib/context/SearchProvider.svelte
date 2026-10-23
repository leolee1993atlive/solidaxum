<script lang="ts">
  import CommandMenu from '$lib/components/CommandMenu.svelte'
  import { setSearchContext } from './search.svelte.js'

  let { children } = $props()

  const searchState = setSearchContext()
</script>

<svelte:window onkeydown={searchState.handleKeyDown} />

{@render children?.()}

<CommandMenu />
