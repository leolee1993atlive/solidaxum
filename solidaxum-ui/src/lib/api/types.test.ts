import { describe, it, expect } from 'vitest'
import {
  toPageQuery,
  type ApiResponse,
  type TableData,
  type ListResult,
  type PageQuery,
} from './types'

describe('ApiResponse envelope', () => {
  it('uses msg not message', () => {
    const r: ApiResponse<string> = { code: 200, msg: 'ok', data: 'x' }
    expect(r.msg).toBe('ok')
    expect('message' in r).toBe(false)
  })
})

describe('TableData is nested shape', () => {
  it('has rows and total at data level', () => {
    const inner: TableData<string> = { code: 200, msg: '查询成功', rows: ['a'], total: 1 }
    expect(inner.rows).toEqual(['a'])
    expect(inner.total).toBe(1)
  })
})

describe('toPageQuery', () => {
  it('serializes pageNum/pageSize camelCase', () => {
    const q: PageQuery = { pageNum: 2, pageSize: 10 }
    expect(toPageQuery(q)).toEqual({ pageNum: '2', pageSize: '10' })
  })
  it('omits undefined optional fields', () => {
    expect(toPageQuery({ pageNum: 1, pageSize: 5, orderByColumn: 'createTime', isAsc: 'desc' })).toEqual({
      pageNum: '1',
      pageSize: '5',
      orderByColumn: 'createTime',
      isAsc: 'desc',
    })
  })
  it('merges extra filters and skips empty/null', () => {
    expect(
      toPageQuery({ pageNum: 1, pageSize: 5 }, { roleName: 'admin', status: '', keyword: undefined })
    ).toEqual({ pageNum: '1', pageSize: '5', roleName: 'admin' })
  })
  it('preserves numeric 0 as a valid filter (not dropped as falsy)', () => {
    expect(toPageQuery({ pageNum: 1, pageSize: 5 }, { status: 0 })).toEqual({
      pageNum: '1',
      pageSize: '5',
      status: '0',
    })
  })
})
