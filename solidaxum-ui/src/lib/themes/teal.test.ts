import { describe, it, expect } from 'vitest'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  parseThemeBlock,
  readThemeCss,
  BASE_TOKENS,
  EXTENDED_TOKENS,
  DARK_REQUIRED_TOKENS,
} from './contract'
import { parseOklch, contrastRatio, type Oklch } from './contrast'
import { COLOR_THEMES, DEFAULT_COLOR_THEME, isValidColorTheme } from './registry'

const light = parseThemeBlock(readThemeCss('teal.css'), '.theme-teal')

/** 取令牌并断言它是可解析的 oklch —— 拿不到就让测试在此处失败,而不是 NaN 传染下去。 */
function color(tokens: Record<string, string>, name: string): Oklch {
  const raw = tokens[name]
  expect(raw, `令牌 ${name} 未定义`).toBeDefined()
  const c = parseOklch(raw)
  expect(c, `令牌 ${name} 不是 oklch:${raw}`).not.toBeNull()
  return c as Oklch
}

describe('registry 注册', () => {
  it('teal 已注册且为默认皮肤', () => {
    expect(COLOR_THEMES.map((t) => t.name)).toContain('teal')
    expect(DEFAULT_COLOR_THEME).toBe('teal')
    expect(isValidColorTheme('teal')).toBe(true)
  })

  it('原有三套皮肤仍保留,可回退对照', () => {
    const names = COLOR_THEMES.map((t) => t.name)
    expect(names).toContain('default')
    expect(names).toContain('de-swiss-design')
    expect(names).toContain('ibm-carbon')
  })

  it('teal 的 cssClass 与 applyToDocument 的约定一致', () => {
    expect(COLOR_THEMES.find((t) => t.name === 'teal')?.cssClass).toBe('theme-teal')
  })
})

describe('teal 亮色契约', () => {
  it('覆盖全部 BASE_TOKENS', () => {
    expect(BASE_TOKENS.filter((t) => !(t in light))).toEqual([])
  })

  it('覆盖全部 EXTENDED_TOKENS', () => {
    expect(EXTENDED_TOKENS.filter((t) => !(t in light))).toEqual([])
  })

  // 青绿按钮/输入框实测 4px。shadcn 的 button 与 input 都吃 rounded-md
  // = calc(--radius - 2px),故 --radius 必须是 0.375rem 才落到 4px。
  // 规格 §4.2 原写 0.25rem 并称「正好对上」,那是默认按钮吃 rounded-lg;
  // 实际不是,于是 4px 落在没人用的 lg 上,按钮/输入框只有 2px。
  it('按钮/输入框圆角落在青绿的 4px —— 由 rounded-md 反推 --radius', () => {
    expect(light['--radius']).toBe('0.375rem')
  })

  it('中文字体优先级已声明', () => {
    expect(light['--font-sans']).toContain('PingFang SC')
    expect(light['--font-sans']).toContain('Microsoft YaHei')
  })

  it('卡片级阴影为 none —— 层次靠描边而非阴影', () => {
    for (const t of ['--shadow-2xs', '--shadow-xs', '--shadow-sm', '--shadow', '--shadow-md']) {
      expect(light[t], `${t} 应为 none`).toBe('none')
    }
  })

  it('浮层阴影保留 —— 脱离文档流需要边界暗示', () => {
    for (const t of ['--shadow-lg', '--shadow-xl', '--shadow-2xl']) {
      expect(light[t], `${t} 不应为 none`).not.toBe('none')
    }
  })

  it('渐变令牌为真实渐变,不是 none', () => {
    expect(light['--gradient-shell']).toContain('linear-gradient')
    expect(light['--gradient-content']).toContain('linear-gradient')
    expect(light['--gradient-menu-active']).toContain('linear-gradient')
  })
})

