import { parseThemeBlock } from '$lib/themes/contract.js'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// happy-dom 不算样式,canvas.css 的规则只能按源码断言(同 themes/teal.test 的做法)。
const css = readFileSync(
  join(process.cwd(), 'src/lib/components/flow-designer/canvas.css'),
  'utf8'
)

/** 找到选择器列表里含 needle 的那个规则块,返回其声明文本;找不到返回 null。 */
function blockContaining(needle: string): string | null {
  const stripped = css.replace(/\/\*[\s\S]*?\*\//g, '')
  for (const m of stripped.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
    if (m[1].includes(needle)) return m[2]
  }
  return null
}

describe('句柄显隐(canvas.css)', () => {
  it('句柄默认隐身,但盒子保留(opacity 而非 display)', () => {
    const rule = parseThemeBlock(css, '.fd-canvas .svelte-flow__handle')
    expect(rule['opacity']).toBe('0')
    expect(rule['display']).toBeUndefined()
    expect(rule['visibility']).toBeUndefined()
  })

  it.each([
    ['.svelte-flow__node.connectable:hover .svelte-flow__handle'],
    ['.svelte-flow__node.connectable.selected .svelte-flow__handle'],
    ['.svelte-flow__handle.connectingfrom'],
    ['.svelte-flow__handle.connectingto'],
  ])('%s 显形', (selector) => {
    const body = blockContaining(selector)
    expect(body, `没有针对 ${selector} 的规则`).not.toBeNull()
    expect(body).toMatch(/opacity:\s*1\s*;/)
  })

  // 只读态(readonly 设计器 / 实例流程图)节点没有 connectable 类,不该有任何
  // 不带 connectable 的悬停显形规则把句柄露出来。
  it('悬停显形只对可连接节点生效', () => {
    const stripped = css.replace(/\/\*[\s\S]*?\*\//g, '')
    for (const m of stripped.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
      const selectors = m[1].split(',').map((s) => s.trim())
      for (const sel of selectors) {
        if (sel.includes(':hover') && sel.includes('svelte-flow__handle')) {
          expect(sel, `悬停规则缺 connectable 限定: ${sel}`).toContain(
            '.connectable'
          )
        }
      }
    }
  })
})

describe('WP6:落线高亮 / 重连锚 / 面板(canvas.css)', () => {
  it('候选边高亮用主色加粗', () => {
    const rule = parseThemeBlock(
      css,
      '.svelte-flow__edge-path.fd-skip.fd-drop-target'
    )
    expect(rule['stroke']).toBe('#2563eb')
    expect(rule['stroke-width']).toBe('3')
  })

  it('重连锚是圆点,悬停只加光晕不动 transform(EdgeLabel 用 transform 定位)', () => {
    const anchor = parseThemeBlock(css, '.fd-canvas .fd-reconnect-anchor')
    expect(anchor['border-radius']).toBe('50%')
    expect(anchor['cursor']).toBe('move')
    const hover = parseThemeBlock(css, '.fd-canvas .fd-reconnect-anchor:hover')
    expect(hover['transform']).toBeUndefined()
    expect(hover['box-shadow']).toBeTruthy()
  })
})

describe('WP6:落线高亮 / 重连锚 / 面板(canvas.css)', () => {
  it('面板固定 128px 宽,项是抓手光标', () => {
    expect(parseThemeBlock(css, '.fd-palette')['width']).toBe('128px')
    expect(parseThemeBlock(css, '.fd-palette-item')['cursor']).toBe('grab')
  })
})

describe('设计态语义色(1.8.9 视觉移植,canvas.css)', () => {
  it('.fd-canvas 定义六个语义色变量与选中强调色', () => {
    const rule = parseThemeBlock(css, '.fd-canvas')
    expect(rule['--fd-start']).toBe('103, 194, 58')
    expect(rule['--fd-between']).toBe('64, 158, 255')
    expect(rule['--fd-end']).toBe('245, 108, 108')
    expect(rule['--fd-serial']).toBe('245, 158, 11')
    expect(rule['--fd-parallel']).toBe('19, 194, 194')
    expect(rule['--fd-inclusive']).toBe('146, 84, 222')
    expect(rule['--fd-accent']).toBe('37, 99, 235')
  })

  it.each([
    ['.fd-gateway.fd-serial', '--fd-serial'],
    ['.fd-gateway.fd-parallel', '--fd-parallel'],
    ['.fd-gateway.fd-inclusive', '--fd-inclusive'],
  ])('%s polygon 语义色描边 + 淡色填充', (selector, variable) => {
    const rule = parseThemeBlock(css, `${selector} polygon`)
    expect(rule['stroke']).toBe(`rgb(var(${variable}))`)
    expect(rule['fill']).toBe(`rgba(var(${variable}), 0.06)`)
  })

  it('start 内核 / end 内环用 ::after 实现', () => {
    const core = parseThemeBlock(css, '.fd-circle::after')
    expect(core['background']).toContain('radial-gradient')
    expect(core['background']).toContain('--fd-start')
    const ring = parseThemeBlock(css, '.fd-end::after')
    expect(ring['border']).toBe('1px solid rgba(var(--fd-end), 0.25)')
    // end 的内环必须盖掉 start 内核:两规则同特异性,.fd-end::after 靠源码顺序取胜
    expect(css.indexOf('.fd-end::after')).toBeGreaterThan(
      css.indexOf('.fd-circle::after')
    )
  })

  it('between 卡片语义描边 + 顶部光泽条 ::before', () => {
    const card = parseThemeBlock(css, '.fd-between')
    expect(card['border-color']).toBe('rgb(var(--fd-between))')
    const sheen = parseThemeBlock(css, '.fd-between::before')
    expect(sheen['background']).toContain('--fd-between')
  })

  it('边与标签胶囊柔化:边 slate-400、标签边框 slate-200', () => {
    expect(parseThemeBlock(css, '.svelte-flow__edge-path.fd-skip')['stroke']).toBe(
      '#94a3b8'
    )
    expect(parseThemeBlock(css, '.fd-edge-label')['border']).toBe(
      '1px solid #e2e8f0'
    )
  })

  it('网关符号改由 path/circle 规则上色,不再有 text 规则', () => {
    expect(parseThemeBlock(css, '.fd-gateway .fd-glyph path')['stroke']).toBe(
      'currentColor'
    )
    expect(css).not.toContain('.fd-gateway text')
  })

  it('实例流程图状态规则在语义规则之后(同特异性时源码顺序决定优先级)', () => {
    const stripped = css.replace(/\/\*[\s\S]*?\*\//g, '')
    const semantic = stripped.indexOf('.fd-gateway.fd-serial polygon')
    const status = stripped.indexOf('.fd-gateway.fd-status-done polygon')
    expect(semantic).toBeGreaterThan(-1)
    expect(status).toBeGreaterThan(semantic)
  })

  it('状态节点规则特异性高于定义态语义规则(节点描边)', () => {
    // .fd-node.fd-status-* (0,2,0) 天然高于 .fd-circle/.fd-between (0,1,0),
    // 实例流程图(ApprovalRecord,FlowChart.svelte)的三原色不被语义色覆盖
    expect(
      parseThemeBlock(css, '.fd-node.fd-status-done')['border-color']
    ).toBe('rgb(var(--fd-done))')
    expect(
      parseThemeBlock(css, '.fd-node.fd-status-todo')['border-style']
    ).toBe('dashed')
  })
})
