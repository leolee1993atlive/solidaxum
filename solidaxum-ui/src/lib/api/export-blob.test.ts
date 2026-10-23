import { afterEach, describe, expect, it, vi } from 'vitest'
import { downloadBlob, exportBlob, toFormBody } from './download.js'

describe('toFormBody', () => {
  // 后端三个导出端点都用 Form<...> 提取(serde_urlencoded),空值必须剥掉 ——
  // 发 status= 空串会被 serde 当成 Some("") 而不是 None,筛出零行。
  it('剥掉 undefined / null / 空串', () => {
    expect(toFormBody({ a: '1', b: undefined, c: null, d: '' })).toBe('a=1')
  })
  it('数字转字符串', () => {
    expect(toFormBody({ pageNum: 1 })).toBe('pageNum=1')
  })
  it('对键值做 urlencode(中文筛选词不能裸发)', () => {
    expect(toFormBody({ configName: '主框架' })).toBe(
      'configName=%E4%B8%BB%E6%A1%86%E6%9E%B6'
    )
  })
  it('无入参 → 空串(后端 Option 字段缺省即 None)', () => {
    expect(toFormBody()).toBe('')
    expect(toFormBody({})).toBe('')
  })
  it('保留 0 与 false 这类假值(它们是有效筛选值)', () => {
    expect(toFormBody({ isFrame: 0 })).toBe('isFrame=0')
  })
})

describe('导出失败时透出后端文案', () => {
  /**
   * exportBlob / downloadBlob 绕开 apiClient 自己 fetch,所以也绕开了它的
   * 错误解包。不主动读 body 就只剩状态码 —— 后端的「导出行数 N 超过单次
   * 上限 50000,请缩小查询范围」会整条丢掉,用户看到的是「导出失败: 400」,
   * 既不知道超了多少也不知道该怎么办。这几条钉的就是这段链路。
   */
  function stubFetch(resp: Partial<Response> & { json?: () => Promise<unknown> }) {
    const full = {
      ok: false,
      status: 400,
      clone() {
        return this
      },
      ...resp,
    }
    vi.stubGlobal('fetch', vi.fn(async () => full as unknown as Response))
  }

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('把 R 信封的 msg 原样抛出(调用方 toast 的就是它)', async () => {
    const msg = '导出行数 123456 超过单次上限 50000,请缩小查询范围(如限定时间段)后重试'
    stubFetch({ json: async () => ({ code: 400, msg }) })
    await expect(exportBlob('/monitor/operlog/export', 'x.xlsx')).rejects.toThrow(msg)
  })

  it('响应体不是 JSON 时退回状态码,不抛解析错', async () => {
    stubFetch({
      status: 502,
      json: async () => {
        throw new SyntaxError('Unexpected token < in JSON')
      },
    })
    await expect(exportBlob('/monitor/operlog/export', 'x.xlsx')).rejects.toThrow(
      '导出失败: 502'
    )
  })

  it('msg 为空串时也退回状态码(空 toast 等于静默)', async () => {
    stubFetch({ json: async () => ({ code: 400, msg: '   ' }) })
    await expect(exportBlob('/monitor/operlog/export', 'x.xlsx')).rejects.toThrow(
      '导出失败: 400'
    )
  })

  it('downloadBlob 走同一条解包(zip/oss 下载共用)', async () => {
    stubFetch({ status: 404, json: async () => ({ code: 404, msg: '文件不存在' }) })
    await expect(downloadBlob('/tool/gen/download/1', 'x.zip')).rejects.toThrow(
      '文件不存在'
    )
  })
})
