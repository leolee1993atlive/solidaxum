import { describe, expect, it } from 'vitest'
import {
  TOOLBAR_ITEMS,
  URL_TOOLBAR_ITEMS,
  isEmptyHtml,
  needsContentSync,
} from './rich-text.js'

describe('isEmptyHtml', () => {
  it('空串与 tiptap 的空文档都算空', () => {
    expect(isEmptyHtml('')).toBe(true)
    expect(isEmptyHtml('   ')).toBe(true)
    expect(isEmptyHtml('<p></p>')).toBe(true)
  })

  it('有字就不算空', () => {
    expect(isEmptyHtml('<p>甲</p>')).toBe(false)
  })
})

describe('needsContentSync', () => {
  it('完全相同不同步 —— 否则每敲一个字光标就跳回开头', () => {
    expect(needsContentSync('<p>甲</p>', '<p>甲</p>')).toBe(false)
  })

  it("空串与 '<p></p>' 视为同一种空,不同步", () => {
    expect(needsContentSync('', '<p></p>')).toBe(false)
    expect(needsContentSync('<p></p>', '')).toBe(false)
  })

  it('外部真换了内容才同步', () => {
    expect(needsContentSync('<p>乙</p>', '<p>甲</p>')).toBe(true)
    expect(needsContentSync('<p>甲</p>', '')).toBe(true)
  })
})

describe('工具栏表', () => {
  it('id 不重复(重复会让 #each 的 key 撞车)', () => {
    const ids = [...TOOLBAR_ITEMS, ...URL_TOOLBAR_ITEMS].map((i) => i.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('每项都有中文标签(工具栏按钮靠它做 aria-label)', () => {
    for (const item of [...TOOLBAR_ITEMS, ...URL_TOOLBAR_ITEMS]) {
      expect(item.label.length).toBeGreaterThan(0)
    }
  })

  it('覆盖设计 §6 列出的全部功能', () => {
    const ids = [...TOOLBAR_ITEMS, ...URL_TOOLBAR_ITEMS].map((i) => i.id)
    expect(ids).toEqual([
      'bold',
      'italic',
      'underline',
      'strike',
      'h1',
      'h2',
      'h3',
      'orderedList',
      'bulletList',
      'blockquote',
      'horizontalRule',
      'unlink',
      'clearFormat',
      'undo',
      'redo',
      'link',
      'image',
    ])
  })

  it('不做表格/代码块/字体/颜色/对齐(D7)', () => {
    const ids = [...TOOLBAR_ITEMS, ...URL_TOOLBAR_ITEMS].map((i) => i.id)
    for (const forbidden of [
      'table',
      'codeBlock',
      'code',
      'fontSize',
      'color',
      'align',
    ]) {
      expect(ids).not.toContain(forbidden)
    }
  })

  it('需要 URL 的两项标了 urlKind,给 isSafeUrl 用', () => {
    expect(URL_TOOLBAR_ITEMS.map((i) => [i.id, i.urlKind])).toEqual([
      ['link', 'href'],
      ['image', 'src'],
    ])
  })
})
