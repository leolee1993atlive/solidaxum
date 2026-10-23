export type ShortcutAction = 'undo' | 'redo' | 'copy' | 'paste'

/**
 * 焦点在可输入元素或任何弹窗里时,快捷键要留给表单 / 浏览器(Ctrl+Z 撤销输入、
 * Ctrl+C 复制文本),画布不拦。
 */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  return (
    target.closest(
      'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="dialog"]'
    ) !== null
  )
}

/**
 * 按键 → 画布动作。Ctrl 与 ⌘ 等价;带 Alt 的组合、可输入焦点一律放行(返回 null)。
 * Delete / Backspace 交给 xyflow 的 deleteKey,不在这里处理。
 */
export function shortcutOf(e: KeyboardEvent): ShortcutAction | null {
  if (!(e.ctrlKey || e.metaKey) || e.altKey) return null
  if (isTypingTarget(e.target)) return null
  switch (e.key.toLowerCase()) {
    case 'z':
      return e.shiftKey ? 'redo' : 'undo'
    case 'y':
      return e.shiftKey ? null : 'redo'
    case 'c':
      return e.shiftKey ? null : 'copy'
    case 'v':
      return e.shiftKey ? null : 'paste'
    default:
      return null
  }
}
