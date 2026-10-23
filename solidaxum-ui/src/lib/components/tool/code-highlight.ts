import hljs from 'highlight.js/lib/core'
import json from 'highlight.js/lib/languages/json'
import rust from 'highlight.js/lib/languages/rust'
import typescript from 'highlight.js/lib/languages/typescript'
import xml from 'highlight.js/lib/languages/xml'

// 只注册后端 template_names(code_gen.rs:197-232)实际会产出的语言:
// rust 五件套 + vue/svelte 模板(hljs 无这两种,用 xml 最接近)+ ts + json。
// 从 highlight.js/lib/core 引而不是 highlight.js —— 后者会打进全部 190+ 语言。
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('xml', xml)
hljs.registerLanguage('json', json)
// plaintext 必须注册:menu.sql 等未知后缀文件走它,hljs/lib/core
// 未注册语言时 highlight() 直接抛 "Unknown language"(走查实测,
// 预览对话框因此卡在加载中)。
hljs.registerLanguage('plaintext', () => ({ name: 'Plain text', contains: [] }))

/**
 * 按输出路径后缀选语言。
 * 未知后缀返回 plaintext 而不是让 hljs 自动检测 —— 猜错语言的高亮
 * 比不高亮更难读(关键字被涂成字符串色之类)。
 */
export function languageForPath(path: string): string {
  if (path.endsWith('.rs')) return 'rust'
  if (path.endsWith('.ts')) return 'typescript'
  if (path.endsWith('.vue') || path.endsWith('.svelte')) return 'xml'
  if (path.endsWith('.json')) return 'json'
  return 'plaintext'
}

/** 返回 hljs 的高亮 HTML。语言未注册时原样返回转义后的文本。 */
export function highlightCode(code: string, path: string): string {
  const lang = languageForPath(path)
  // plaintext 已在上方注册为空规则语言,不会抛 Unknown language。
  return hljs.highlight(code, { language: lang }).value
}
