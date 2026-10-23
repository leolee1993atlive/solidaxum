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
