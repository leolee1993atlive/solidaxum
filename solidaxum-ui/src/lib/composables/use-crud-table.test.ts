import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { TABLE_DEFAULT_STATE } from '$lib/table-defaults.js'
import {
  fromListResult,
  selectedIdsFrom,
  withSortingFlags,
} from './use-crud-table.svelte.js'

describe('TABLE_DEFAULT_STATE 前提锁定', () => {
  // 这条**现在就应该 PASS** —— TABLE_DEFAULT_STATE = table-core 的 initialState,
  // 已含 rowSelection。它是前提锁定而非失败测试:createTable 的 getState() 不合并
  // initialState,若 table-core 升级后键集变化,这条会第一个红,提醒去铺键。
  it('含 rowSelection(createTable 不合并 initialState,受控键必须已在默认 state 内)', () => {
    expect(TABLE_DEFAULT_STATE).toHaveProperty('rowSelection')
  })
})

describe('selectedIdsFrom', () => {
  it('只取值为 true 的键', () => {
    expect(selectedIdsFrom({ '10': true, '11': false, '12': true })).toEqual(['10', '12'])
  })
  it('空选择返回空数组', () => {
    expect(selectedIdsFrom({})).toEqual([])
  })
  it('返回的是业务主键字符串,不是行索引', () => {
    // getRowId 设为主键后,rowSelection 的键即主键;若仍是 '0'/'1' 这类索引,
    // 说明 getRowId 没配,选择状态会串页。
    const ids = selectedIdsFrom({ '1761100000000000001': true })
    expect(ids[0]).toBe('1761100000000000001')
  })
})

describe('selectable ⇒ getRowId 护栏(全仓)', () => {
  // 🔴 useCrudTable 的 enableRowSelection 是 Boolean(getRowId)(见 .svelte.ts)。
  // 页面开了 CrudTableShell 的 selectable 却不传 getRowId 时:
  //   - 勾选框照常渲染,但 table-core 的 mutateRowIsSelected 被 getCanSelect() 拦掉
  //   - rowSelection 永远是 {} → selectedCount 恒 0 → 所有批量按钮恒 disabled
  // 不报错、不告警,只是"点了没反应"。实测三页曾中招:
  // allTaskWaiting · logininfo(解锁)· processDefinition(导出/修改)。
  const ROUTES = join(process.cwd(), 'src', 'routes')

  function walk(dir: string, out: string[] = []): string[] {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name)
      if (statSync(p).isDirectory()) walk(p, out)
      else if (p.endsWith('.svelte')) out.push(p)
    }
    return out
  }

  it('凡在 CrudTableShell 上开 selectable 的页面都传了 getRowId', () => {
    const offenders = selectablePages()
      .filter(({ src }) => !src.includes('getRowId'))
      .map(({ f }) => f.replaceAll('\\', '/').split('/src/')[1])
    expect(offenders).toEqual([])
  })

  // 🔴 reload() 不动 rowSelection,而 selectedIdsFrom 只过滤 truthy、
  // **不与当前行求交** —— 批量删除后已删的业务主键仍留在选择集里:
  //   - 批量按钮保持可点、计数是旧的,再点一次就是对已删 id 重发 DELETE
  //   - user 页的「解锁」门是 selectedCount !== 1,读的是同一张脏表
  // 全仓 role/post/user 三页曾中招;其余页面用 clearSelection() 或
  // table.resetRowSelection()(两者等价,都走 onRowSelectionChange)。
  it('凡开 selectable 的页面都有清空选择的路径', () => {
    const offenders = selectablePages()
      .filter(
        ({ src }) =>
          !src.includes('clearSelection') && !src.includes('resetRowSelection')
      )
      .map(({ f }) => f.replaceAll('\\', '/').split('/src/')[1])
    expect(offenders).toEqual([])
  })

  function selectablePages(): { f: string; src: string }[] {
    return (
      walk(ROUTES)
        .map((f) => ({ f, src: readFileSync(f, 'utf8') }))
        // 只认作为 prop 出现的 selectable(排除注释/字符串/同名局部函数)
        .filter(({ src }) => /^\s*selectable(\s|$|=|\/>)/m.test(src))
    )
  }
})

describe('fromListResult', () => {
  it('把 ListResult 的四个字段整体搬到 FetchPageResult', () => {
    expect(
      fromListResult({
        rows: [{ id: 1 }],
        total: 10000,
        totalCapped: true,
        totalApprox: 200002,
      })
    ).toEqual({
      rows: [{ id: 1 }],
      totalRowCount: 10000,
      totalCapped: true,
      totalApprox: 200002,
    })
  })

  it('后端没给计数口径时留 undefined(视图按未截断处理)', () => {
    const r = fromListResult({ rows: [], total: 0 })
    expect(r.totalCapped).toBeUndefined()
    expect(r.totalApprox).toBeUndefined()
  })
})