describe('teal 亮色对比度', () => {
  const bg = () => color(light, '--background')
  const card = () => color(light, '--card')

  it.each([
    ['--foreground'],
    ['--primary-emphasis'],
    ['--muted-foreground'],
    ['--warning-emphasis'],
    ['--success-emphasis'],
  ])('%s 对 --background 与 --card 均达 AA 4.5:1', (token) => {
    expect(contrastRatio(color(light, token), bg())).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio(color(light, token), card())).toBeGreaterThanOrEqual(4.5)
  })

  it.each([['--chart-1'], ['--chart-2'], ['--chart-3'], ['--chart-4'], ['--chart-5']])(
    '%s 对 --background 达图形 3:1',
    (token) => {
      expect(contrastRatio(color(light, token), bg())).toBeGreaterThanOrEqual(3)
    }
  )

  // 以下两项是青绿原样保留的已知缺陷(见 Global Constraints)。
  // 钉死数值而非跳过检查,任何漂移都会让测试失败并被显式复核。
  it('【已知不达标】白字 on --primary = 2.93:1', () => {
    expect(
      contrastRatio(color(light, '--primary'), color(light, '--primary-foreground'))
    ).toBeCloseTo(2.93, 1)
  })

  it('【已知不达标】白字 on --destructive = 3.51:1', () => {
    expect(contrastRatio(color(light, '--destructive'), [1, 0, 0])).toBeCloseTo(3.51, 1)
  })
})

const dark = parseThemeBlock(readThemeCss('teal.css'), '.theme-teal.dark')

describe('teal 暗色契约', () => {
  it('暗色块存在', () => {
    expect(Object.keys(dark).length).toBeGreaterThan(0)
  })

  it('重新定义了全部 DARK_REQUIRED_TOKENS —— 少一个就会从亮色穿透', () => {
    expect(DARK_REQUIRED_TOKENS.filter((t) => !(t in dark))).toEqual([])
  })

  it('声明 color-scheme: dark', () => {
    expect(readThemeCss('teal.css')).toContain('color-scheme: dark')
  })
})

describe('teal 暗色的两处反转', () => {
  it('主色按钮极性反转:--primary-foreground 必须是暗色,不能沿用白字', () => {
    // 暗色下 --primary 提亮为 #25D0CD,白字对它仅 1.91:1。
    // 按钮必须是「亮青填充 + 近黑文字」。
    const fg = color(dark, '--primary-foreground')
    expect(fg[0], '--primary-foreground 的 L 应低于 0.35(近黑)').toBeLessThan(0.35)
    expect(
      contrastRatio(color(dark, '--primary'), fg)
    ).toBeGreaterThanOrEqual(4.5)
  })

  it('侧栏渐变退化:亮青绿实底在暗色下是刺眼色块', () => {
    expect(dark['--gradient-shell']).toContain('linear-gradient')
    expect(dark['--gradient-content']).toBe('none')
    expect(dark['--gradient-menu-active']).toBe('none')
  })
})

describe('teal 暗色对比度', () => {
  const bg = () => color(dark, '--background')
  const card = () => color(dark, '--card')

  it.each([
    ['--foreground'],
    ['--primary-emphasis'],
    ['--muted-foreground'],
    ['--warning-emphasis'],
    ['--success-emphasis'],
  ])(
    '%s 对 --background 与 --card 均达 AA 4.5:1',
    (token) => {
      expect(contrastRatio(color(dark, token), bg())).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(color(dark, token), card())).toBeGreaterThanOrEqual(4.5)
    }
  )

  it.each([['--chart-1'], ['--chart-2'], ['--chart-3'], ['--chart-4'], ['--chart-5']])(
    '%s 对 --background 达图形 3:1',
    (token) => {
      expect(contrastRatio(color(dark, token), bg())).toBeGreaterThanOrEqual(3)
    }
  )
})

