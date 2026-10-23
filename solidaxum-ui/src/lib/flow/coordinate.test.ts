import { describe, expect, it } from 'vitest'
import {
  parseEdgeCoord,
  parseNodeCoord,
  serializeEdgeCoord,
  serializeNodeCoord,
} from './coordinate.js'

// 格式权威:warm-flow-ui tool.js(节点 :46-56 / 边 :107-124 解析,:173-185 / :228-231 序列化)。
describe('parseNodeCoord', () => {
  it('"cx,cy" → 中心点,无标签位', () => {
    expect(parseNodeCoord('300,200')).toEqual({ center: { x: 300, y: 200 } })
  })

  it('"cx,cy|tx,ty" → 中心点 + 标签绝对位', () => {
    expect(parseNodeCoord('700,300|700,340')).toEqual({
      center: { x: 700, y: 300 },
      text: { x: 700, y: 340 },
    })
  })

  it('空串 / null / undefined → null(走自动布局)', () => {
    expect(parseNodeCoord('')).toBeNull()
    expect(parseNodeCoord(null)).toBeNull()
    expect(parseNodeCoord(undefined)).toBeNull()
  })

  it('中心点残缺或非数字 → null', () => {
    expect(parseNodeCoord('300')).toBeNull()
    expect(parseNodeCoord('abc,200')).toBeNull()
    expect(parseNodeCoord('300,200,1')).toBeNull()
  })

  it('标签段是 NaN 时当作没有标签位', () => {
    expect(parseNodeCoord('300,200|NaN,NaN')).toEqual({
      center: { x: 300, y: 200 },
    })
  })

  it('小数与空白容错:取整', () => {
    expect(parseNodeCoord(' 300.4 , 199.6 ')).toEqual({
      center: { x: 300, y: 200 },
    })
  })
})

describe('parseEdgeCoord', () => {
  it('两点直线', () => {
    expect(parseEdgeCoord('410,200;490,200')).toEqual({
      points: [
        { x: 410, y: 200 },
        { x: 490, y: 200 },
      ],
    })
  })

  it('折线 + 标签位', () => {
    expect(parseEdgeCoord('700,275;700,200;810,200|700,237')).toEqual({
      points: [
        { x: 700, y: 275 },
        { x: 700, y: 200 },
        { x: 810, y: 200 },
      ],
      text: { x: 700, y: 237 },
    })
  })

  it('老设计器写出的 "|NaN,NaN" 标签段被忽略', () => {
    expect(parseEdgeCoord('410,200;490,200|NaN,NaN')).toEqual({
      points: [
        { x: 410, y: 200 },
        { x: 490, y: 200 },
      ],
    })
  })

  it('无效点被丢弃;剩余不足 2 点 → null', () => {
    expect(parseEdgeCoord('410,200;x,y;490,200')?.points).toHaveLength(2)
    expect(parseEdgeCoord('410,200;x,y')).toBeNull()
    expect(parseEdgeCoord('')).toBeNull()
    expect(parseEdgeCoord(null)).toBeNull()
  })
})

describe('serialize*', () => {
  it('节点总是两段式,数值取整', () => {
    expect(serializeNodeCoord({ x: 300.4, y: 199.5 }, { x: 300, y: 240 })).toBe(
      '300,200|300,240'
    )
  })

  it('边写完整点列 + 标签段', () => {
    expect(
      serializeEdgeCoord(
        [
          { x: 700, y: 275 },
          { x: 700, y: 200 },
          { x: 810, y: 200 },
        ],
        { x: 700, y: 237 }
      )
    ).toBe('700,275;700,200;810,200|700,237')
  })

  it('边点数 < 2 抛错;非有限数抛错而不是写出 NaN', () => {
    expect(() => serializeEdgeCoord([{ x: 1, y: 1 }], { x: 0, y: 0 })).toThrow()
    expect(() =>
      serializeNodeCoord({ x: Number.NaN, y: 1 }, { x: 0, y: 0 })
    ).toThrow(/有限数/)
  })

  it('parse(serialize(x)) 还原同一几何', () => {
    const points = [
      { x: 1, y: 2 },
      { x: 3, y: 2 },
      { x: 3, y: 9 },
    ]
    const text = { x: 2, y: 5 }
    expect(parseEdgeCoord(serializeEdgeCoord(points, text))).toEqual({
      points,
      text,
    })
    expect(parseNodeCoord(serializeNodeCoord({ x: 7, y: 8 }, text))).toEqual({
      center: { x: 7, y: 8 },
      text,
    })
  })
})
