<script lang="ts">
  import { Toaster } from '$lib/components/ui/sonner/index.js'
  import ThemeProvider from '$lib/context/ThemeProvider.svelte'
  import { setAuthContext } from '$lib/context/auth.svelte.js'
  import './layout.css'

  // Initialize auth context for the entire app
  setAuthContext()

  let { children } = $props()
</script>

<ThemeProvider>
  {@render children()}
  <Toaster position="top-right" richColors closeButton />
</ThemeProvider>
