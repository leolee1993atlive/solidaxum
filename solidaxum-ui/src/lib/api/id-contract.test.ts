import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

/** 递归收集 src 下的 .ts/.svelte 源文件(排除测试文件)。 */
function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) {
      sourceFiles(p, out)
    } else if (
      (p.endsWith('.ts') || p.endsWith('.svelte')) &&
      !p.endsWith('.test.ts')
    ) {
      out.push(p)
    }
  }
  return out
}

describe('ID 契约', () => {
  const files = sourceFiles('src')

  // 后端 ID 出站恒为字符串(见 solidaxum crates/core/src/vo/response.rs
  // 的 stringify_ids)。任何 Number(xxxId) 都会在 19 位雪花 ID 上静默截断。
  it('源码中不得把 ID 转成 number', () => {
    const offenders: string[] = []
    for (const f of files) {
      const src = readFileSync(f, 'utf8')
      for (const [i, line] of src.split('\n').entries()) {
        // \wId 要求大写 I,\bid 要求独立单词 —— 避免 Number(valid) 之类误伤
        if (/(?:Number|parseInt)\s*\(\s*[^)]*(?:\wIds?\b|\bids?\b)/.test(line)) {
          offenders.push(`${f}:${i + 1}: ${line.trim()}`)
        }
      }
    }
    expect(offenders).toEqual([])
  })

  it('api 层不得把 ID 声明为 number', () => {
    const offenders: string[] = []
    for (const f of files.filter((f) => f.includes(join('src', 'lib', 'api')))) {
      const src = readFileSync(f, 'utf8')
      for (const [i, line] of src.split('\n').entries()) {
        if (/(?:\wIds?|\bids?)\??\s*:\s*number/.test(line)) {
          offenders.push(`${f}:${i + 1}: ${line.trim()}`)
        }
      }
    }
    expect(offenders).toEqual([])
  })

  // 后端 is_id_key(crates/core/src/vo/response.rs:104-127)把这些**不带 Id 后缀**的
  // 语义 ID 键也无条件字符串化。它们名字里没有 "id",上一条正则扫不到 ——
  // dept.leader 就这么以 `number` 混过了三个包,还绑在 <Input type="number"> 上
  // (Svelte 会把 19 位雪花 id 转成 JS number → 静默截断 → 写坏 leader)。
  const SEMANTIC_ID_KEYS = [
    'leader',
    'avatar',
    'associated',
    'dictCode',
    'createBy',
    'updateBy',
    'createDept',
    'checkedKeys',
  ]

  it('语义 ID 键(leader/avatar/createBy 等)不得声明为 number', () => {
    const offenders: string[] = []
    const re = new RegExp(`\\b(?:${SEMANTIC_ID_KEYS.join('|')})\\??\\s*:\\s*number\\b`)
    for (const f of files) {
      const src = readFileSync(f, 'utf8')
      for (const [i, line] of src.split('\n').entries()) {
        if (re.test(line)) offenders.push(`${f}:${i + 1}: ${line.trim()}`)
      }
    }
    expect(offenders).toEqual([])
  })
})