describe('teal 外壳规则(侧栏皮层)', () => {
  const css = readThemeCss('teal.css')

  it('纹样挂在 sidebar-inner 而非容器层 —— 容器背景会被内层盖住', () => {
    const inner = parseThemeBlock(css, ".theme-teal:not(.dark) [data-slot='sidebar-inner']")
    expect(inner['background-image']).toBe("url('/sidebar-bg.png')")
    expect(inner['background-size']).toBe('cover')
    expect(inner['background-repeat']).toBe('no-repeat')
  })

  // 侧栏里 [data-slot=sidebar-menu-button] 的元素数是 0 —— 一级项经 Tooltip
  // 包裹后 data-slot 变成 tooltip-trigger,叶子项是 sidebar-menu-sub-button。
  // 原按 data-slot 写的选中胶囊规则从未命中,选中态一直是 shadcn 的平色 + 4px。
  it('选中胶囊用 data-sidebar 钩子,且带渐变与 8px(规格 §3.2)', () => {
    const rule = css.match(
      /\[data-sidebar='menu-sub-button'\]\[data-active='true'\][\s\S]{0,140}?\{([^}]*)\}/
    )
    expect(rule, '未找到选中胶囊规则').not.toBeNull()
    expect(rule![0]).toContain("[data-sidebar='menu-button'][data-active='true']")
    expect(rule![1]).toContain('background-image: var(--gradient-menu-active)')
    expect(rule![1]).toContain('border-radius: 8px')
  })

  it('不再按 data-slot 写菜单按钮规则 —— 那个选择器在本应用匹配不到元素', () => {
    expect(css).not.toContain("[data-slot='sidebar-menu-button']")
  })

  it('sidebar-shell.ts 与 JS 门控已移除(机制已迁移到 CSS)', () => {
    expect(existsSync(join(process.cwd(), 'src/lib/components/layout/sidebar-shell.ts'))).toBe(false)
  })
})

describe('teal 外壳几何(压平 shadcn inset 变体)', () => {
  const css = readThemeCss('teal.css')

  // 本应用侧栏默认 variant="inset"。规格 §5 的外壳按齐边布局写就,
  // 两者叠加时 shadcn 的 inset 装饰会逐一抵消三项招牌特征。
  // 前七个任务的测试全是源码字串扫描与令牌解析,谁都看不见"合成后的样子",
  // 于是外壳视觉全废而 CI 全绿。以下三条把压平规则钉死。

  it('wrapper 背景透明 —— 否则不透明 bg-sidebar 会盖掉 S1 的青绿渐变', () => {
    const w = parseThemeBlock(css, ".theme-teal [data-slot='sidebar-wrapper']")
    expect(w['background-color']).toBe('transparent')
  })

  it('inset 下侧栏容器去 p-2 —— 否则实绘宽 168px,184x1270 纹样被缩放', () => {
    const c = parseThemeBlock(
      css,
      ".theme-teal [data-slot='sidebar'][data-variant='inset'] [data-slot='sidebar-container']"
    )
    expect(c['padding']).toBe('0')
  })

  it('主区域去 m-2 与 rounded-xl —— 圆角只许留 Header 左上那一处(规格 §5.2)', () => {
    const i = parseThemeBlock(css, ".theme-teal [data-slot='sidebar-inset']")
    expect(i['margin']).toBe('0')
    expect(i['border-radius']).toBe('0')
  })
})

