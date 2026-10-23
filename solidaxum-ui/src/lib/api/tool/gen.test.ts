import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { columnFieldOptions, sortPreviewNames } from './gen.js'
import {
  languageForPath,
  highlightCode,
} from '$lib/components/tool/code-highlight.js'
import type { GenColumnRow } from './gen.js'

// 这三条是「端点字符串」护栏 —— 三个 404/参数错都不报编译错、不报 typecheck 错,
// 只在运行时打空气。用源码扫描锁死,比 mock fetch 更直接。
// 后端权威:crates/system/src/codegen/route/gen_route.rs:11-22
//   /tool/gen/getDataNames         (GET)
//   /tool/gen/batchGenCode         (GET, 查询参数 tableIdStr,值是逗号分隔的 i64 表 ID)
//   /tool/gen/download/{tableId}   (GET)
// 不存在的:/tool/gen/datasource/names · /tool/gen/genCode/*
describe('gen.ts 端点形状护栏', () => {
  const SRC = readFileSync(
    join(process.cwd(), 'src', 'lib', 'api', 'tool', 'gen.ts'),
    'utf8'
  )

  it('数据源列表打的是 /tool/gen/getDataNames', () => {
    expect(SRC).toContain('/tool/gen/getDataNames')
  })

  it('不再出现后端没有的 /tool/gen/datasource/names', () => {
    expect(SRC).not.toContain('/tool/gen/datasource/names')
  })

  it('不再出现后端没有的 /tool/gen/genCode/', () => {
    expect(SRC).not.toContain('/tool/gen/genCode/')
  })

  it('批量生成用 tableIdStr 而不是 tables', () => {
    expect(SRC).toContain('batchGenCode?tableIdStr=')
    expect(SRC).not.toMatch(/batchGenCode\?tables=/)
  })
})

const rows: GenColumnRow[] = [
  {
    columnId: '1',
    tableId: '9',
    columnName: 'dept_id',
    columnComment: '部门id',
    columnType: 'bigint',
    javaType: 'Long',
    javaField: 'deptId',
    isPk: '1',
    isIncrement: '0',
    isRequired: '1',
    isInsert: '1',
    isEdit: '0',
    isList: '1',
    isQuery: '0',
    queryType: 'EQ',
    htmlType: 'input',
    sort: 1,
  },
  {
    columnId: '2',
    tableId: '9',
    columnName: 'status',
    columnComment: '状态',
    columnType: 'char',
    javaType: 'String',
    javaField: 'status',
    isPk: '0',
    isIncrement: '0',
    isRequired: '0',
    isInsert: '1',
    isEdit: '1',
    isList: '1',
    isQuery: '1',
    queryType: 'EQ',
    htmlType: 'select',
    sort: 2,
  },
]

describe('columnFieldOptions', () => {
  it('用 javaField 作值、「注释(字段名)」作展示', () => {
    expect(columnFieldOptions(rows)).toEqual([
      { value: 'deptId', label: '部门id (dept_id)' },
      { value: 'status', label: '状态 (status)' },
    ])
  })

  it('无注释时只显示字段名,不出现空括号', () => {
    expect(columnFieldOptions([{ ...rows[0], columnComment: undefined }])[0].label).toBe(
      'dept_id'
    )
  })
})

describe('sortPreviewNames', () => {
  // 后端 preview() 返回 HashMap<String,String>(gen_handler.rs:45),
  // Object.keys() 顺序不保证 → 默认 tab 是随机的。
  // 注意:Vue 硬编码默认 domain.java —— 那是 Java 版的事实,
  // 本仓是 Rust 生成器,键是输出路径(code_gen.rs:327+),没有 domain.java。
  // 正确修法是给一个确定性排序,默认落在排序后第一个。
  const NAMES = [
    'src/api/system/foo/types.ts',
    'src/views/system/foo/index.vue',
    'src/system/foo/foo_handler.rs',
    'src/system/foo/foo.rs',
    'src/api/system/foo/index.ts',
    'src/system/foo/foo_repo.rs',
  ]

  it('Rust 文件排在前,其次前端模板,最后 ts', () => {
    const sorted = sortPreviewNames(NAMES)
    expect(sorted[0]).toBe('src/system/foo/foo.rs')
    expect(sorted.slice(0, 3).every((n) => n.endsWith('.rs'))).toBe(true)
    expect(sorted[3]).toBe('src/views/system/foo/index.vue')
  })

  it('同类内按字典序,model 文件(foo.rs)排在 foo_handler.rs 之前', () => {
    const sorted = sortPreviewNames(NAMES)
    expect(sorted.indexOf('src/system/foo/foo.rs')).toBeLessThan(
      sorted.indexOf('src/system/foo/foo_handler.rs')
    )
  })

  it('输入顺序不影响输出(确定性)', () => {
    const a = sortPreviewNames(NAMES)
    const b = sortPreviewNames([...NAMES].reverse())
    expect(a).toEqual(b)
  })

  it('空输入返回空数组', () => {
    expect(sortPreviewNames([])).toEqual([])
  })
})

describe('languageForPath', () => {
  it('.rs → rust', () => {
    expect(languageForPath('src/system/foo/foo.rs')).toBe('rust')
  })
  it('.ts → typescript', () => {
    expect(languageForPath('src/api/system/foo/index.ts')).toBe('typescript')
  })
  it('.vue / .svelte → xml(hljs 无这两种,模板走 xml 最接近)', () => {
    expect(languageForPath('a/index.vue')).toBe('xml')
    expect(languageForPath('a/+page.svelte')).toBe('xml')
  })
  it('.json → json', () => {
    expect(languageForPath('a/b.json')).toBe('json')
  })
  it('未知后缀 → plaintext(不让 hljs 猜,猜错比不高亮更难读)', () => {
    expect(languageForPath('a/b.sql')).toBe('plaintext')
    expect(languageForPath('noext')).toBe('plaintext')
  })
})

describe('highlightCode', () => {
  // 走查修复钉(2026-10-10): menu.sql 等未知后缀文件走 plaintext,
  // 而 highlight.js/lib/core 不内置该语言——未注册时 highlight()
  // 直接抛 "Unknown language",代码预览对话框因此卡在「加载中」。
  it('plaintext 路径不抛 Unknown language,且返回转义后的高亮 HTML', () => {
    expect(() =>
      highlightCode('insert into sys_menu values(...)', 'sql/Foo/menu.sql')
    ).not.toThrow()
    const out = highlightCode('-- 菜单 SQL\ninsert into x', 'sql/Foo/menu.sql')
    expect(out).toContain('菜单 SQL')
  })
  it('已注册语言路径正常返回', () => {
    expect(highlightCode('let a = 1', 'src/api/x/index.ts')).toContain('a')
  })
})
