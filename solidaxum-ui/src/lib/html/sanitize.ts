import DOMPurify from 'dompurify'

/**
 * 渲染面标签白名单 —— **全仓唯一事实源**。
 *
 * 与 `$lib/components/rich-text.ts` 的工具栏表逐条对应,由
 * `sanitize-parity.test.ts` 钉死:这里删一行、那边立刻红。改之前先看那个测试。
 */
export const ALLOWED_TAGS = [
  'p',
  'br',
  'strong',
  'em',
  'u',
  's',
  'h1',
  'h2',
  'h3',
  'ul',
  'ol',
  'li',
  'blockquote',
  'hr',
  'a',
  'img',
] as const

/**
 * 逐标签的属性白名单。表里没有的标签一个属性都不带。
 *
 * DOMPurify 的 `ALLOWED_ATTR` 是**全局**的 —— 只配它的话 `<p title>` 也会过,
 * 所以 per-tag 收紧放在 `afterSanitizeAttributes` 钩子里做。
 */
const ATTR_BY_TAG: Record<string, readonly string[]> = {
  a: ['href', 'target', 'rel'],
  img: ['src', 'alt', 'title', 'width'],
}

/** 交给 DOMPurify 的全局属性白名单 = 逐标签表的并集。 */
export const ALLOWED_ATTR = [...new Set(Object.values(ATTR_BY_TAG).flat())]

/**
 * 协议校验 —— 编辑器**插入前**与渲染**消毒时**共用同一把尺子。
 *
 * - 不带协议的一律当站内相对路径放行(`/x`、`x.png`、`#a`、`?q=1`)。
 * - 带协议的:`href` 认 http/https/mailto,`src` 只认 http/https。
 * - 先剥掉所有控制字符(空格/换行/制表符),挡 `java\nscript:` 这类拆词绕法。
 *
 * 已知放行项:`//host/x` 这种协议相对地址会被当成相对路径 —— 它跟随当前页协议,
 * 与直接允许 https 外链等价,不额外收紧。
 */
export function isSafeUrl(url: string, kind: 'href' | 'src' = 'href'): boolean {
  const normalized = url.replace(/[\u0000-\u0020]/g, '').toLowerCase()
  if (normalized === '') return false
  if (!/^[a-z][a-z0-9+.-]*:/.test(normalized)) return true // 无协议 = 相对路径
  return kind === 'href'
    ? /^(?:https?|mailto):/.test(normalized)
    : /^https?:/.test(normalized)
}

let hooksInstalled = false

/**
 * DOMPurify 的钩子挂在单例上,只能挂一次 —— 重复挂会让每个属性被处理 N 遍。
 * 本仓只有本文件用这个单例。
 */
function installHooks(): void {
  if (hooksInstalled) return
  hooksInstalled = true

  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    const el = node as Element
    if (typeof el.tagName !== 'string') return
    const tag = el.tagName.toLowerCase()
    const allowed = ATTR_BY_TAG[tag] ?? []

    // 倒序遍历:removeAttribute 会就地改动 attributes 这个动态集合
    for (let i = el.attributes.length - 1; i >= 0; i--) {
      const name = el.attributes[i].name
      if (!allowed.includes(name)) el.removeAttribute(name)
    }

    const href = el.getAttribute('href')
    if (href !== null && !isSafeUrl(href, 'href')) el.removeAttribute('href')

    const src = el.getAttribute('src')
    if (src !== null && !isSafeUrl(src, 'src')) el.removeAttribute('src')

    // target=_blank 会把 window.opener 交给目标页 —— 一律补 rel
    if (tag === 'a' && el.getAttribute('target') === '_blank') {
      el.setAttribute('rel', 'noopener noreferrer')
    }
  })
}

/**
 * 渲染面消毒。**只在渲染时做,不回写数据库** —— 历史脏数据因此无需迁移。
 *
 * 这是第二道过滤:第一道是编辑器的 schema(管「编辑器里能产生什么」),
 * 这一道管「页面上能渲染什么」。后者防的是绕过 UI 直接调 API 写入的 HTML ——
 * 有 `system:notice:edit` 权限的人可以这么干。
 */
export function sanitizeHtml(dirty: string): string {
  if (!dirty) return ''
  installHooks()
  return DOMPurify.sanitize(dirty, {
    ALLOWED_TAGS: [...ALLOWED_TAGS],
    ALLOWED_ATTR: [...ALLOWED_ATTR],
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
  })
}

/** 块级结束处补一个空格,否则 `<p>甲</p><p>乙</p>` 会被压成「甲乙」。 */
const BLOCK_END = /<\/(?:p|div|li|h[1-3]|blockquote)>|<br\s*\/?>/gi

/**
 * HTML → 纯文本。给推送 toast 这类**不渲染 HTML** 的场合用。
 *
 * 走 DOMPurify 而不是正则剥标签:`script` / `style` 在它的默认 FORBID_CONTENTS
 * 里,连内容一起丢;正则只会把标签删掉、把 `alert(1)` 留在文案里。
 */
export function stripHtml(html: string): string {
  if (!html) return ''
  installHooks()
  const escaped = DOMPurify.sanitize(html.replace(BLOCK_END, '$& '), {
    ALLOWED_TAGS: [],
    ALLOWED_ATTR: [],
    KEEP_CONTENT: true,
  })
  // 上一步出来的是转义后的文本(`&amp;`),toast 要的是人读的字
  const doc = new DOMParser().parseFromString(escaped, 'text/html')
  return (doc.body.textContent ?? '').replace(/\s+/g, ' ').trim()
}
