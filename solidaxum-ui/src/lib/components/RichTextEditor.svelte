<script lang="ts">
  import { untrack } from 'svelte'
  import {
    TOOLBAR_ITEMS,
    URL_TOOLBAR_ITEMS,
    isEmptyHtml,
    needsContentSync,
  } from '$lib/components/rich-text.js'
  import type {
    ToolbarItem,
    UrlToolbarItem,
  } from '$lib/components/rich-text.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { isSafeUrl } from '$lib/html/sanitize.js'
  import { cn } from '$lib/utils.js'
  import { Editor } from '@tiptap/core'
  import Image from '@tiptap/extension-image'
  import StarterKit from '@tiptap/starter-kit'
  import { toast } from 'svelte-sonner'

  let {
    value = $bindable(''),
    readonly = false,
    placeholder = '',
    minHeight = 192,
    class: className = '',
  }: {
    /** HTML 字符串。空内容统一回写成 `''`,不是 tiptap 的 `'<p></p>'`。 */
    value?: string
    readonly?: boolean
    placeholder?: string
    /** 编辑区最小高度(px)。默认 192,对齐 Vue 的 :min-height="192"。 */
    minHeight?: number
    class?: string
  } = $props()

  let host = $state<HTMLDivElement | null>(null)
  let editor = $state<Editor | null>(null)

  /**
   * tiptap 的状态在 Svelte 的响应式系统之外。工具栏的按下态、placeholder 的
   * 显隐都靠这个计数器驱动重算 —— 读它就等于订阅「编辑器状态变了」。
   */
  let version = $state(0)

  // tiptap 要一个已挂载的真实元素,只能在 $effect 里创建。
  // 弹窗关闭时 bits-ui 会卸载子节点,清理函数必须 destroy(),否则每开一次泄漏一个视图。
  $effect(() => {
    const el = host
    if (!el) return

    // placeholder / minHeight 也只读一次 —— 直接读会让它们成为本 effect 的依赖,
    // 父组件改个文案就销毁重建整个编辑器(选区与撤销栈一起丢)。代价是改了之后
    // 编辑区的 aria-label / 高度不跟着变,而这两个 prop 在调用方都是字面量。
    const [initialMinHeight, initialLabel] = untrack(() => [
      minHeight,
      placeholder,
    ])

    const instance = new Editor({
      element: el,
      // 初值只读一次:把 value 放进依赖会让每敲一个字就重建编辑器。
      // readonly 同理用 untrack —— 但初值必须在这里给,不能只靠下面那条
      // setEditable 的 effect:那条要等 editor 赋值后再跑一轮,中间有一帧可编辑。
      // 边界:若将来把 value 改成异步加载(先开弹窗、后拉详情),这里读到的是 '',
      // 初值会空一帧,靠下面那条 setContent effect 用 needsContentSync 补 ——
      // 所以那条 effect 不能因为「看着没用」就删。
      content: untrack(() => value),
      editable: untrack(() => !readonly),
      extensions: [
        StarterKit.configure({
          // 不做代码块 —— 显式关掉,而不是让白名单去吃它的输出
          code: false,
          codeBlock: false,
          // 默认 1-6。h4-h6 不在白名单里,不限就会「编辑得出、渲染时被吃掉」
          heading: { levels: [1, 2, 3] },
          link: {
            openOnClick: false,
            autolink: false,
            HTMLAttributes: { target: '_blank', rel: 'noopener noreferrer' },
          },
        }),
        // 只支持插入 URL,不做编辑器内上传,更不收内嵌 base64
        Image.configure({ inline: false, allowBase64: false }),
      ],
      editorProps: {
        attributes: {
          class: 'rich-text px-3 py-2 outline-none',
          style: `min-height:${initialMinHeight}px`,
          // a11y:contenteditable 不是 labelable 元素,<Label for=...> 关联不过去
          // (故 notice 页不用 for)。改用 ARIA 显式声明:
          // role=textbox 让屏幕阅读器当成输入框,aria-multiline 告诉它是多行,
          // aria-label 用 placeholder 文案兜底(没传 placeholder 就用通用文案)。
          // 用 aria-label 而非 aria-labelledby:Label 没稳定 id 可指。
          role: 'textbox',
          'aria-multiline': 'true',
          'aria-label': initialLabel || '富文本编辑区',
        },
      },
      onUpdate: ({ editor: e }) => {
        const html = e.getHTML()
        // 空文档回写成 '',否则表单的 `noticeContent || undefined` 会把 '<p></p>' 当有值
        value = isEmptyHtml(html) ? '' : html
        version++
      },
      onSelectionUpdate: () => version++,
      onTransaction: () => version++,
    })

    editor = instance
    return () => {
      instance.destroy()
      editor = null
    }
  })

  // readonly 单独一条 effect:写进创建那条会让切换只读态重建整个编辑器。
  // 注意只用 readonly 当依赖 —— editor 用 untrack 读,否则 editor 从 null→instance
  // 时本 effect 也会跑,而那时创建逻辑已经用 editable:untrack(()=>!readonly) 给过
  // 初值了。更关键的是:直接读 editor 会让本 effect 追踪 editor,setEditable 触发
  // 的 onUpdate 里 version++ 又卷进响应式调度,在 Svelte 5 下会触发死循环
  // (effect_update_depth_exceeded)。untrack 切断这条链。
  $effect(() => {
    const ro = readonly
    untrack(() => {
      if (editor) editor.setEditable(!ro)
    })
  })

  // 外部改 value 时回灌。相等就不动,否则每次输入都会重置选区(光标跳回开头)
  $effect(() => {
    const next = value
    const e = editor
    if (!e) return
    if (!needsContentSync(next, e.getHTML())) return
    e.commands.setContent(next, { emitUpdate: false })
  })

  function showPlaceholder(): boolean {
    void version // 订阅编辑器状态
    return placeholder !== '' && !!editor && editor.isEmpty
  }

  /** 正在等用户输入 URL 的那一项。null = 输入框不显示。 */
  let urlPrompt = $state<{ item: UrlToolbarItem; url: string } | null>(null)

  function isActive(item: ToolbarItem | UrlToolbarItem): boolean {
    void version // 订阅编辑器状态
    return editor ? (item.isActive?.(editor) ?? false) : false
  }

  function confirmUrl(): void {
    if (!urlPrompt || !editor) return
    const url = urlPrompt.url.trim()
    // 第一道校验:编辑面。渲染面的白名单会再挡一次(sanitize.ts 用同一个 isSafeUrl)
    if (!isSafeUrl(url, urlPrompt.item.urlKind)) {
      toast.error('地址不合法，只支持 http/https 或站内相对路径')
      return
    }
    urlPrompt.item.run(editor, url)
    urlPrompt = null
  }
