import {
  Bold,
  Eraser,
  Heading1,
  Heading2,
  Heading3,
  Image as ImageIcon,
  Italic,
  Link as LinkIcon,
  List,
  ListOrdered,
  Minus,
  Quote,
  Redo2,
  Strikethrough,
  Underline as UnderlineIcon,
  Undo2,
  Unlink,
} from '@lucide/svelte'
// LucideIcon = Component<LucideProps>,由 @lucide/svelte 从根导出(dist/types.d.ts)。
// 别写成裸的 svelte `Component` —— 图标的 props 类型对不上,svelte-check 会红。
import type { LucideIcon } from '@lucide/svelte'
import type { Editor } from '@tiptap/core'
import '@tiptap/extension-image'
// 副作用 import:tiptap 的扩展用 `declare module '@tiptap/core'` 把命令
// (toggleBold/setLink/setImage 等)增强进 ChainedCommands。这些类型增强只在
// 「import 了对应扩展模块」时才对 TS 可见 —— 光 import type { Editor } 拿到的
// 是裸 Editor 类型,下面工具栏表里的命令调用会全部报「不存在于 ChainedCommands」。
// starter-kit 聚合了 bold/italic/underline/strike/heading/list/blockquote/
// horizontalRule/link/undoRedo 等全部子扩展的类型声明,image 单独引。
// 运行时这两个模块的代码不会执行(run 回调只在 RichTextEditor 里被调),
// 但它们必须出现在编译产物里,否则 TS 看不到命令类型。
import '@tiptap/starter-kit'

/**
 * 工具栏一项。
 *
 * `tags` 是这项功能**能写进 HTML 的标签** —— `sanitize-parity.test.ts` 靠它
 * 逐条对齐 `$lib/html/sanitize.ts` 的白名单。**加功能必须同时填 tags**,
 * 否则会出现「编辑器写得出来、渲染时被自己的消毒器吃掉」。
 */
export interface ToolbarItem {
  id: string
  label: string
  icon: LucideIcon
  tags: readonly string[]
  run: (editor: Editor) => void
  isActive?: (editor: Editor) => boolean
}

/** 需要先问用户要一个 URL 的项 —— `run` 多一个入参,所以跟上面分开放。 */
export interface UrlToolbarItem extends Omit<ToolbarItem, 'run'> {
  /** 交给 isSafeUrl 的校验口径:link 走 href(认 mailto),image 走 src(不认)。 */
  urlKind: 'href' | 'src'
  placeholder: string
  run: (editor: Editor, url: string) => void
}

