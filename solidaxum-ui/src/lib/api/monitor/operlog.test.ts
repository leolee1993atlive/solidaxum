import { describe, expect, it } from 'vitest'
import { prettyJson } from './operlog.js'

describe('prettyJson', () => {
  it('合法 JSON 缩进美化', () => {
    expect(prettyJson('{"a":1,"b":[2,3]}')).toBe(
      '{\n  "a": 1,\n  "b": [\n    2,\n    3\n  ]\n}'
    )
  })
  it('非 JSON 原样返回,不抛异常', () => {
    expect(prettyJson('not json at all')).toBe('not json at all')
  })
  it('空值返回占位符', () => {
    expect(prettyJson('')).toBe('-')
    expect(prettyJson(undefined)).toBe('-')
  })
})
