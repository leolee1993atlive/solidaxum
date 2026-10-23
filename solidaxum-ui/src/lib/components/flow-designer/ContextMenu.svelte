<script module lang="ts">
  export interface MenuItem {
    label: string
    shortcut?: string
    disabled?: boolean
    /** 在本项之前画一条分隔线 */
    separatorBefore?: boolean
    onselect: () => void
  }
</script>

<script lang="ts">
  // 纯渲染:菜单项由 CanvasEditor 按右键落点算好传进来。不用 bits-ui 的 ContextMenu ——
  // 它靠 Trigger 元素的 contextmenu 事件开菜单,而这里的"目标"由 xyflow 的 DOM 决定。
  let {
    x,
    y,
    items,
    onclose,
  }: {
    /** 相对画布容器的坐标 */
    x: number
    y: number
    items: MenuItem[]
    onclose: () => void
  } = $props()

  let el = $state<HTMLDivElement>()

  // 夹在容器边界内:菜单不出画布
  const left = $derived.by(() => {
    const parent = el?.offsetParent as HTMLElement | null
    if (!el || !parent) return x
    return Math.max(0, Math.min(x, parent.clientWidth - el.offsetWidth - 4))
  })
  const top = $derived.by(() => {
    const parent = el?.offsetParent as HTMLElement | null
    if (!el || !parent) return y
    return Math.max(0, Math.min(y, parent.clientHeight - el.offsetHeight - 4))
  })

  function outside(e: PointerEvent) {
    if (el && !el.contains(e.target as Node)) onclose()
  }
</script>

<svelte:window
  onpointerdown={outside}
  onkeydown={(e) => e.key === 'Escape' && onclose()}
  onwheel={onclose}
  onblur={onclose}
/>

<div
  bind:this={el}
  class="fd-context-menu nopan bg-popover text-popover-foreground absolute z-10 min-w-[10rem] rounded-md border p-1 text-sm shadow-md"
  role="menu"
  style:left="{left}px"
  style:top="{top}px"
>
  {#each items as item, i (i)}
    {#if item.separatorBefore}
      <div class="bg-border -mx-1 my-1 h-px" role="separator"></div>
    {/if}
    <button
      type="button"
      role="menuitem"
      class="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-4 rounded-sm px-2 py-1.5 text-left text-sm disabled:pointer-events-none disabled:opacity-50"
      disabled={item.disabled}
      onclick={() => {
        item.onselect()
        onclose()
      }}
    >
      <span>{item.label}</span>
      {#if item.shortcut}
        <kbd class="text-muted-foreground ms-auto text-xs tracking-widest"
          >{item.shortcut}</kbd
        >
      {/if}
    </button>
  {/each}
</div>