/** 一键就能用的项。顺序即工具栏渲染顺序。 */
export const TOOLBAR_ITEMS: readonly ToolbarItem[] = [
  {
    id: 'bold',
    label: '粗体',
    icon: Bold,
    tags: ['strong'],
    run: (e) => e.chain().focus().toggleBold().run(),
    isActive: (e) => e.isActive('bold'),
  },
  {
    id: 'italic',
    label: '斜体',
    icon: Italic,
    tags: ['em'],
    run: (e) => e.chain().focus().toggleItalic().run(),
    isActive: (e) => e.isActive('italic'),
  },
  {
    id: 'underline',
    label: '下划线',
    icon: UnderlineIcon,
    tags: ['u'],
    run: (e) => e.chain().focus().toggleUnderline().run(),
    isActive: (e) => e.isActive('underline'),
  },
  {
    id: 'strike',
    label: '删除线',
    icon: Strikethrough,
    tags: ['s'],
    run: (e) => e.chain().focus().toggleStrike().run(),
    isActive: (e) => e.isActive('strike'),
  },
  {
    id: 'h1',
    label: '一级标题',
    icon: Heading1,
    tags: ['h1'],
    run: (e) => e.chain().focus().toggleHeading({ level: 1 }).run(),
    isActive: (e) => e.isActive('heading', { level: 1 }),
  },
  {
    id: 'h2',
    label: '二级标题',
    icon: Heading2,
    tags: ['h2'],
    run: (e) => e.chain().focus().toggleHeading({ level: 2 }).run(),
    isActive: (e) => e.isActive('heading', { level: 2 }),
  },
  {
    id: 'h3',
    label: '三级标题',
    icon: Heading3,
    tags: ['h3'],
    run: (e) => e.chain().focus().toggleHeading({ level: 3 }).run(),
    isActive: (e) => e.isActive('heading', { level: 3 }),
  },
  {
    id: 'orderedList',
    label: '有序列表',
    icon: ListOrdered,
    tags: ['ol', 'li'],
    run: (e) => e.chain().focus().toggleOrderedList().run(),
    isActive: (e) => e.isActive('orderedList'),
  },
  {
    id: 'bulletList',
    label: '无序列表',
    icon: List,
    tags: ['ul', 'li'],
    run: (e) => e.chain().focus().toggleBulletList().run(),
    isActive: (e) => e.isActive('bulletList'),
  },
  {
    id: 'blockquote',
    label: '引用',
    icon: Quote,
    tags: ['blockquote'],
    run: (e) => e.chain().focus().toggleBlockquote().run(),
    isActive: (e) => e.isActive('blockquote'),
  },
  {
    id: 'horizontalRule',
    label: '分割线',
    icon: Minus,
    tags: ['hr'],
    run: (e) => e.chain().focus().setHorizontalRule().run(),
  },
  {
    id: 'unlink',
    label: '移除链接',
    icon: Unlink,
    tags: [],
    run: (e) => e.chain().focus().unsetLink().run(),
  },
  {
    id: 'clearFormat',
    label: '清除格式',
    icon: Eraser,
    tags: [],
    run: (e) => e.chain().focus().unsetAllMarks().clearNodes().run(),
  },
  {
    id: 'undo',
    label: '撤销',
    icon: Undo2,
    tags: [],
    run: (e) => e.chain().focus().undo().run(),
  },
  {
    id: 'redo',
    label: '重做',
    icon: Redo2,
    tags: [],
    run: (e) => e.chain().focus().redo().run(),
  },
]

/** 需要输入 URL 的项。图片只支持插入 URL,不做编辑器内上传。 */
export const URL_TOOLBAR_ITEMS: readonly UrlToolbarItem[] = [
  {
    id: 'link',
    label: '插入链接',
    icon: LinkIcon,
    tags: ['a'],
    urlKind: 'href',
    placeholder: 'https://example.com 或 /system/notice',
    run: (e, url) => {
      // 光标塌缩且不在已有链接里时,setLink 只往 stored mark 里塞一笔 —— 文档
      // 一个字符都不变,用户看到的是「输了地址、点了确定、什么也没发生」。
      // 这种情况改插一段以地址本身为文字的链接,对齐 Vue 编辑器的表现。
      // 判断用 isActive 而不是 selection.empty:光标在已有链接里时下面那条链的
      // extendMarkRange 会把整个链接选中,那才是真正的「改这个链接的地址」。
      if (e.state.selection.empty && !e.isActive('link')) {
        e.chain()
          .focus()
          .insertContent({
            type: 'text',
            text: url,
            marks: [{ type: 'link', attrs: { href: url } }],
          })
          .run()
        return
      }
      e.chain().focus().extendMarkRange('link').setLink({ href: url }).run()
    },
    isActive: (e) => e.isActive('link'),
  },
  {
    id: 'image',
    label: '插入图片',
    icon: ImageIcon,
    tags: ['img'],
    urlKind: 'src',
    placeholder: '图片地址(可在文件管理页复制)',
    run: (e, url) => e.chain().focus().setImage({ src: url }).run(),
  },
]

/** 空内容有两种写法:`''`(外部)与 `'<p></p>'`(tiptap 的空文档)。 */
export function isEmptyHtml(html: string): boolean {
  const t = html.trim()
  return t === '' || t === '<p></p>'
}

/**
 * 外部 `value` 变了才回灌编辑器。
 *
 * 相等还调 setContent 会重建整个文档、把选区弹回开头 —— 表现为「打一个字光标
 * 跳到开头」。挂载时 tiptap 会把 `''` 变成 `'<p></p>'`,所以两种空必须视为相等,
 * 否则第一次输入就进死循环。
 */
export function needsContentSync(next: string, currentHtml: string): boolean {
  if (next === currentHtml) return false
  return !(isEmptyHtml(next) && isEmptyHtml(currentHtml))
}
