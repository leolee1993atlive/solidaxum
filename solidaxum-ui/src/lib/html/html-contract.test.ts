import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

/** 递归收集 src 下的 .ts/.svelte 源文件(排除测试文件)。 */
function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) {
      sourceFiles(p, out)
    } else if (
      (p.endsWith('.ts') || p.endsWith('.svelte')) &&
      !p.endsWith('.test.ts')
    ) {
      out.push(p)
    }
  }
  return out
}

/**
 * `{@html}` 护栏 —— 全仓只允许白名单内的两处。
 *
 * SafeHtml.svelte 是唯一走过 sanitizeHtml 的出口;在别处直接写 {@html} 等于绕开白名单,
 * 而公告正文是**任何有 system:notice:edit 权限的人**都能写的内容。
 *
 * GenPreviewDialog.svelte 是例外:内容是 hljs.highlight() 的确定性输出,
 * hljs 会把源码里的 `<`/`&` 转义后再包 `<span class="hljs-*">`,不会有可执行标签
 * —— 与 SafeHtml 面对的用户富文本不同,这边是库函数输出,不经过用户输入。
 */
const ALLOWLIST = [
  join('src', 'lib', 'components', 'SafeHtml.svelte'),
  join('src', 'lib', 'components', 'tool', 'GenPreviewDialog.svelte'),
]

describe('{@html} 契约', () => {
  it('只有白名单内的组件能用 {@html}', () => {
    const offenders: string[] = []
    for (const f of sourceFiles('src')) {
      if (ALLOWLIST.some((allowed) => f.endsWith(allowed))) continue
      const src = readFileSync(f, 'utf8')
      for (const [i, line] of src.split('\n').entries()) {
        if (line.includes('{@html')) {
          offenders.push(`${f}:${i + 1}: ${line.trim()}`)
        }
      }
    }
    expect(offenders).toEqual([])
  })

  it('SafeHtml.svelte 确实在用它(白名单不是空转)', () => {
    const src = readFileSync(ALLOWLIST[0], 'utf8')
    expect(src).toContain('{@html')
    expect(src).toContain('sanitizeHtml')
  })
})
