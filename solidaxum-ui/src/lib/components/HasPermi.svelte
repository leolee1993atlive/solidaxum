<script lang="ts">
  import type { Snippet } from 'svelte'
  import { useAuth } from '$lib/context/auth.svelte.js'

  let { perm, children }: { perm: string | string[]; children: Snippet } = $props()

  const auth = useAuth()

  // 数组任一命中即通过，对齐 Vue v-hasPermi 的「有其一」语义。
  // 读取 auth.hasPermission 会依赖 auth.permissions($state)，
  // getInfo 异步加载后此 derived 自动重算 → 按钮延迟显现。
  const allowed = $derived(
    Array.isArray(perm)
      ? perm.some((p) => auth.hasPermission(p))
      : auth.hasPermission(perm),
  )
</script>

{#if allowed}{@render children()}{/if}
