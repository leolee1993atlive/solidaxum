import { describe, expect, it } from 'vitest'
import {
  COMPARE_OPS,
  formatSkipCondition,
  parseSkipCondition,
  validateSkipCondition,
} from './condition.js'

// 语义锚在引擎 crates/flow/src/expression/condition.rs
describe('parseSkipCondition', () => {
  it('空 → none', () => {
    expect(parseSkipCondition(null)).toEqual({ kind: 'none' })
    expect(parseSkipCondition('  ')).toEqual({ kind: 'none' })
  })

  it('比较条件 op@@变量|值', () => {
    expect(parseSkipCondition('gt@@leaveDays|2')).toEqual({
      kind: 'compare',
      op: 'gt',
      variable: 'leaveDays',
      value: '2',
    })
    expect(parseSkipCondition('like@@title|紧急')).toMatchObject({
      op: 'like',
      value: '紧急',
    })
  })

  it('值里可以再出现 |,只按第一个 | 切', () => {
    expect(parseSkipCondition('eq@@code|a|b')).toMatchObject({
      variable: 'code',
      value: 'a|b',
    })
  })

  it('spel@@ 取整段表达式', () => {
    expect(parseSkipCondition('spel@@#amount > 1000 && #type == "a"')).toEqual({
      kind: 'spel',
      expr: '#amount > 1000 && #type == "a"',
    })
  })

  it('引擎不认的 default@@ / snel@@ / 无 @@ 的串 → raw 原样保留', () => {
    expect(parseSkipCondition('default@@${leaveDays > 2}')).toEqual({
      kind: 'raw',
      raw: 'default@@${leaveDays > 2}',
    })
    expect(parseSkipCondition('snel@@#{x}')).toMatchObject({ kind: 'raw' })
    expect(parseSkipCondition('whatever')).toMatchObject({ kind: 'raw' })
    expect(parseSkipCondition('gt@@noBar')).toMatchObject({ kind: 'raw' })
  })
})

describe('formatSkipCondition', () => {
  it('parse ∘ format 还原;none 写 null', () => {
    for (const s of [
      'gt@@leaveDays|2',
      'spel@@#amount > 1000',
      'default@@${x}',
    ]) {
      expect(formatSkipCondition(parseSkipCondition(s))).toBe(s)
    }
    expect(formatSkipCondition({ kind: 'none' })).toBeNull()
  })

  it('比较条件与 spel 都去首尾空白', () => {
    expect(
      formatSkipCondition({
        kind: 'compare',
        op: 'eq',
        variable: ' a ',
        value: ' 1 ',
      })
    ).toBe('eq@@a|1')
    expect(formatSkipCondition({ kind: 'spel', expr: ' #a > 1 ' })).toBe(
      'spel@@#a > 1'
    )
  })
})

describe('validateSkipCondition', () => {
  it('比较条件:变量与值都要有', () => {
    expect(
      validateSkipCondition({
        kind: 'compare',
        op: 'gt',
        variable: '',
        value: '1',
      })
    ).toMatch(/变量/)
    expect(
      validateSkipCondition({
        kind: 'compare',
        op: 'gt',
        variable: 'a',
        value: ' ',
      })
    ).toMatch(/比较值/)
    expect(
      validateSkipCondition({
        kind: 'compare',
        op: 'gt',
        variable: 'a',
        value: '1',
      })
    ).toBeNull()
  })

  it('spel:非空,且不能用 Java 风格 #{ } 包裹(引擎按 rhai 直接求值)', () => {
    expect(validateSkipCondition({ kind: 'spel', expr: '' })).toMatch(/spel/)
    expect(validateSkipCondition({ kind: 'spel', expr: '#{#a > 1}' })).toMatch(
      /包裹/
    )
    expect(validateSkipCondition({ kind: 'spel', expr: '#a > 1' })).toBeNull()
    expect(validateSkipCondition({ kind: 'spel', expr: 'a > 1' })).toBeNull()
  })

  it('raw 一律提示不支持;none 通过', () => {
    expect(
      validateSkipCondition({ kind: 'raw', raw: 'default@@${x}' })
    ).toMatch(/不支持/)
    expect(validateSkipCondition({ kind: 'none' })).toBeNull()
  })

  it('比较算子共 8 个,与引擎 match 分支一致', () => {
    expect(COMPARE_OPS.map((o) => o.value)).toEqual([
      'gt',
      'ge',
      'eq',
      'ne',
      'lt',
      'le',
      'like',
      'notLike',
    ])
  })
})
