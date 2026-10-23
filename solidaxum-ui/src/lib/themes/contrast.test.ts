import { describe, it, expect } from 'vitest'
import { parseOklch, relativeLuminance, contrastRatio } from './contrast'

describe('parseOklch', () => {
  it('解析标准 oklch(L C H) 三元组', () => {
    expect(parseOklch('oklch(0.662 0.1121 192.51)')).toEqual([0.662, 0.1121, 192.51])
  })

  it('解析无彩度写法 oklch(1 0 0)', () => {
    expect(parseOklch('oklch(1 0 0)')).toEqual([1, 0, 0])
  })

  it('忽略 alpha 分量,只取前三个', () => {
    expect(parseOklch('oklch(0.6486 0.1147 182.81 / 40%)')).toEqual([0.6486, 0.1147, 182.81])
  })

  it('非 oklch 值返回 null', () => {
    expect(parseOklch('none')).toBeNull()
    expect(parseOklch('linear-gradient(#fff, #000)')).toBeNull()
  })
})

describe('relativeLuminance', () => {
  it('白色为 1', () => {
    expect(relativeLuminance([1, 0, 0])).toBeCloseTo(1, 2)
  })

  it('黑色为 0', () => {
    expect(relativeLuminance([0, 0, 0])).toBeCloseTo(0, 3)
  })
})

describe('contrastRatio', () => {
  it('黑白对比度为 21', () => {
    expect(contrastRatio([1, 0, 0], [0, 0, 0])).toBeCloseTo(21, 1)
  })

  it('同色对比度为 1', () => {
    const c: [number, number, number] = [0.662, 0.1121, 192.51]
    expect(contrastRatio(c, c)).toBeCloseTo(1, 5)
  })

  it('与顺序无关', () => {
    const a: [number, number, number] = [0.662, 0.1121, 192.51]
    const b: [number, number, number] = [1, 0, 0]
    expect(contrastRatio(a, b)).toBeCloseTo(contrastRatio(b, a), 5)
  })

  it('青绿主色 #0BA8A5 对白 = 2.93(已知不达标,钉死)', () => {
    expect(contrastRatio([0.662, 0.1121, 192.51], [1, 0, 0])).toBeCloseTo(2.93, 1)
  })
})
