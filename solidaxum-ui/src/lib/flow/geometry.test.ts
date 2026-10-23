import { describe, expect, it } from 'vitest'
import {
  anchorPoint,
  centerToPosition,
  DEFAULT_LABEL_OFFSET,
  facingSide,
  nearestSide,
  NODE_SIZE,
  nodeKindOf,
  positionToCenter,
  SIDES,
} from './geometry.js'

describe('NODE_SIZE 对齐 LogicFlow 节点尺寸', () => {
  it('start / end 圆 r=25 → 50×50', () => {
    expect(NODE_SIZE.start).toEqual({ w: 50, h: 50 })
    expect(NODE_SIZE.end).toEqual({ w: 50, h: 50 })
  })

  it('between 圆角矩形 100×80', () => {
    expect(NODE_SIZE.between).toEqual({ w: 100, h: 80 })
  })

  it('gateway 菱形 50×50', () => {
    expect(NODE_SIZE.gateway).toEqual({ w: 50, h: 50 })
  })

  it('半宽半高都是整数,中心↔左上角换算才无损', () => {
    for (const { w, h } of Object.values(NODE_SIZE)) {
      expect(w % 2).toBe(0)
      expect(h % 2).toBe(0)
    }
  })
})

describe('nodeKindOf', () => {
  it('0 / 1 / 2 → start / between / end', () => {
    expect(nodeKindOf(0)).toBe('start')
    expect(nodeKindOf(1)).toBe('between')
    expect(nodeKindOf(2)).toBe('end')
  })

  it('3 / 4 / 5 三种网关共用 gateway', () => {
    expect(nodeKindOf(3)).toBe('gateway')
    expect(nodeKindOf(4)).toBe('gateway')
    expect(nodeKindOf(5)).toBe('gateway')
  })

  it('未知 nodeType 抛错而不是静默归类', () => {
    expect(() => nodeKindOf(6)).toThrow(/nodeType/)
    expect(() => nodeKindOf(-1)).toThrow(/nodeType/)
  })
})

describe('SIDES', () => {
  it('顺序固定为 top / right / bottom / left(nearestSide 并列取舍依据)', () => {
    expect(SIDES).toEqual(['top', 'right', 'bottom', 'left'])
  })
})

describe('中心点 ↔ position', () => {
  it('between 中心 (400,300) → 左上角 (350,260)', () => {
    expect(centerToPosition({ x: 400, y: 300 }, 'between')).toEqual({
      x: 350,
      y: 260,
    })
  })

  it('gateway 中心 (700,300) → 左上角 (675,275)', () => {
    expect(centerToPosition({ x: 700, y: 300 }, 'gateway')).toEqual({
      x: 675,
      y: 275,
    })
  })

  it('往返无损(四种尺寸)', () => {
    for (const kind of ['start', 'between', 'end', 'gateway'] as const) {
      const c = { x: 123, y: 456 }
      expect(positionToCenter(centerToPosition(c, kind), kind)).toEqual(c)
    }
  })
})

describe('anchorPoint', () => {
  it('between (400,300):四侧锚点在外接盒四边中点', () => {
    const c = { x: 400, y: 300 }
    expect(anchorPoint(c, 'between', 'top')).toEqual({ x: 400, y: 260 })
    expect(anchorPoint(c, 'between', 'right')).toEqual({ x: 450, y: 300 })
    expect(anchorPoint(c, 'between', 'bottom')).toEqual({ x: 400, y: 340 })
    expect(anchorPoint(c, 'between', 'left')).toEqual({ x: 350, y: 300 })
  })

  it('gateway (700,300):四个菱形顶点', () => {
    const c = { x: 700, y: 300 }
    expect(anchorPoint(c, 'gateway', 'top')).toEqual({ x: 700, y: 275 })
    expect(anchorPoint(c, 'gateway', 'right')).toEqual({ x: 725, y: 300 })
    expect(anchorPoint(c, 'gateway', 'bottom')).toEqual({ x: 700, y: 325 })
    expect(anchorPoint(c, 'gateway', 'left')).toEqual({ x: 675, y: 300 })
  })

  it('start (240,300):圆周四点', () => {
    const c = { x: 240, y: 300 }
    expect(anchorPoint(c, 'start', 'right')).toEqual({ x: 265, y: 300 })
    expect(anchorPoint(c, 'start', 'top')).toEqual({ x: 240, y: 275 })
  })
})

describe('nearestSide', () => {
  const c = { x: 400, y: 300 }

  it('点正好在锚点上 → 该侧', () => {
    expect(nearestSide(c, 'between', { x: 450, y: 300 })).toBe('right')
    expect(nearestSide(c, 'between', { x: 400, y: 340 })).toBe('bottom')
    expect(nearestSide(c, 'between', { x: 350, y: 300 })).toBe('left')
    expect(nearestSide(c, 'between', { x: 400, y: 260 })).toBe('top')
  })

  it('半径 20 时代的旧线头 (260,300) 仍归到 r=25 的右侧', () => {
    expect(nearestSide({ x: 240, y: 300 }, 'start', { x: 260, y: 300 })).toBe(
      'right'
    )
  })

  it('等距(点在中心)按 SIDES 顺序取 top', () => {
    expect(nearestSide(c, 'between', c)).toBe('top')
  })

  it('between 的对角点:离右边中点更近就归右', () => {
    // (445,330):到 right(450,300) 距离 ≈30.4,到 bottom(400,340) ≈46
    expect(nearestSide(c, 'between', { x: 445, y: 330 })).toBe('right')
  })
})

describe('facingSide', () => {
  const a = { x: 0, y: 0 }

  it('四象限:横向更远走左右,否则走上下', () => {
    expect(facingSide(a, { x: 100, y: 10 })).toBe('right')
    expect(facingSide(a, { x: -100, y: 10 })).toBe('left')
    expect(facingSide(a, { x: 10, y: 100 })).toBe('bottom')
    expect(facingSide(a, { x: 10, y: -100 })).toBe('top')
  })

  it('|dx| == |dy| 时走上下;重合视为在下方', () => {
    expect(facingSide(a, { x: 50, y: 50 })).toBe('bottom')
    expect(facingSide(a, { x: 50, y: -50 })).toBe('top')
    expect(facingSide(a, a)).toBe('bottom')
  })

  it('反向观察互为相对侧', () => {
    const b = { x: 300, y: 0 }
    expect(facingSide(a, b)).toBe('right')
    expect(facingSide(b, a)).toBe('left')
  })
})

describe('DEFAULT_LABEL_OFFSET', () => {
  it('网关标签在菱形下方 40px,其余居中', () => {
    expect(DEFAULT_LABEL_OFFSET.gateway).toEqual({ x: 0, y: 40 })
    expect(DEFAULT_LABEL_OFFSET.between).toEqual({ x: 0, y: 0 })
    expect(DEFAULT_LABEL_OFFSET.start).toEqual({ x: 0, y: 0 })
    expect(DEFAULT_LABEL_OFFSET.end).toEqual({ x: 0, y: 0 })
  })
})
