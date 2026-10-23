import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * 三类交付缺陷的防回归护栏。
 *
 * 三条都有同一个性质:**不报错、不告警、不 typecheck 失败**,只有真去点才发现。
 * 所以只能靠源码扫描兜底 —— 与 use-crud-table.test.ts 的「selectable ⇒ getRowId」
 * 和 auth.test.ts 的权限串形状护栏同一路数。
 */

const ROOT = process.cwd()
const routes = (rel: string) =>
  readFileSync(join(ROOT, 'src', 'routes', '(app)', rel, '+page.svelte'), 'utf8')
const lib = (...seg: string[]) => readFileSync(join(ROOT, 'src', 'lib', ...seg), 'utf8')

describe('A: processInstance 行操作的权限守卫', () => {
  // 后端 instance_handler.rs 对这三个动作分别要三个不同的权限;Vue 三个按钮
  // 都挂了 v-hasPermi,Svelte 交付时三个都漏了 —— 无权用户点开必吃 403。
  const src = routes('workflow/processMonitor/processInstance')

  it.each([
    ['workflow:instance:query', '查看'],
    ['workflow:instance:variableQuery', '变量(打开,GET instanceVariable)'],
    ['workflow:instance:variable', '保存变量(PUT updateVariable)'],
    ['workflow:instance:invalid', '作废'],
    ['workflow:instance:remove', '删除'],
  ])('%s 有 HasPermi 守卫 —— %s', (perm) => {
    expect(src).toContain(`perm="${perm}"`)
  })
})

describe('B: 自造 fetch 的下载/导出必须自己 toast', () => {
  // downloadBlob / exportBlob 都绕开 apiClient 自己 fetch,抛的是裸 Error,
  // 全局 toast 管不到 —— 调用方 catch 里不自己报就是「点了没反应」。
  const CALLERS: Array<[string, string]> = [
    ['workflow/processDefinition', 'exportDef'],
    ['system/config', 'configApi.export'],
    ['system/dict', 'typeExport'],
  ]

  it.each(CALLERS)('%s 的导出 catch 里有 toast.error', (page) => {
    const src = routes(page)
    // 取导出函数体到下一个函数定义之前,确认里面有 toast.error。
    const body = src.slice(src.search(/async function handle\w*Export/))
    const end = body.search(/\n  (?:async )?function /)
    expect(end === -1 ? body : body.slice(0, end)).toContain('toast.error')
  })

  it('download.ts 两个自造 fetch 的入口都标注了「调用方必须自己 toast」', () => {
    const src = lib('api', 'download.ts')
    const warnings = src.match(/必须自己 toast\.error/g) ?? []
    expect(warnings.length).toBe(2) // downloadBlob + exportBlob
  })
})

describe('C: dict 的每条变更路径都要清 dictCache', () => {
  // dictCache 是模块级 Map 且无 TTL(use-dict.ts:5,14-18)—— 不清就是整个 SPA
  // 会话内全站 DictTag 都停在旧标签上。交付时 5 条变更路径只清了 1 条。
  const TYPE_PAGE = routes('system/dict')
  const DATA_PANEL = readFileSync(
    join(ROOT, 'src', 'routes', '(app)', 'system', 'dict', 'dict-data-panel.svelte'),
    'utf8'
  )

  // 变更路径数:类型侧 新增/改 + 单删 + 批删 + 刷新缓存 = 4;数据侧 新增/改 + 单删 + 批删 = 3。
  it('类型侧 4 条变更路径都清了缓存', () => {
    expect((TYPE_PAGE.match(/dictCache\.clear\(\)/g) ?? []).length).toBe(4)
  })

  it('数据侧 3 条变更路径都清了缓存', () => {
    expect((DATA_PANEL.match(/dictCache\.clear\(\)/g) ?? []).length).toBe(3)
  })

  it('dictCache 仍是无 TTL 的模块级 Map(护栏前提,变了要重新评估)', () => {
    expect(lib('hooks', 'use-dict.ts')).toContain('export const dictCache = new Map')
  })
})
