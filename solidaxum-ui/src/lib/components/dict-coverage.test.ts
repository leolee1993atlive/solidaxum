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

describe('S6 字典接入护栏', () => {
  // 这些页面的硬编码 label 已换成 DictTag。新写的页面若再手写映射，
  // 应改用 <DictTag dictType="..."/>，否则字典维护对该页失效。
  // 注意路径精确匹配:用 `/page` 形式避免 `system/user` 误匹配 `system/role-auth/user`。
  const SWEPT = [
    'system/user/+page', 'system/dept/+page', 'system/role/+page', 'system/post/+page',
    'system/notice/+page', 'system/client/+page', 'system/menu/+page',
    'system/log/operlog/+page', 'system/log/logininfo/+page',
    'system/oss-config/+page',
    // role-auth 的两个子页同样渲染 sys_normal_disable 状态列,
    // 属同一根因覆盖范围,一并纳入护栏。
    'system/role-auth/user/[roleId]/+page',
    'system/role-auth/user/[roleId]/select-user-dialog',
  ]

  it('已 sweep 的页面不再手写 statusLabel/genderLabel/noticeTypeLabel/businessTypeLabel', () => {
    const offenders: string[] = []
    for (const file of walk(ROUTES)) {
      const rel = file.replaceAll('\\', '/')
      if (!SWEPT.some((p) => rel.includes(p))) continue
      const src = readFileSync(file, 'utf8')
      for (const fn of ['statusLabel', 'genderLabel', 'noticeTypeLabel', 'businessTypeLabel']) {
        if (src.includes(`function ${fn}`)) offenders.push(`${rel}: ${fn}`)
      }
    }
    expect(offenders).toEqual([])
  })
})

describe('S3 changeStatus 死代码护栏', () => {
  // roleApi/ossConfigApi/clientApi.changeStatus 曾「已定义但页面从未调用」。
  // 这条测试保证它们不会再退化成死代码。
  const CALLERS: Array<[string, string]> = [
    ['roleApi.changeStatus', 'system/role'],
    ['clientApi.changeStatus', 'system/client'],
    ['ossConfigApi.changeStatus', 'system/oss-config'],
    ['userApi.changeStatus', 'system/user'],
  ]

  it.each(CALLERS)('%s 在 %s 页面被真实调用', (call, page) => {
    const hit = walk(ROUTES)
      .filter((f) => f.replaceAll('\\', '/').includes(page))
      .some((f) => readFileSync(f, 'utf8').includes(call))
    expect(hit).toBe(true)
  })
})