</script>

<!--
  focus-within 的聚焦环由外框给:编辑区自己的 outline 被 .rich-text:focus 关掉了
  (ProseMirror 的默认 outline 与 shadcn 的 ring 打架),不补这层,键盘用户在这个
  控件上就完全看不到焦点 —— 它取代的 Textarea 是有 focus ring 的。
  取值对齐 ui/input:focus-visible:border-ring + ring-ring/50 + ring-[3px]。
-->
<div
  class={cn(
    'border-input bg-background rounded-md border',
    'focus-within:border-ring focus-within:ring-ring/50 focus-within:ring-[3px]',
    className
  )}
>
  {#if !readonly}
    <div
      class="border-input flex flex-wrap items-center gap-0.5 border-b px-2 py-1.5"
      role="toolbar"
      aria-label="富文本工具栏"
    >
      {#each TOOLBAR_ITEMS as item (item.id)}
        {@const Icon = item.icon}
        <button
          type="button"
          title={item.label}
          aria-label={item.label}
          aria-pressed={item.isActive ? isActive(item) : undefined}
          class={cn(
            'hover:bg-accent hover:text-accent-foreground rounded p-1.5',
            isActive(item) && 'bg-accent text-accent-foreground'
          )}
          onclick={() => editor && item.run(editor)}
        >
          <Icon class="size-4" />
        </button>
      {/each}
      {#each URL_TOOLBAR_ITEMS as item (item.id)}
        {@const Icon = item.icon}
        <button
          type="button"
          title={item.label}
          aria-label={item.label}
          aria-pressed={item.isActive ? isActive(item) : undefined}
          class={cn(
            'hover:bg-accent hover:text-accent-foreground rounded p-1.5',
            isActive(item) && 'bg-accent text-accent-foreground'
          )}
          onclick={() => (urlPrompt = { item, url: '' })}
        >
          <Icon class="size-4" />
        </button>
      {/each}
    </div>

    {#if urlPrompt}
      <div class="border-input flex items-center gap-2 border-b px-2 py-1.5">
        <Input
          bind:value={urlPrompt.url}
          placeholder={urlPrompt.item.placeholder}
          class="h-8"
          onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && confirmUrl()}
        />
        <Button size="sm" onclick={confirmUrl}>确定</Button>
        <Button size="sm" variant="outline" onclick={() => (urlPrompt = null)}
          >取消</Button
        >
      </div>
    {/if}
  {/if}

  <div class="relative">
    <div bind:this={host}></div>
    {#if showPlaceholder()}
      <div
        class="text-muted-foreground pointer-events-none absolute top-0 left-0 px-3 py-2 text-sm"
      >
        {placeholder}
      </div>
    {/if}
  </div>
</div>
