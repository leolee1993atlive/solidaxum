// @vitest-environment jsdom
//
// 本测试调 sanitizeHtml(DOMPurify),happy-dom 下输出错乱,故切 jsdom。
// 理由见 src/lib/html/sanitize.test.ts 顶部注释。
import { TOOLBAR_ITEMS, URL_TOOLBAR_ITEMS } from '$lib/components/rich-text.js'
import { describe, expect, it } from 'vitest'
import { ALLOWED_TAGS, sanitizeHtml } from './sanitize.js'

/**
 * 每个标签的最小可渲染样例 —— 消毒器吃的是完整 HTML,不是裸标签名。
 * **工具栏加功能时这张表必须同步加行**,否则下面第一条用例会红。
 */
const SAMPLE: Record<string, string> = {
  p: '<p>正文</p>',
  br: '<p>上<br>下</p>',
  strong: '<p><strong>粗</strong></p>',
  em: '<p><em>斜</em></p>',
  u: '<p><u>下划</u></p>',
  s: '<p><s>删除</s></p>',
  h1: '<h1>一级</h1>',
  h2: '<h2>二级</h2>',
  h3: '<h3>三级</h3>',
  ul: '<ul><li>项</li></ul>',
  ol: '<ol><li>项</li></ol>',
  li: '<ul><li>项</li></ul>',
  blockquote: '<blockquote><p>引</p></blockquote>',
  hr: '<hr>',
  a: '<p><a href="https://example.com">链</a></p>',
  img: '<img src="https://example.com/a.png" alt="图">',
}

/** 由 StarterKit 的基础节点产出、不挂在任何工具栏按钮上的结构标签。 */
const STRUCTURAL = ['p', 'br']

const producedTags = [
  ...new Set([...TOOLBAR_ITEMS, ...URL_TOOLBAR_ITEMS].flatMap((i) => i.tags)),
]

describe('编辑器产出 ⊆ 渲染白名单', () => {
  it('工具栏产出的每个标签都有样例(加功能时同步加 SAMPLE 行)', () => {
    expect(producedTags.filter((t) => !(t in SAMPLE))).toEqual([])
  })

  it.each(producedTags)('%s 过完 sanitizeHtml 仍在', (tag) => {
    expect(sanitizeHtml(SAMPLE[tag])).toContain(`<${tag}`)
  })

  it('白名单里没有工具栏产不出、也不是结构标签的多余项', () => {
    const extra = [...ALLOWED_TAGS].filter(
      (t) => !producedTags.includes(t) && !STRUCTURAL.includes(t)
    )
    expect(extra).toEqual([])
  })

  it('结构标签也在白名单里(少了正文会整段变裸文本)', () => {
    for (const tag of STRUCTURAL) {
      expect([...ALLOWED_TAGS]).toContain(tag)
    }
  })

  it('D7 明确不做的标签不在白名单里', () => {
    for (const tag of [
      'table',
      'tr',
      'td',
      'code',
      'pre',
      'span',
      'font',
      'div',
    ]) {
      expect([...ALLOWED_TAGS]).not.toContain(tag)
    }
  })

  it('链接的 href 与图片的 src 都能活着出来(否则等于功能不可用)', () => {
    expect(sanitizeHtml(SAMPLE.a)).toContain('href="https://example.com"')
    expect(sanitizeHtml(SAMPLE.img)).toContain(
      'src="https://example.com/a.png"'
    )
  })
})
