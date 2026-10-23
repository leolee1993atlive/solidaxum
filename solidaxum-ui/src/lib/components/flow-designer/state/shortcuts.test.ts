import { describe, expect, it } from 'vitest'
import { isTypingTarget, shortcutOf } from './shortcuts.js'

function key(
  k: string,
  mods: Partial<
    Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>
  > = {},
  target: EventTarget | null = document.body
): KeyboardEvent {
  const e = new KeyboardEvent('keydown', {
    key: k,
    ctrlKey: mods.ctrlKey ?? false,
    metaKey: mods.metaKey ?? false,
    shiftKey: mods.shiftKey ?? false,
    altKey: mods.altKey ?? false,
  })
  // happy-dom 的 KeyboardEvent 构造后 target 为 null,派发到目标上才有值
  target?.dispatchEvent(e)
  return e
}

describe('shortcutOf', () => {
  it.each([
    ['z', { ctrlKey: true }, 'undo'],
    ['Z', { ctrlKey: true, shiftKey: true }, 'redo'],
    ['y', { ctrlKey: true }, 'redo'],
    ['c', { ctrlKey: true }, 'copy'],
    ['v', { ctrlKey: true }, 'paste'],
  ] as const)('%s + %o → %s', (k, mods, action) => {
    expect(shortcutOf(key(k, mods))).toBe(action)
  })

  it('⌘ 与 Ctrl 等价', () => {
    expect(shortcutOf(key('z', { metaKey: true }))).toBe('undo')
    expect(shortcutOf(key('v', { metaKey: true }))).toBe('paste')
  })

  it('没有修饰键、带 Alt、Shift+C/V/Y、其它键 → null', () => {
    expect(shortcutOf(key('z'))).toBeNull()
    expect(shortcutOf(key('z', { ctrlKey: true, altKey: true }))).toBeNull()
    expect(shortcutOf(key('c', { ctrlKey: true, shiftKey: true }))).toBeNull()
    expect(shortcutOf(key('y', { ctrlKey: true, shiftKey: true }))).toBeNull()
    expect(shortcutOf(key('s', { ctrlKey: true }))).toBeNull()
    expect(shortcutOf(key('Delete'))).toBeNull()
  })

  it('焦点在 input / textarea / select / contenteditable / 弹窗内 → 放行', () => {
    const html = `
      <input id="i" /><textarea id="t"></textarea><select id="s"></select>
      <div id="ce" contenteditable="true"><span id="ce-child">x</span></div>
      <div id="ce-off" contenteditable="false"></div>
      <div role="dialog"><button id="in-dialog">ok</button></div>
      <div id="plain"></div>`
    document.body.innerHTML = html
    for (const id of ['i', 't', 's', 'ce-child', 'in-dialog']) {
      const el = document.getElementById(id)!
      expect(isTypingTarget(el), id).toBe(true)
      expect(shortcutOf(key('z', { ctrlKey: true }, el)), id).toBeNull()
    }
    for (const id of ['ce-off', 'plain']) {
      const el = document.getElementById(id)!
      expect(isTypingTarget(el), id).toBe(false)
      expect(shortcutOf(key('z', { ctrlKey: true }, el)), id).toBe('undo')
    }
    expect(isTypingTarget(null)).toBe(false)
    document.body.innerHTML = ''
  })
})