describe('卡片容器落实"底色反差"(规格 §3.4 / §4.1)', () => {
  // 青绿的层次感来自「底色反差 + 有色相的描边」而非阴影。
  // 描边此前已就位,但容器没有 bg-card,白卡片从未出现 ——
  // 表格与树面板直接坐在内容渐变上,反差为零,这正是"素"的观感来源。
  // 只认"卡片容器"这一种形态,不做全文件扫描 —— TreePanel 里的搜索框同样是
  // `rounded-md border`,但它是输入控件,拿的是 --input 描边,不该有 bg-card。
  // 青绿的内容区是一整块白色面板(线上实测 .bg-card.rounded-xl 1420x784,
  // 筛选、列表标题、表格、分页全在其内),不是几块浮在渐变上的碎片。
  it('CrudTableShell 是单张白卡:查询栏 + 工具栏 + 表格 + 分页同处一个 bg-card 内', () => {
    const text = readFileSync(
      join(process.cwd(), 'src/lib/components/crud/CrudTableShell.svelte'),
      'utf8'
    )
    // fit 视口适配:卡片根 class 走 cn(),fit 时追加 flex 高度链,bg-card 不变
    expect(text).toContain('data-slot="content-card"')
    expect(text).toContain("'bg-card rounded-lg border'")
    expect(text).toContain("fit && 'flex min-h-0 flex-1 flex-col'")
    // 查询栏独立成行,与下方工具栏/表格之间无分割线(层次靠底色反差)
    expect(text).toContain('data-testid="shell-query" class="p-3"')
    // 小标题与工具栏同行,与表格之间刻意不留分割线
    expect(text).toContain('data-testid="shell-toolbar"')
    expect(text).toContain('class="flex flex-wrap items-center gap-2 px-3 pt-3"')
    // 表格自带一层圆角描边,与外层白卡留出间距:加载态保持原样一处;
    // 数据态在 fit 时升级为纵向滚动区(仍是 m-3 圆角描边 + 粘性表头)
    expect(
      text.match(/<div class="m-3 overflow-hidden rounded-lg border">/g)
    ).toHaveLength(1)
    expect(text).toContain("'m-3 overflow-hidden rounded-lg border',")
    expect(
      text.includes('mb-0 min-h-0 flex-1 overflow-auto [&_thead]:sticky')
    ).toBe(true)
    // 表格已有边框,分页不再画 border-t
    expect(text).toContain('<div class="px-3 pb-3">')
    expect(text).not.toContain('border-t p-3')
  })

  it('树面板容器带 bg-card,其内的搜索框保持 --input 描边不受影响', () => {
    const text = readFileSync(
      join(process.cwd(), 'src/lib/components/TreePanel.svelte'),
      'utf8'
    )
    expect(text).toContain('data-slot="content-card"')
    expect(text).toContain("cn('bg-card shrink-0 rounded-lg border'")
    expect(text).toContain('border-input mb-2 h-8 w-full rounded-md border')
  })

  // 护栏。曾有一轮改动以「层次靠白底与渐变底的反差」为由把三个卡片
  // 容器的 border 直接删掉了 —— 但 --gradient-content 只在 .theme-teal 下有值,
  // 老皮肤在 _fallback.css 里兜底成 none;而 default / de-swiss-design 亮色下
  // --card 与 --background 同为 oklch(1 0 0),白卡坐在白底上、无描边无阴影,
  // 卡片边界彻底消失。
  // 正确做法是两头配对:类名里留 border 给老皮肤兜底,青绿在自己的皮肤 CSS 里
  // 抹平。这条测试把这一对锁在一起,任一头单独改动都会红。
  it('三个卡片容器必须带 border —— 有皮肤的 --card 与 --background 完全相同', () => {
    const identical = (
      [
        ['default.css', '.theme-default'],
        ['de-swiss-design.css', '.theme-de-swiss-design'],
      ] as const
    )
      .filter(([file, selector]) => {
        const t = parseThemeBlock(readThemeCss(file), selector)
        return t['--card'] && t['--card'] === t['--background']
      })
      .map(([file]) => file)
    // 前提还成立这条护栏才有意义;哪天皮肤令牌改了,这里会先失败提醒复核。
    expect(
      identical,
      '不再有 --card === --background 的皮肤,请复核本护栏'
    ).toEqual(['default.css', 'de-swiss-design.css'])

    for (const file of [
      'src/lib/components/crud/CrudTableShell.svelte',
      'src/lib/components/crud/TreeTableShell.svelte',
      'src/lib/components/TreePanel.svelte',
    ]) {
      const text = readFileSync(join(process.cwd(), file), 'utf8')
      expect(text, `${file} 的卡片容器丢了 border`).toMatch(
        /bg-card[^'"]*\bborder\b/
      )
      expect(
        text,
        `${file} 缺少 content-card 钩子,青绿抹不平这圈外框`
      ).toContain('data-slot="content-card"')
    }

    // 配对的另一头:青绿自己不要这圈外框。
    const rule = parseThemeBlock(
      readThemeCss('teal.css'),
      ".theme-teal [data-slot='content-card']"
    )
    expect(rule['border-color'], '青绿未抹平 content-card 外框').toBe(
      'transparent'
    )
  })

  // outline 按钮在青绿亮色下要跟白卡同底色。钩子走 data-variant,不是把
  // ui/button 的 bg-background 改写成 bg-card —— 那样下次重新生成组件就丢了。
  it('outline 按钮底色按主题作用域改写,不动 shadcn 原语', () => {
    const btn = readFileSync(
      join(process.cwd(), 'src/lib/components/ui/button/button.svelte'),
      'utf8'
    )
    // 正则而非 toContain:工作区是 CRLF,写死 \n 会永远匹配不上。
    expect(btn, 'outline 变体不该在原语里被改成 bg-card').toMatch(
      /outline:\s*'bg-background/
    )
    const hooks = btn.match(/data-variant=\{variant\}/g)
    expect(hooks, 'a 与 button 两个分支都要带钩子').toHaveLength(2)

    const rule = parseThemeBlock(
      readThemeCss('teal.css'),
      ".theme-teal:not(.dark) [data-slot='button'][data-variant='outline']"
    )
    expect(rule['background-color']).toBe('var(--card)')
  })

  // Input / Textarea 与 outline 按钮是同一个问题:原语用 bg-background,青绿亮色下
  // 那是 #F1F3F6 灰,坐在白卡里就是一格格灰底(流程设计器属性面板、个人设置页)。
  // 同样只在主题作用域覆盖,不改 ui/input、ui/textarea。
  it('Input / Textarea 亮色下跟白卡同底色,不动 shadcn 原语', () => {
    const input = readFileSync(
      join(process.cwd(), 'src/lib/components/ui/input/input.svelte'),
      'utf8'
    )
    expect(input, 'input 原语不该被改成 bg-card').toMatch(/bg-background/)
    const css = readThemeCss('teal.css')
    for (const slot of ['input', 'textarea']) {
      const rule = parseThemeBlock(
        css,
        `.theme-teal:not(.dark) [data-slot='${slot}']`
      )
      expect(rule['background-color'], `${slot} 未覆盖`).toBe('var(--card)')
    }
  })

  // 分段页签的当前项与 Input 是同一个问题:原语 bg-background 压在 bg-muted 上
  // 几乎同色,青绿又没有 shadow-sm 兜底。只在主题作用域覆盖,不改 ui/tabs。
  it('分段页签当前项亮色下跟白卡同底色,不动 shadcn 原语', () => {
    const trigger = readFileSync(
      join(process.cwd(), 'src/lib/components/ui/tabs/tabs-trigger.svelte'),
      'utf8'
    )
    expect(trigger, 'tabs-trigger 原语不该被改成 bg-card').toMatch(
      /data-\[state=active\]:bg-background/
    )
    const rule = parseThemeBlock(
      readThemeCss('teal.css'),
      ".theme-teal:not(.dark) [data-slot='tabs-trigger'][data-state='active']"
    )
    expect(rule['background-color']).toBe('var(--card)')
  })
})

describe('表格与筛选表单对齐青绿实测', () => {
  const css = readThemeCss('teal.css')

  it('表头淡青底复用 --header,字重 700(线上 #E4F4F5 / 700 / #606266)', () => {
    const th = parseThemeBlock(css, ".theme-teal [data-slot='table-head']")
    expect(th['background-color']).toBe('var(--header)')
    expect(th['font-weight']).toBe('700')
    expect(th['color']).toBe('var(--muted-foreground)')
  })

  // 钩子必须是 data-sidebar:目录项的 data-slot 被 Tooltip 改写成
  // tooltip-trigger,子项是 sidebar-menu-sub-button —— 按 data-slot 写
  // 一条都命不中(实测)。这条测试守的就是这个钩子。
  it('菜单项行高 38px,且钩子用 data-sidebar 而非 data-slot', () => {
    // 这是一条多选择器规则,parseThemeBlock 按单选择器取块,故直接匹原文
    // (顺带不受 CRLF 影响)。
    const rule = css.match(
      /\.theme-teal \[data-sidebar='menu-button'\][\s\S]{0,120}?\{([^}]*)\}/
    )
    expect(rule, '未找到 data-sidebar 菜单高度规则').not.toBeNull()
    expect(rule![0]).toContain("[data-sidebar='menu-sub-button']")
    expect(rule![1]).toContain('height: 38px')
    // 反向断言:不许退回按 data-slot 写 —— 那样三类节点一条都命不中
    expect(css).not.toMatch(/\.theme-teal \[data-slot='sidebar-menu-button'\]\s*\{/)
  })

  it('子菜单缩进收窄 —— 否则二级项只剩 55px,"日志管理"差 1px 被截断', () => {
    const sub = parseThemeBlock(css, ".theme-teal [data-slot='sidebar-menu-sub']")
    expect(sub['margin-inline']).toBe('0.5rem')
    expect(sub['padding-inline']).toBe('0.25rem')
  })

  it('筛选标签左置定宽 —— 线上是 80px 标签列 + 右侧输入,不是标签压在输入上方', () => {
    const qf = readFileSync(
      join(process.cwd(), 'src/lib/components/QueryForm.svelte'),
      'utf8'
    )
    expect(qf).not.toContain('flex flex-col gap-1 text-sm')
    // 5 处:dateRange/treeSelect/userSelect/multiSelect/segmented
    // (ST-1.6 新增后两型,同规格:定宽标签 + 右侧输入)
    expect(qf.match(/class="flex items-center gap-2 text-sm"/g)).toHaveLength(5)
    // 右对齐:定宽列左对齐时,「状态」这类两字标签离输入框有 50px 空档
    expect(qf.match(/w-20 shrink-0 text-right font-medium/g)).toHaveLength(6)
  })

  it('目录菜单标题显式 truncate —— 它不是 last-child,吃不到 shadcn 的截断', () => {
    const nav = readFileSync(
      join(process.cwd(), 'src/lib/components/layout/NavTreeItem.svelte'),
      'utf8'
    )
    expect(nav).toContain('<span class="truncate">{node.title}</span>')
  })
})

describe('本轮观感修正(2026-08-04 用户逐条反馈)', () => {
  const css = readThemeCss('teal.css')

  // 定稿口径:描边由「结构 #B1C4CD + 表单 #C9D7DE 淡一档」两档合一,
  // 统一为 #E4E4E7 中性浅灰。分两档原本是为了压住 #B1C4CD 亮度下并排输入框
  // 连成格子的问题,换到比原表单描边还淡的 #E4E4E7 后该问题不复存在。
  it('亮色描边统一 #E4E4E7,结构与表单不再分档', () => {
    expect(light['--border']).toBe('oklch(0.9197 0.004 286.32)')
    expect(light['--input']).toBe(light['--border'])
    // 反向断言:别再退回蓝灰。#E4E4E7 的 C 是 0.004,原 #B1C4CD 是 0.0244。
    const [, chroma] = parseOklch(light['--border'])!
    expect(chroma, '描边应是中性灰,不该重新带上蓝相').toBeLessThan(0.01)
  })

  it('暗色描边不跟着改浅 —— 浅灰铺到暗底上是刺眼亮边', () => {
    expect(dark['--border']).toBe('oklch(0.3344 0.0069 286.02)')
    expect(dark['--input']).toBe(dark['--border'])
  })

  it('二三级菜单未选中项的图标翻成浅色', () => {
    const rule = css.match(
      /\[data-sidebar='menu-sub-button'\][^{]*>\s*svg\s*\{([^}]*)\}/
    )
    expect(rule, '未找到子项图标翻色规则').not.toBeNull()
    expect(rule![1]).toContain('color: var(--sidebar-foreground)')
  })

  // 选中/悬停/按下三种态下 shadcn 都会把底色切成淡青的 bg-sidebar-accent,
  // 文字随之变深青。翻色规则必须把这三种态全排掉,否则白图标落在浅底上看不见
  // ——「悬停时字变深、图标还是白」正是漏排 :hover 的表现。
  it.each([[":not([data-active='true'])"], [':not(:hover)'], [':not(:active)']])(
    '翻色规则排除了 %s',
    (guard) => {
      const rule = css.match(
        /\[data-sidebar='menu-sub-button'\][^{]*>\s*svg\s*\{/
      )!
      expect(rule[0]).toContain(guard)
    }
  )
})
