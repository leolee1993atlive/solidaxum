<script lang="ts">
  import { facingSide } from '$lib/flow/geometry.js'
  import { orthogonalRoute } from '$lib/flow/route.js'
  import type { Side } from '$lib/flow/types.js'
  import { useConnection } from '@xyflow/svelte'

  // 拖线预览与真实边用同一个路由,落点未吸附到句柄时按相向侧估一个入口
  const connection = useConnection()

  const path = $derived.by(() => {
    const c = connection.current
    if (!c.inProgress) return ''
    const toSide = c.toHandle
      ? (c.toPosition as Side)
      : facingSide(c.to, c.from)
    return orthogonalRoute(c.from, c.fromPosition as Side, c.to, toSide)
      .map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x} ${p.y}`)
      .join(' ')
  })
</script>

<path d={path} fill="none" class="svelte-flow__connection-path" />
