import { describe, expect, it } from 'vitest'
import {
  midpoint,
  orthogonalRoute,
  ROUTE_OFFSET,
  simplifyOrthogonal,
} from './route.js'

describe('simplifyOrthogonal', () => {
  it('去掉连续重复点', () => {
    expect(
      simplifyOrthogonal([
        { x: 0, y: 0 },
        { x: 0, y: 0 },
        { x: 5, y: 0 },
      ])
    ).toEqual([
      { x: 0, y: 0 },
      { x: 5, y: 0 },
    ])
  })

  it('去掉同一直线上的中间点(含折返点)', () => {
    expect(
      simplifyOrthogonal([
        { x: 0, y: 0 },
        { x: 0, y: 20 },
        { x: 0, y: -80 },
        { x: 0, y: -100 },
      ])
    ).toEqual([
      { x: 0, y: 0 },
      { x: 0, y: -100 },
    ])
  })

  it('真正的拐角保留', () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 0, y: 50 },
      { x: 80, y: 50 },
    ]
    expect(simplifyOrthogonal(pts)).toEqual(pts)
  })
})

describe('orthogonalRoute', () => {
  it('默认外伸 20,对齐 LogicFlow', () => {
    expect(ROUTE_OFFSET).toBe(20)
  })

  it('同轴(目标正下方):退化为一条竖线', () => {
    expect(
      orthogonalRoute({ x: 300, y: 340 }, 'bottom', { x: 300, y: 460 }, 'top')
    ).toEqual([
      { x: 300, y: 340 },
      { x: 300, y: 460 },
    ])
  })

  it('竖向起始、目标在右下:下-右-下 三段,中线取两外伸点的 y 中点', () => {
    expect(
      orthogonalRoute({ x: 300, y: 340 }, 'bottom', { x: 500, y: 500 }, 'top')
    ).toEqual([
      { x: 300, y: 340 },
      { x: 300, y: 420 }, // (340+20 + 500-20) / 2
      { x: 500, y: 420 },
      { x: 500, y: 500 },
    ])
  })

  it('横向起始、目标在右上:右-上-右 三段,中线取 x 中点', () => {
    expect(
      orthogonalRoute({ x: 450, y: 300 }, 'right', { x: 650, y: 100 }, 'left')
    ).toEqual([
      { x: 450, y: 300 },
      { x: 550, y: 300 }, // (450+20 + 650-20) / 2
      { x: 550, y: 100 },
      { x: 650, y: 100 },
    ])
  })

  it('外伸点同 y 时直接相连(横向同轴)', () => {
    expect(
      orthogonalRoute({ x: 450, y: 300 }, 'right', { x: 650, y: 300 }, 'left')
    ).toEqual([
      { x: 450, y: 300 },
      { x: 650, y: 300 },
    ])
  })

  it('目标在正上方且从底部出发:折返被吸收成一条竖线(观感问题,数据正确)', () => {
    expect(
      orthogonalRoute({ x: 300, y: 340 }, 'bottom', { x: 300, y: 100 }, 'top')
    ).toEqual([
      { x: 300, y: 340 },
      { x: 300, y: 100 },
    ])
  })

  it('中点取整:奇数和向上取整', () => {
    const pts = orthogonalRoute(
      { x: 0, y: 0 },
      'bottom',
      { x: 100, y: 101 },
      'top'
    )
    // p1=(0,20), q1=(100,81) → midY = round(50.5) = 51
    expect(pts[1]).toEqual({ x: 0, y: 51 })
    expect(pts[2]).toEqual({ x: 100, y: 51 })
  })

  it('首点 = src,末点 = tgt,全部整数', () => {
    const pts = orthogonalRoute(
      { x: 12.4, y: 7.6 },
      'left',
      { x: 99.5, y: 200.2 },
      'right'
    )
    expect(pts[0]).toEqual({ x: 12, y: 8 })
    expect(pts[pts.length - 1]).toEqual({ x: 100, y: 200 })
    for (const p of pts) {
      expect(Number.isInteger(p.x) && Number.isInteger(p.y)).toBe(true)
    }
  })
})

describe('midpoint', () => {
  it('两点:线段中点', () => {
    expect(
      midpoint([
        { x: 0, y: 0 },
        { x: 100, y: 0 },
      ])
    ).toEqual({ x: 50, y: 0 })
  })

  it('三点折线:按弧长落在较长段上', () => {
    // 段长 75 + 110 = 185,半程 92.5 → 第二段上距 (700,200) 17.5
    expect(
      midpoint([
        { x: 700, y: 275 },
        { x: 700, y: 200 },
        { x: 810, y: 200 },
      ])
    ).toEqual({ x: 717.5, y: 200 })
  })

  it('偶数段且对称:落在拐点', () => {
    expect(
      midpoint([
        { x: 0, y: 0 },
        { x: 0, y: 50 },
        { x: 50, y: 50 },
      ])
    ).toEqual({ x: 0, y: 50 })
  })

  it('单点 / 零长度 → 该点;空列抛错', () => {
    expect(midpoint([{ x: 3, y: 4 }])).toEqual({ x: 3, y: 4 })
    expect(
      midpoint([
        { x: 3, y: 4 },
        { x: 3, y: 4 },
      ])
    ).toEqual({ x: 3, y: 4 })
    expect(() => midpoint([])).toThrow()
  })
})
