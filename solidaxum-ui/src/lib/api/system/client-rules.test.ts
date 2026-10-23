import { describe, expect, it } from 'vitest'
import { parseRuleText, toRuleText, resolveRuleList } from './client-rules.js'

describe('parseRuleText', () => {
  // 分隔符集合对齐 Vue getRuleList(client/index.vue:360-368):换行/逗号/分号
  it('按换行切分 —— Vue 的 placeholder 示例就是换行', () => {
    expect(parseRuleText('127.0.0.1\n192.168.*.*\n10.0.0.0/24')).toEqual([
      '127.0.0.1', '192.168.*.*', '10.0.0.0/24',
    ])
  })
  it('按逗号切分', () => {
    expect(parseRuleText('/app/**,/api/**')).toEqual(['/app/**', '/api/**'])
  })
  it('按分号切分', () => {
    expect(parseRuleText('/a/**;/b/**')).toEqual(['/a/**', '/b/**'])
  })
  it('三种分隔符混用、连续分隔符合并', () => {
    expect(parseRuleText('/a/**,\n;/b/**\n\n,/c/**')).toEqual(['/a/**', '/b/**', '/c/**'])
  })
  it('逐项 trim,丢弃空串', () => {
    expect(parseRuleText('  /a/**  ,  ,  /b/**  ')).toEqual(['/a/**', '/b/**'])
  })
  it('空输入 → 空数组(语义:放行全部)', () => {
    expect(parseRuleText('')).toEqual([])
    expect(parseRuleText('   ')).toEqual([])
    expect(parseRuleText(undefined)).toEqual([])
    expect(parseRuleText(null)).toEqual([])
  })
})

describe('toRuleText', () => {
  // 提交时统一成逗号串:后端 split_comma_list 只认逗号(client.rs:123-130)
  it('合并为逗号串,后端才切得开', () => {
    expect(toRuleText(['/a/**', '/b/**'])).toBe('/a/**,/b/**')
  })
  it('空数组 → 空串(而不是 undefined,保持字段存在)', () => {
    expect(toRuleText([])).toBe('')
  })
  it('换行录入经 parse→to 往返后变成后端认得的逗号串', () => {
    expect(toRuleText(parseRuleText('127.0.0.1\n10.0.0.0/24'))).toBe('127.0.0.1,10.0.0.0/24')
  })
})

describe('resolveRuleList', () => {
  // 对齐 Vue getRuleList:优先用后端已切分的数组,没有才回退切原串
  it('后端给了数组就用数组', () => {
    expect(resolveRuleList(['/a/**'], '/ignored/**')).toEqual(['/a/**'])
  })
  it('数组为空则回退切原串', () => {
    expect(resolveRuleList([], '/a/**,/b/**')).toEqual(['/a/**', '/b/**'])
  })
  it('两者都空 → 空数组(调用方据此显示「全部路径」)', () => {
    expect(resolveRuleList(undefined, undefined)).toEqual([])
  })
})
