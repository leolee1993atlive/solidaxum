/** oklch 三元组:[L(0..1), C, H(度)]。不含 alpha —— 对比度计算按不透明处理。 */
export type Oklch = [number, number, number]

const clamp01 = (v: number) => Math.min(1, Math.max(0, v))

/**
 * 从 CSS 值里取出 oklch 三元组。非 oklch(如 `none`、渐变、`var(...)`)返回 null,
 * 让调用方能区分"没配"和"配了但不是颜色"。
 */
export function parseOklch(value: string): Oklch | null {
  const m = value.trim().match(/^oklch\(\s*([^)]+)\)$/i)
  if (!m) return null
  // 先切掉 alpha(`/ 40%`),再按空白拆前三个分量
  const parts = m[1].split('/')[0].trim().split(/\s+/)
  if (parts.length < 3) return null
  const nums = parts.slice(0, 3).map(Number)
  if (nums.some((n) => Number.isNaN(n))) return null
  return nums as Oklch
}

/** oklch → 线性 sRGB(未做 gamma 编码,超出色域的分量按 0..1 截断)。 */
function oklchToLinearSrgb([L, C, H]: Oklch): [number, number, number] {
  const h = (H * Math.PI) / 180
  const a = C * Math.cos(h)
  const b = C * Math.sin(h)

  const l_ = L + 0.3963377774 * a + 0.2158037573 * b
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b
  const s_ = L - 0.0894841775 * a - 1.291485548 * b

  const l = l_ ** 3
  const m = m_ ** 3
  const s = s_ ** 3

  return [
    clamp01(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    clamp01(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    clamp01(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ]
}

/**
 * WCAG 相对亮度。WCAG 定义的输入就是线性化后的 sRGB 分量,
 * 而 oklch→linear sRGB 的产物正是线性值,故此处直接加权即可,无需再反 gamma。
 */
export function relativeLuminance(c: Oklch): number {
  const [r, g, b] = oklchToLinearSrgb(c)
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** WCAG 对比度,值域 1..21,与传入顺序无关。 */
export function contrastRatio(a: Oklch, b: Oklch): number {
  const la = relativeLuminance(a)
  const lb = relativeLuminance(b)
  const hi = Math.max(la, lb)
  const lo = Math.min(la, lb)
  return (hi + 0.05) / (lo + 0.05)
}