describe('计数口径不得在页面里被丢掉(全仓)', () => {
  // 🔴 后端 total 默认走上限计数(1 万):触顶时 total 只是**下界**,
  // totalCapped 说明这件事、totalApprox 补一个量级。页面若手写
  // `{ rows: resp.rows, totalRowCount: resp.total }`,这两个字段就被静默丢掉,
  // 分页条会把一个被截断的 10000 显示得和精确总数一模一样 —— 不报错、不告警。
  // 唯一正确的写法是 fromListResult(resp),它一次性搬全部字段。
  //
  // 两处刻意不走 fromListResult 的例外,都不经后端分页、没有计数口径可言:
  //   monitor/online   —— 后端返全量,前端自己切片,总数就是 all.length
  //   dict-data-panel  —— 未选中字典类型时的空守卫 { rows: [], totalRowCount: 0 }
  const ALLOWED = ['monitor/online/+page.svelte', 'dict/dict-data-panel.svelte']
  const ROUTES = join(process.cwd(), 'src', 'routes')

  function walk(dir: string, out: string[] = []): string[] {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name)
      if (statSync(p).isDirectory()) walk(p, out)
      else if (p.endsWith('.svelte')) out.push(p)
    }
    return out
  }

  it('列表页一律经 fromListResult 交付分页结果', () => {
    const offenders = walk(ROUTES)
      .map((f) => ({
        rel: f.replaceAll('\\', '/').split('/src/')[1],
        src: readFileSync(f, 'utf8'),
      }))
      .filter(({ src }) => src.includes('totalRowCount'))
      .filter(({ rel }) => !ALLOWED.some((a) => rel.endsWith(a)))
      .map(({ rel }) => rel)
    expect(offenders).toEqual([])
  })
})

describe('withSortingFlags', () => {
  // ColumnHeader 只在 column.getCanSort() 为真时画排序下拉,而 table-core
  // 默认每列都可排序。不按 sortableIds 关掉其余列,就等于给每一列都画一个
  // 点了被 sortableIds.has() 过滤掉的控件 —— 把「装饰表头」从 0 列扩到全部列,
  // 比现状更糟(全仓可排序表头目前都是装饰)。
  const cols = [
    { accessorKey: 'id', header: '编号' },
    { accessorKey: 'name', header: '名称' },
    { id: 'actions', header: '操作' },
  ]

  it('白名单内的列 enableSorting 为 true', () => {
    const out = withSortingFlags(cols, new Set(['name']))
    expect(out[1].enableSorting).toBe(true)
  })

  it('白名单外的列 enableSorting 为 false', () => {
    const out = withSortingFlags(cols, new Set(['name']))
    expect(out[0].enableSorting).toBe(false)
    expect(out[2].enableSorting).toBe(false)
  })

  it('空白名单时全表 enableSorting 为 false', () => {
    const out = withSortingFlags(cols, new Set())
    expect(out.every((c) => c.enableSorting === false)).toBe(true)
  })

  it('按 id 优先、accessorKey 兜底取列标识', () => {
    // 'actions' 列只有 id;'name' 列只有 accessorKey。两种都要认得出来。
    const out = withSortingFlags(cols, new Set(['actions', 'name']))
    expect(out.map((c) => c.enableSorting)).toEqual([false, true, true])
  })

  it('不改动原数组(避免调用方传的 $derived 列被就地污染)', () => {
    const original = [{ accessorKey: 'id', header: '编号' }]
    withSortingFlags(original, new Set(['id']))
    expect('enableSorting' in original[0]).toBe(false)
  })
})

describe('sortableIds 不得悬空(全仓)', () => {
  // 声明了非空 sortableIds 却不在 fetchPage 里转发 sort/order,
  // 后端 build_order_by 收不到 orderByColumn → 点了原样重取,又是「点了没反应」。
  const ROUTES = join(process.cwd(), 'src', 'routes')

  function walkSvelte(dir: string, out: string[] = []): string[] {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name)
      if (statSync(p).isDirectory()) walkSvelte(p, out)
      else if (p.endsWith('.svelte')) out.push(p)
    }
    return out
  }

  it('凡声明非空 sortableIds 的页面都转发了 orderByColumn', () => {
    const offenders = walkSvelte(ROUTES)
      .map((f) => ({ f, src: readFileSync(f, 'utf8') }))
      .filter(({ src }) => /sortableIds:\s*new Set\(\[\s*'/.test(src))
      .filter(({ src }) => !src.includes('orderByColumn'))
      .map(({ f }) => f.replaceAll('\\', '/').split('/src/')[1])
    expect(offenders).toEqual([])
  })
})
