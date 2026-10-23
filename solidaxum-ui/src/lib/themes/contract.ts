import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/**
 * 每套皮肤的亮色块都必须定义的令牌 —— shadcn 基线集合。
 * 现有三套皮肤已全部满足,新增皮肤同样受约束。
 */
export const BASE_TOKENS = [
  '--radius',
  '--background',
  '--foreground',
  '--card',
  '--card-foreground',
  '--popover',
  '--popover-foreground',
  '--primary',
  '--primary-foreground',
  '--secondary',
  '--secondary-foreground',
  '--muted',
  '--muted-foreground',
  '--accent',
  '--accent-foreground',
  '--destructive',
  '--border',
  '--input',
  '--ring',
  '--chart-1',
  '--chart-2',
  '--chart-3',
  '--chart-4',
  '--chart-5',
  '--sidebar',
  '--sidebar-foreground',
  '--sidebar-primary',
  '--sidebar-primary-foreground',
  '--sidebar-accent',
  '--sidebar-accent-foreground',
  '--sidebar-border',
  '--sidebar-ring',
] as const

/**
 * 本次为青绿皮肤扩容的新令牌。老皮肤不定义它们,由 _fallback.css 的
 * :root / .dark 兜底,因此这三处必须齐全。
 */
export const EXTENDED_TOKENS = [
  '--primary-emphasis',
  '--header',
  '--success',
  '--success-foreground',
  '--success-emphasis',
  '--warning',
  '--warning-foreground',
  '--warning-emphasis',
  '--info',
  '--info-foreground',
  '--gradient-shell',
  '--gradient-content',
  '--gradient-menu-active',
] as const

/**
 * 暗色块必须重新定义的令牌。少了任何一个都会从亮色块穿透过来,
 * 造成暗色模式下出现亮色色块 —— 这是主题最容易出的一类 bug。
 */
export const DARK_REQUIRED_TOKENS = [
  '--background',
  '--foreground',
  '--card',
  '--card-foreground',
  '--popover',
  '--popover-foreground',
  '--primary',
  '--primary-foreground',
  '--primary-emphasis',
  '--muted',
  '--muted-foreground',
  '--accent',
  '--accent-foreground',
  '--border',
  '--input',
  '--header',
  '--warning-emphasis',
  '--success-emphasis',
  '--sidebar',
  '--sidebar-foreground',
  '--sidebar-accent',
  '--sidebar-accent-foreground',
  '--gradient-shell',
  '--gradient-content',
  '--gradient-menu-active',
] as const

/** 读取相对本文件所在目录的文件内容。测试在 node 环境执行,可直接用 fs。 */
export function readThemeCss(file: string): string {
  return readFileSync(fileURLToPath(new URL(file, import.meta.url)), 'utf8')
}

/**
 * 从 CSS 文本里抽出某个选择器块内的声明 —— 自定义属性与普通属性都保留
 * (外壳规则断言的是 background-* 等普通属性,不能只留 `--` 前缀)。
 *
 * 用块边界精确匹配而非宽松查找:`.theme-x` 的正则以 `\s*\{` 收尾,
 * 因此不会误命中 `.theme-x.dark {`。
 *
 * 先剥掉块注释:声明之间的 `/* ... *\/` 会让按 `;` 切分出的下一条声明
 * 带上注释前缀,污染 key。
 */
export function parseThemeBlock(
  css: string,
  selector: string
): Record<string, string> {
  const stripped = css.replace(/\/\*[\s\S]*?\*\//g, '')
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const re = new RegExp(`(?:^|\\})\\s*${escaped}\\s*\\{([^}]*)\\}`, 'm')
  const m = stripped.match(re)
  if (!m) return {}

  const out: Record<string, string> = {}
  for (const decl of m[1].split(';')) {
    const i = decl.indexOf(':')
    if (i < 0) continue
    const key = decl.slice(0, i).trim()
    if (!key) continue
    // 跨行值折叠成单行,便于断言
    out[key] = decl.slice(i + 1).trim().replace(/\s+/g, ' ')
  }
  return out
}
