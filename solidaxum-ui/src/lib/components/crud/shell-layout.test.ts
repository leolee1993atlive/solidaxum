import { describe, it, expect } from 'vitest'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

const ROUTES = join(process.cwd(), 'src/routes')
const SRC = join(process.cwd(), 'src')

function svelteFiles(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) svelteFiles(full, acc)
    else if (entry.endsWith('.svelte')) acc.push(full)
  }
  return acc
}

// src 全仓文本文件(含 .test.ts),用于旧 snippet 名清零护栏。
function allSrcFiles(): string[] {
  const out: string[] = []
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      const full = join(dir, entry)
      if (statSync(full).isDirectory()) walk(full)
      else if (/\.(svelte|ts|js|tsx|jsx)$/.test(entry)) out.push(full)
    }
  }
  walk(SRC)
  return out
}

const pages = svelteFiles(ROUTES).map((p) => ({
  path: p.replace(ROUTES, ''),
  text: readFileSync(p, 'utf8'),
}))

// 大标题只出现在 <h1>/<h2> 上;text-2xl 也可能用在非标题处
// (如首页 index/+page.svelte 的统计卡片大数字 <div class="text-2xl ...">)。
// 首页统计卡片那类用法是合法保留的,故断言口径必须钉在「标题元素 + text-2xl」,
// 而不是裸 text-2xl 字符串 —— 否则会误伤首页统计卡片,测试永远红。
// 同时仍用 text-2xl 而非完整 class 串作锚:大标题有三种写法
// (h2 bold tracking-tight / h1 bold / h1 semibold tracking-tight),
// 按完整串 grep 在制定规格时连续漏掉过两批页面。
const TITLE_RE = /<h[12]\b[^>]*\btext-2xl\b/

describe('页面大标题已清零', () => {
  it('src/routes 下不再有标题级 text-2xl(<h1>/<h2>)', () => {
    const offenders = pages.filter((p) => TITLE_RE.test(p.text)).map((p) => p.path)
    expect(offenders).toEqual([])
  })
})

describe('旧 snippet 名已清零', () => {
  it.each([['extraToolbar'], ['batchToolbar']])('全仓不再出现 %s', (name) => {
    // 扫 src 全仓(含 .test.ts 等非 svelte 文件),排除本护栏文件自身——
    // 它的 it.each 名单必然包含这两个名字。
    const all = allSrcFiles()
      .filter((p) => !p.endsWith('shell-layout.test.ts'))
      .map((p) => ({
        path: p.replace(process.cwd(), ''),
        text: readFileSync(p, 'utf8'),
      }))
    const offenders = all.filter((p) => p.text.includes(name)).map((p) => p.path)
    expect(offenders).toEqual([])
  })
})
