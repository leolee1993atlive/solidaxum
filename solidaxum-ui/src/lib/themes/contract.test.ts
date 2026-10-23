import { describe, it, expect } from 'vitest'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

/** src/ 的绝对路径。与仓库内其他护栏测试一致按 process.cwd() 取 ——
 *  vitest 经 `npm test` 在包根启动;从别处手动跑 vitest 时 cwd 会漂,
 *  扫描根跟着漂(那样连测试文件本身都收集不到,不会假绿)。 */
const SRC_ROOT = join(process.cwd(), 'src')
import {
  parseThemeBlock,
  readThemeCss,
  BASE_TOKENS,
  EXTENDED_TOKENS,
} from './contract'

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

describe('parseThemeBlock', () => {
  const css = `
.theme-x {
  --background: oklch(1 0 0);
  --radius: 0.25rem;
  --font-sans:
    ui-sans-serif, system-ui;
}

.theme-x.dark {
  --background: oklch(0.2 0 0);
}
`

  it('提取指定块内的自定义属性', () => {
    const t = parseThemeBlock(css, '.theme-x')
    expect(t['--background']).toBe('oklch(1 0 0)')
    expect(t['--radius']).toBe('0.25rem')
  })

  it('跨行值被折叠为单行', () => {
    const t = parseThemeBlock(css, '.theme-x')
    expect(t['--font-sans']).toBe('ui-sans-serif, system-ui')
  })

  it('.theme-x 不会误匹配到 .theme-x.dark', () => {
    expect(parseThemeBlock(css, '.theme-x')['--background']).toBe('oklch(1 0 0)')
    expect(parseThemeBlock(css, '.theme-x.dark')['--background']).toBe('oklch(0.2 0 0)')
  })

  it('块不存在时返回空对象', () => {
    expect(parseThemeBlock(css, '.theme-nope')).toEqual({})
  })
})

describe('现有三套皮肤覆盖 BASE 契约', () => {
  for (const file of ['default.css', 'de-swiss-design.css', 'ibm-carbon.css']) {
    const name = file.replace('.css', '')
    it(`${name} 亮色块定义了全部 BASE_TOKENS`, () => {
      const tokens = parseThemeBlock(readThemeCss(file), `.theme-${name}`)
      const missing = BASE_TOKENS.filter((t) => !(t in tokens))
      expect(missing).toEqual([])
    })
  }
})

describe('EXTENDED 令牌由 _fallback.css 兜底', () => {
  it(':root 定义了全部 EXTENDED_TOKENS', () => {
    const tokens = parseThemeBlock(readThemeCss('_fallback.css'), ':root')
    const missing = EXTENDED_TOKENS.filter((t) => !(t in tokens))
    expect(missing).toEqual([])
  })

  it('.dark 定义了全部 EXTENDED_TOKENS', () => {
    const tokens = parseThemeBlock(readThemeCss('_fallback.css'), '.dark')
    const missing = EXTENDED_TOKENS.filter((t) => !(t in tokens))
    expect(missing).toEqual([])
  })

  it('三个渐变令牌在兜底里为 none —— 老皮肤下外壳退化为纯色', () => {
    const tokens = parseThemeBlock(readThemeCss('_fallback.css'), ':root')
    expect(tokens['--gradient-shell']).toBe('none')
    expect(tokens['--gradient-content']).toBe('none')
    expect(tokens['--gradient-menu-active']).toBe('none')
  })
})

describe('级联顺序', () => {
  it('_fallback.css 的导入早于所有具名皮肤,否则会反向覆盖它们', () => {
    const layout = readThemeCss('../../routes/layout.css')
    const fallbackAt = layout.indexOf('_fallback.css')
    expect(fallbackAt).toBeGreaterThan(-1)
    for (const file of ['default.css', 'de-swiss-design.css', 'ibm-carbon.css']) {
      expect(fallbackAt).toBeLessThan(layout.indexOf(file))
    }
  })
})

describe('滚动条', () => {
  const layout = () =>
    readThemeCss('../../routes/layout.css').replace(/\/\*[\s\S]*?\*\//g, '')

  // 这条护栏守的是一个反复踩的坑:Chrome 121 起,元素上只要出现标准属性
  // scrollbar-width / scrollbar-color,同一元素的 ::-webkit-scrollbar-*
  // 伪元素就被整体忽略。写在 `*` 上就等于把 6px 宽、去箭头、圆角、悬停显形
  // 全部作废,Windows Chrome 回落到自带上下箭头的标准 thin 滚动条。
  // (Chrome 148 实测:webkit 单独用 gutter 6px,叠加
  //  scrollbar-width:thin 后 10px。)
  it('基础 * 规则里不许出现 scrollbar-width / scrollbar-color', () => {
    const base = layout().match(/@layer base \{\s*\*\s*\{([^}]*)\}/)
    expect(base, '未找到 @layer base 里的 * 规则').not.toBeNull()
    expect(base![1]).not.toMatch(/scrollbar-(width|color)/)
  })

  it('标准属性只发给不认 webkit 伪元素的浏览器(Firefox)', () => {
    const block = layout().match(
      /@supports not selector\(::-webkit-scrollbar\)\s*\{([\s\S]*?)\n {2}\}/
    )
    expect(
      block,
      '缺少 @supports not selector(::-webkit-scrollbar) 分流'
    ).not.toBeNull()
    expect(block![1]).toMatch(/scrollbar-width:\s*thin/)
    expect(block![1]).toMatch(/scrollbar-color:/)
  })

  it('去掉两端箭头按钮', () => {
    expect(layout()).toMatch(
      /\*::-webkit-scrollbar-button\s*\{\s*display:\s*none;?\s*\}/
    )
  })

  // 原生滚动条无法悬浮(overflow:overlay 已在 Chrome 121 移除,实测 computed
  // 值就是 auto)。6px 槽位常驻 + sidebar-group 右内边距扣掉同样的 6px,
  // 菜单项实绘宽度才不会随滚动条出现而变窄、选中胶囊才不会跟着缩。
  it('侧栏滚动条槽位常驻,且由 sidebar-group 的右内边距让位', () => {
    const css = layout()
    expect(css).toMatch(
      /\[data-slot='sidebar-content'\]\s*\{[^}]*scrollbar-gutter:\s*stable/
    )
    expect(css).toMatch(
      /\[data-slot='sidebar-content'\]\s*\[data-slot='sidebar-group'\]\s*\{[^}]*padding-right:\s*calc\(0\.5rem - 6px\)/
    )
  })

  // overflow:overlay 是非标准值,Chrome 121 已移除、现在只是 auto 的别名
  // (Chrome 148 实测:computed 值 auto,gutter 照占 15px),
  // Firefox 从未支持。写了等于没写,别再拿它当「悬浮滚动条」用。
  it('全仓不再使用已被移除的 overflow:overlay', () => {
    const stripComments = (s: string) =>
      s
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/<!--[\s\S]*?-->/g, '')
        .replace(/^\s*\/\/.*$/gm, '')
    const hits = sourceFiles(SRC_ROOT)
      .concat(join(SRC_ROOT, 'routes/layout.css'))
      .filter((f) =>
        /overflow[-a-z]*:\s*overlay/.test(
          stripComments(readFileSync(f, 'utf8'))
        )
      )
    expect(hits, `overflow:overlay 是死代码: ${hits.join(', ')}`).toEqual([])
  })
})
