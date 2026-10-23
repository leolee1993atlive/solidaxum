<script lang="ts">
  import { setThemeContext, type Theme } from './theme.svelte.js'

  let { children } = $props()

  const themeState = setThemeContext({
    defaultTheme: 'system',
    storageKey: 'vite-ui-theme',
  })

  // Use $effect to handle theme side effects (document class and system listener)
  $effect(() => {
    themeState.applyToDocument()
  })

  $effect(() => {
    if (themeState.theme !== 'system') return

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
    const handleChange = () => {
      themeState.applyToDocument()
    }

    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  })
</script>

{@render children?.()}
