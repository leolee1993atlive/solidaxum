import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const ROUTES = join(process.cwd(), 'src', 'routes')

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, out)
    else if (p.endsWith('.svelte')) out.push(p)
  }
  return out
}

function read(page: string): string {
  return walk(ROUTES)
    .filter((f) => f.replaceAll('\\', '/').includes(page))
    .map((f) => readFileSync(f, 'utf8'))
    .join('\n')
}

describe('spel 页可维护性护栏', () => {
  // 该页曾是「只读列表 + 逐行删除」,建好的表达式只能删不能改。
  const src = read('workflow/spel')
  it.each(['spelApi.create', 'spelApi.update', 'buildViewSpel'])('引用 %s', (call) => {
    expect(src).toContain(call)
  })
})

describe('processInstance 端点覆盖护栏', () => {
  // 该页曾只调 pageByRunning,已完成实例在界面上不可见。
  const src = read('workflow/processMonitor/processInstance')
  it.each(['pageRunning', 'pageFinish', 'deleteByInstanceIds', 'instanceVariable'])(
    '引用 %s',
    (call) => {
      expect(src).toContain(call)
    }
  )

  it('不再用 deleteByBusinessIds(PIN14:应按实例 id 删除)', () => {
    expect(src).not.toContain('deleteByBusinessIds')
  })
})
