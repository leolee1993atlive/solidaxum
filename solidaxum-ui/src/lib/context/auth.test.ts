import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, it, expect } from 'vitest'
import { hasPermission, hasRole } from './auth-helpers'

describe('hasPermission', () => {
  it('true when perms include *:*:*', () => {
    expect(hasPermission(['*:*:*'], 'system:user:list')).toBe(true)
  })
  it('true when exact match', () => {
    expect(hasPermission(['system:user:list'], 'system:user:list')).toBe(true)
  })
  it('false otherwise', () => {
    expect(hasPermission(['system:role:list'], 'system:user:list')).toBe(false)
  })
  it('false on empty', () => {
    expect(hasPermission([], 'system:user:list')).toBe(false)
  })
})

describe('HasPermi 权限串形状护栏(全仓)', () => {
  // 🔴 hasPermission 是精确 includes 匹配(只有 *:*:* 通配逃逸,见上方用例)。
  // 权限串多一个字符就永不命中 —— 而且**不报错**:守卫直接不渲染,按钮凭空消失。
  // processDefinition 曾把 Vue 的 v-hasPermi="['workflow:definition:add']" 去掉方括号
  // 却留下内层单引号,写成 perm="'workflow:definition:add'",12 处全中招 →
  // 该页所有受守卫按钮对非超管一律不可见,因超管有 *:*:* 而一直没被发现。
  const SRC = join(process.cwd(), 'src')
  // RuoYi 权限串形如 system:user:list / monitor:job:changeStatus(段内允许驼峰)。
  const PERM_SHAPE = /^[a-z][a-zA-Z]*(:[a-zA-Z*]+){2,}$|^\*:\*:\*$/

  function walk(dir: string, out: string[] = []): string[] {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name)
      if (statSync(p).isDirectory()) walk(p, out)
      else if (p.endsWith('.svelte')) out.push(p)
    }
    return out
  }

  it('所有字面量 perm="..." 都是合法权限串(无引号/方括号/空格残留)', () => {
    const offenders: string[] = []
    for (const f of walk(SRC)) {
      const src = readFileSync(f, 'utf8')
      // 只取字面量写法 perm="..."(perm={expr} 是表达式,不在本护栏范围)
      for (const m of src.matchAll(/\bperm="([^"]*)"/g)) {
        if (!PERM_SHAPE.test(m[1])) {
          offenders.push(`${f.replaceAll('\\', '/').split('/src/')[1]}: perm="${m[1]}"`)
        }
      }
    }
    expect(offenders).toEqual([])
  })
})

describe('hasRole', () => {
  it('true when admin', () => {
    expect(hasRole(['admin'], 'anything')).toBe(true)
  })
  it('true when exact match', () => {
    expect(hasRole(['common'], 'common')).toBe(true)
  })
  it('false otherwise', () => {
    expect(hasRole(['common'], 'admin')).toBe(false)
  })
})
