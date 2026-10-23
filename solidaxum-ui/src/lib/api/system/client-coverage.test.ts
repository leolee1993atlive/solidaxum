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

const clientSrc = walk(ROUTES)
  .filter((f) => f.replaceAll('\\', '/').includes('system/client/+page'))
  .map((f) => readFileSync(f, 'utf8'))
  .join('\n')

describe('client OAuth2 边界配置护栏', () => {
  // CLI1/CLI2/CLI3: 三个字段曾整块缺失或退化，client.ts 里类型预留却从未使用。
  it.each(['accessPath', 'ipWhitelist', 'grantTypeList'])('弹窗引用 %s', (field) => {
    expect(clientSrc).toContain(field)
  })

  it('授权类型走字典多选，不再硬编码 password/refresh_token', () => {
    expect(clientSrc).toContain('sys_grant_type')
  })

  it('白名单走 client-rules 纯函数（两端分隔符不一致，不能裸 split）', () => {
    expect(clientSrc).toContain('parseRuleText')
    expect(clientSrc).toContain('resolveRuleList')
  })

  // CLI1 回归护栏：多选可以被全部取消，而两端都没有兜底 ——
  // 后端 ClientCreateBo/ClientUpdateBo 对 grant_type 无 #[validate]，
  // from_create_bo 把空数组 join 成 ""，supports_grant_type("") 恒不匹配
  // （auth_handler.rs:59）→ 存成空 = 一个永远登不进来的客户端，且保存时无提示。
  // Vue 用 rules.grantTypeList required 拦（index.vue:340），前端是唯一防线。
  it('提交前拦截空的授权类型（否则会存出登不进来的客户端）', () => {
    expect(clientSrc).toMatch(/form\.grantTypeList\.length === 0/)
  })
})
