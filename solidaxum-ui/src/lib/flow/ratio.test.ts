import { describe, expect, it } from 'vitest'
import {
  formatNodeRatio,
  parseNodeRatio,
  ratioForWay,
  validateNodeRatio,
} from './ratio.js'

// 语义锚在引擎:vote.rs / CooperateType(0 或签,100 会签,其余票签,@@sequence 依次)
describe('parseNodeRatio', () => {
  it('空 / "0" → 或签;"100" → 会签', () => {
    expect(parseNodeRatio(null).way).toBe('1')
    expect(parseNodeRatio('').way).toBe('1')
    expect(parseNodeRatio('0').way).toBe('1')
    expect(parseNodeRatio('100').way).toBe('3')
  })

  it('导入件里的 "0.000" / "100.0" 按数值归到或签 / 会签', () => {
    expect(parseNodeRatio('0.000').way).toBe('1')
    expect(parseNodeRatio('100.0').way).toBe('3')
    expect(formatNodeRatio(parseNodeRatio('0.000'))).toBe('0')
  })

  it('数字 → 票签通过率;passCount/rejectCount → 固定票数', () => {
    expect(parseNodeRatio('50')).toMatchObject({
      way: '2',
      type: 'passRatio',
      value: '50',
    })
    expect(parseNodeRatio('passCount=2')).toMatchObject({
      way: '2',
      type: 'passCount',
      value: '2',
    })
    expect(parseNodeRatio('rejectCount=1')).toMatchObject({
      way: '2',
      type: 'rejectCount',
      value: '1',
    })
  })

  it('@@sequence 后缀单独记录', () => {
    expect(parseNodeRatio('100@@sequence')).toMatchObject({
      way: '3',
      sequence: true,
    })
    expect(parseNodeRatio('passCount=2@@sequence')).toMatchObject({
      type: 'passCount',
      value: '2',
      sequence: true,
    })
  })

  it('引擎不认的策略表达式按 raw 保留', () => {
    expect(parseNodeRatio('spel@@#{@x.eval()}')).toEqual({
      way: '2',
      type: 'raw',
      value: 'spel@@#{@x.eval()}',
      sequence: false,
    })
  })
})

describe('formatNodeRatio', () => {
  it('parse ∘ format 还原;或签/会签写 0/100', () => {
    for (const s of [
      '0',
      '100',
      '50',
      '66.7',
      'passCount=3',
      'rejectCount=2',
      '100@@sequence',
      'spel@@x',
    ]) {
      expect(formatNodeRatio(parseNodeRatio(s))).toBe(s)
    }
    expect(formatNodeRatio(parseNodeRatio(''))).toBe('0')
  })

  it('ratioForWay:票签默认 50', () => {
    expect(formatNodeRatio(ratioForWay('1'))).toBe('0')
    expect(formatNodeRatio(ratioForWay('2'))).toBe('50')
    expect(formatNodeRatio(ratioForWay('3'))).toBe('100')
  })
})

describe('validateNodeRatio(对齐 between.vue 校验)', () => {
  const r = (
    type: 'passRatio' | 'passCount' | 'rejectCount' | 'raw',
    value: string
  ) => validateNodeRatio({ way: '2', type, value, sequence: false })

  it('或签/会签不校验', () => {
    expect(
      validateNodeRatio({
        way: '1',
        type: 'passRatio',
        value: '',
        sequence: false,
      })
    ).toBeNull()
    expect(
      validateNodeRatio({
        way: '3',
        type: 'passRatio',
        value: 'x',
        sequence: false,
      })
    ).toBeNull()
  })

  it('通过率:0.001–100,最多三位小数', () => {
    expect(r('passRatio', '')).toMatch(/通过率/)
    expect(r('passRatio', 'abc')).toMatch(/数字/)
    expect(r('passRatio', '0')).toMatch(/0.001/)
    expect(r('passRatio', '101')).toMatch(/0.001/)
    expect(r('passRatio', '12.3456')).toMatch(/三位/)
    expect(r('passRatio', '66.667')).toBeNull()
    expect(r('passRatio', '100')).toBeNull()
  })

  it('固定人数:正整数', () => {
    expect(r('passCount', '0')).toMatch(/正整数/)
    expect(r('passCount', '2.5')).toMatch(/正整数/)
    expect(r('rejectCount', '3')).toBeNull()
  })

  it('raw 只要非空', () => {
    expect(r('raw', '')).not.toBeNull()
    expect(r('raw', 'spel@@x')).toBeNull()
  })
})
