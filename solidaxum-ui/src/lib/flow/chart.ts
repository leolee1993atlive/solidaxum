/**
 * 实例流程图的状态着色(规格 §10.2)。
 * status 来自 query-flow-chart:0 未到 / 1 进行中 / 2 已完成(engine/chart.rs);
 * chartStatusColor 是 [done, todo, notDone] 三个 "r,g,b" 串,缺省值与后端一致。
 */

export const DEFAULT_STATUS_COLORS = [
  '56,161,105',
  '245,158,11',
  '107,114,128',
] as const

/** 节点/边的状态 class;status 不是数字(定义态)时为空串,不着色 */
export function statusClass(
  status: number | string | null | undefined
): string {
  const n = typeof status === 'string' ? Number(status) : status
  if (n === 2) return 'fd-status-done'
  if (n === 1) return 'fd-status-todo'
  if (n === 0) return 'fd-status-notdone'
  return ''
}

const RGB = /^\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*$/

/**
 * 画布容器的内联样式:把三原色写成 CSS 变量,canvas.css 里的状态样式读它们。
 * 后端没给或格式不对时用缺省色,不让一条坏数据把整张图画成黑白。
 */
export function statusColorStyle(
  colors: readonly string[] | null | undefined
): string {
  const pick = (i: number) => {
    const c = colors?.[i]
    return c && RGB.test(c) ? c.replace(/\s+/g, '') : DEFAULT_STATUS_COLORS[i]
  }
  return `--fd-done:${pick(0)};--fd-todo:${pick(1)};--fd-notdone:${pick(2)}`
}
