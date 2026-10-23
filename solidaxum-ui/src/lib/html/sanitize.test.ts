// @vitest-environment jsdom
//
// DOMPurify 依赖 NodeIterator 遍历 DOM 树来决定保留/删除哪些节点。
// happy-dom 20.x 的 NodeIterator 实现有 off-by-one 缺陷(会多遍历一次),
// 导致 DOMPurify 把 <h2>x</h2> 错误吃成 x。jsdom 的实现正确,所以本文件
// 以及所有直接/间接调用 sanitizeHtml 的测试都跑在 jsdom 下。
// sanitize.ts 的运行时逻辑在真实浏览器里是对的 —— 这是纯测试环境问题。
import { describe, expect, it } from 'vitest'
import { isSafeUrl, sanitizeHtml, stripHtml } from './sanitize.js'

describe('isSafeUrl', () => {
  it('http / https 两种都放行', () => {
    expect(isSafeUrl('https://a.com/x.png', 'src')).toBe(true)
    expect(isSafeUrl('http://a.com/x.png', 'src')).toBe(true)
  })

  it('站内相对路径放行', () => {
    expect(isSafeUrl('/system/notice', 'href')).toBe(true)
    expect(isSafeUrl('x.png', 'src')).toBe(true)
    expect(isSafeUrl('#anchor', 'href')).toBe(true)
  })

  it('mailto 只对 href 放行,img 的 src 不认', () => {
    expect(isSafeUrl('mailto:a@b.com', 'href')).toBe(true)
    expect(isSafeUrl('mailto:a@b.com', 'src')).toBe(false)
  })

  it('javascript / data / vbscript 一律拒', () => {
    expect(isSafeUrl('javascript:alert(1)', 'href')).toBe(false)
    expect(isSafeUrl('data:image/svg+xml;base64,PHN2Zz4=', 'src')).toBe(false)
    expect(isSafeUrl('vbscript:msgbox', 'href')).toBe(false)
  })

  it('用控制字符拆开协议名的绕法也拒', () => {
    expect(isSafeUrl('java\nscript:alert(1)', 'href')).toBe(false)
    expect(isSafeUrl('  JaVaScRiPt:alert(1)', 'href')).toBe(false)
  })

  it('空串不算合法地址', () => {
    expect(isSafeUrl('', 'href')).toBe(false)
    expect(isSafeUrl('   ', 'href')).toBe(false)
  })
})

describe('sanitizeHtml', () => {
  it('白名单内的标签原样保留', () => {
    const html =
      '<h2>标题</h2><p><strong>粗</strong><em>斜</em><u>下划</u><s>删</s></p><ul><li>甲</li></ul>'
    expect(sanitizeHtml(html)).toBe(html)
  })

  it('剥掉 script,连同它的文本', () => {
    expect(sanitizeHtml('<p>前</p><script>alert(1)</script>')).toBe('<p>前</p>')
  })

  it('剥掉事件属性', () => {
    const out = sanitizeHtml(
      '<img src="https://a.com/x.png" onerror="alert(1)">'
    )
    expect(out).not.toContain('onerror')
    expect(out).toContain('src="https://a.com/x.png"')
  })

  it('剥掉 javascript: 链接但留下文字', () => {
    const out = sanitizeHtml('<a href="javascript:alert(1)">点我</a>')
    expect(out).not.toContain('javascript:')
    expect(out).toContain('点我')
  })

  it('剥掉 data: 图片(D5 只允许 URL,不允许内嵌)', () => {
    expect(
      sanitizeHtml('<img src="data:image/svg+xml;base64,PHN2Zz4=">')
    ).not.toContain('data:')
  })

  it('target=_blank 一律补 rel=noopener noreferrer', () => {
    const out = sanitizeHtml('<a href="https://a.com" target="_blank">点</a>')
    expect(out).toContain('rel="noopener noreferrer"')
  })

  it('a 以外的标签不带任何属性', () => {
    expect(
      sanitizeHtml('<p title="x" class="y" style="color:red">文</p>')
    ).toBe('<p>文</p>')
  })

  it('img 只留白名单里那四个属性', () => {
    const out = sanitizeHtml(
      '<img src="https://a.com/x.png" alt="图" title="t" width="100" height="9" srcset="evil">'
    )
    expect(out).toContain('alt="图"')
    expect(out).toContain('width="100"')
    expect(out).not.toContain('height')
    expect(out).not.toContain('srcset')
  })

  it('站内相对路径与 mailto 保留', () => {
    expect(sanitizeHtml('<a href="/system/notice">内</a>')).toContain(
      'href="/system/notice"'
    )
    expect(sanitizeHtml('<a href="mailto:a@b.com">邮</a>')).toContain(
      'mailto:a@b.com'
    )
  })

  it('白名单外的标签丢掉但文字留下', () => {
    expect(sanitizeHtml('<table><tr><td>格</td></tr></table>')).toBe('格')
  })

  it('空输入给空串', () => {
    expect(sanitizeHtml('')).toBe('')
  })
})

describe('stripHtml', () => {
  it('去掉标签只留文字', () => {
    expect(stripHtml('<p><strong>公告</strong>正文</p>')).toBe('公告正文')
  })

  it('块级之间补空格,不把两段粘成一个词', () => {
    expect(stripHtml('<p>甲</p><p>乙</p>')).toBe('甲 乙')
    expect(stripHtml('<ul><li>甲</li><li>乙</li></ul>')).toBe('甲 乙')
  })

  it('实体解码成人读的字', () => {
    expect(stripHtml('<p>A &amp; B</p>')).toBe('A & B')
  })

  it('不留标签残片,script 的文本也不漏出来', () => {
    const out = stripHtml('<p>正</p><script>alert(1)</script>')
    expect(out).toBe('正')
    expect(out).not.toContain('<')
  })

  it('空输入给空串', () => {
    expect(stripHtml('')).toBe('')
  })
})
