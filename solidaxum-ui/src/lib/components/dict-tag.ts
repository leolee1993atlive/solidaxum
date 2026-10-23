/**
 * RuoYi `sys_dict_data.list_class` → tailwind class / 内联样式。
 *
 * 值域两族:
 * 1. 语义键 —— ry_vue.sql 的 6 个(primary/success/info/warning/danger/default)
 *    + antd Tag 同名色名透传键(blue/cyan/green/orange/purple/red/pink,
 *    旧版 renderDict 把这些键原样透传给 antd Tag)。
 * 2. 十六进制色 —— 业务字典可把 `#fa8c16` 这类 hex 直接写进 list_class,
 *    旧前端 antd Tag 支持任意 hex color。此处转 antd Tag 观感的内联样式
 *    (10% 透明底 + 实色字 + 35% 透明描边)。
 *
 * 为什么不走 shadcn `Badge` 的 variant:`badge.svelte` 只有
 * default/secondary/destructive/outline 四种,表达不了这套色。统一用
 * `variant="outline"` 再用这里的 class/内联样式覆盖配色,避免改动 Badge 本身。
 */
export const DICT_TAG_CLASS: Record<string, string> = {
  primary: 'border-transparent bg-blue-100 text-blue-800 dark:bg-blue-950 dark:text-blue-300',
  success: 'border-transparent bg-green-100 text-green-800 dark:bg-green-950 dark:text-green-300',
  info: 'border-transparent bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
  warning: 'border-transparent bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-300',
  danger: 'border-transparent bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300',
  default: 'border-transparent bg-zinc-100 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300',
  blue: 'border-transparent bg-blue-100 text-blue-800 dark:bg-blue-950 dark:text-blue-300',
  cyan: 'border-transparent bg-cyan-100 text-cyan-800 dark:bg-cyan-950 dark:text-cyan-300',
  green: 'border-transparent bg-green-100 text-green-800 dark:bg-green-950 dark:text-green-300',
  orange: 'border-transparent bg-orange-100 text-orange-800 dark:bg-orange-950 dark:text-orange-300',
  purple: 'border-transparent bg-purple-100 text-purple-800 dark:bg-purple-950 dark:text-purple-300',
  red: 'border-transparent bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300',
  pink: 'border-transparent bg-pink-100 text-pink-800 dark:bg-pink-950 dark:text-pink-300',
}

const HEX_RE = /^#[0-9a-fA-F]{6}$/

/** 纯函数:listClass → class 串。未知值返回空串(调用方据此退化为纯文本)。 */
export function resolveDictTagClass(listClass?: string | null): string {
  if (!listClass) return ''
  return DICT_TAG_CLASS[listClass] ?? ''
}

/**
 * 纯函数:listClass 为 6 位 hex 时 → antd Tag 观感的内联样式串
 * (hex+`1A` ≈10% 透明底 / hex 实色字 / hex+`59` ≈35% 透明描边);
 * 否则 null(调用方走语义类分支)。
 */
export function resolveDictTagInlineStyle(
  listClass?: string | null
): string | null {
  if (!listClass) return null
  const hex = listClass.toLowerCase()
  if (!HEX_RE.test(hex)) return null
  return `background-color:${hex}1a;color:${hex};border-color:${hex}59`
}
