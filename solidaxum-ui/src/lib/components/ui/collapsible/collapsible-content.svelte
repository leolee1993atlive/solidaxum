<script lang="ts">
  import { Collapsible as CollapsiblePrimitive } from 'bits-ui'

  let {
    ref = $bindable(null),
    ...restProps
  }: CollapsiblePrimitive.ContentProps = $props()
</script>

<!-- 子菜单折叠/展开高度动画。tw-animate-css 自带
     collapsible-down/up keyframes(引用 --bits-collapsible-content-height);
     bits-ui PresenceManager 保证关闭动画播完后再卸载 DOM。 -->
<CollapsiblePrimitive.Content
  bind:ref
  data-slot="collapsible-content"
  class="overflow-hidden data-[state=open]:animate-collapsible-down data-[state=closed]:animate-collapsible-up"
  {...restProps}
/>
