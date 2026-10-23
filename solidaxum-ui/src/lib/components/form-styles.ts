/**
 * 原生表单控件的共享样式常量。
 *
 * 优先使用 ui/ 组件(Input / Textarea / Select);这些常量只给仍依赖
 * 原生 <select> bind:value 简单语义的场景兜底,视觉对齐 ui/select
 * Trigger(shadcn v4:shadow-xs + ring-[3px] ring-ring/50)。
 * 不要在组件里再手抄一份——改样式只改这里。
 */

/** 标准高度(h-9)原生 select 样式,对齐 ui/select Trigger 视觉。 */
export const SELECT_CLASS =
  'flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50'
