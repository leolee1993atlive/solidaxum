import { describe, expect, it } from 'vitest'
import { jobLogStatusLabel } from './joblog.js'

describe('jobLogStatusLabel', () => {
  // 后端真值: scheduler.rs:144-147,205-208 → Ok=>"1", Err=>"0"
  it('"1" 是成功', () => {
    expect(jobLogStatusLabel('1')).toBe('成功')
  })
  it('"0" 是失败', () => {
    expect(jobLogStatusLabel('0')).toBe('失败')
  })
  it('未知值原样返回', () => {
    expect(jobLogStatusLabel('9')).toBe('9')
  })
  it('undefined 返回占位符', () => {
    expect(jobLogStatusLabel(undefined)).toBe('-')
  })
})
