import { describe, expect, it } from 'vitest'
import { slicePage } from './online.js'

describe('slicePage', () => {
  const all = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]

  it('第 1 页取前 pageSize 条', () => {
    expect(slicePage(all, 1, 5)).toEqual([1, 2, 3, 4, 5])
  })
  it('第 2 页取接下来 pageSize 条(不与第 1 页相同)', () => {
    expect(slicePage(all, 2, 5)).toEqual([6, 7, 8, 9, 10])
  })
  it('末页不足 pageSize 时只返回剩余', () => {
    expect(slicePage(all, 3, 5)).toEqual([11, 12])
  })
  it('超出范围返回空数组', () => {
    expect(slicePage(all, 99, 5)).toEqual([])
  })
  it('空输入返回空数组', () => {
    expect(slicePage([], 1, 10)).toEqual([])
  })
})
