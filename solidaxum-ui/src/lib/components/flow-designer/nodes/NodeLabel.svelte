<script lang="ts">
  import type { Point } from '$lib/flow/types.js'

  // 标签相对节点盒中心定位;网关默认偏移 (0,40) 会落到菱形下方,盒子允许溢出。
  let { text, offset }: { text: string; offset: Point } = $props()
</script>

<div
  class="fd-label"
  style:left="calc(50% + {offset.x}px)"
  style:top="calc(50% + {offset.y}px)"
>
  {text}
</div>

<style>
  .fd-label {
    position: absolute;
    transform: translate(-50%, -50%);
    white-space: nowrap;
    font-size: 12px;
    line-height: 1.2;
    color: var(--foreground, #333);
    user-select: none;
  }
</style>
