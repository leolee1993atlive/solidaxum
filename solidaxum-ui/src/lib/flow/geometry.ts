import type { NodeKind, Point, Side, Size } from './types.js'

/**
 * 节点外接盒尺寸,与 LogicFlow 各 NodeModel 一致(start/end r=25,
 * between 100×80,网关菱形 50×50)。画布组件的 DOM 盒子必须写死为此尺寸:
 * 中心点↔左上角换算与四侧锚点位置都以它为准,被内容撑开就会对不上老数据。
 */
export const NODE_SIZE: Readonly<Record<NodeKind, Size>> = {
  start: { w: 50, h: 50 },
  between: { w: 100, h: 80 },
  end: { w: 50, h: 50 },
  gateway: { w: 50, h: 50 },
}

/** 四侧锚点的固定顺序;多侧等距时按此顺序取首个,保证推断可复现。 */
export const SIDES: readonly Side[] = ['top', 'right', 'bottom', 'left']

/** 后端 nodeType(enums.rs 0..5)→ 画布节点种类;三种网关共用一个组件。 */
export function nodeKindOf(nodeType: number): NodeKind {
  switch (nodeType) {
    case 0:
      return 'start'
    case 1:
      return 'between'
    case 2:
      return 'end'
    case 3:
    case 4:
    case 5:
      return 'gateway'
    default:
      throw new Error(`未知的 nodeType: ${nodeType}`)
  }
}

/**
 * 节点标签相对中心的默认偏移。LogicFlow 里矩形/圆的文字居中,
 * 网关的文字放在菱形下方 40px(serial.js `y: data.y + 40`)。
 */
export const DEFAULT_LABEL_OFFSET: Readonly<Record<NodeKind, Point>> = {
  start: { x: 0, y: 0 },
  between: { x: 0, y: 0 },
  end: { x: 0, y: 0 },
  gateway: { x: 0, y: 40 },
}

/** LogicFlow 存中心点,Svelte Flow 的 position 是左上角。 */
export function centerToPosition(center: Point, kind: NodeKind): Point {
  const { w, h } = NODE_SIZE[kind]
  return { x: center.x - w / 2, y: center.y - h / 2 }
}

export function positionToCenter(position: Point, kind: NodeKind): Point {
  const { w, h } = NODE_SIZE[kind]
  return { x: position.x + w / 2, y: position.y + h / 2 }
}

/** 某一侧锚点的坐标:外接盒四边中点(圆的四点、菱形的四个顶点与之重合)。 */
export function anchorPoint(center: Point, kind: NodeKind, side: Side): Point {
  const { w, h } = NODE_SIZE[kind]
  switch (side) {
    case 'top':
      return { x: center.x, y: center.y - h / 2 }
    case 'right':
      return { x: center.x + w / 2, y: center.y }
    case 'bottom':
      return { x: center.x, y: center.y + h / 2 }
    case 'left':
      return { x: center.x - w / 2, y: center.y }
  }
}

/**
 * 离给定点最近的一侧;等距时按 SIDES 顺序取首个。
 * 老数据的线头就落在锚点上,推断是精确的;半径改过的旧节点线头会偏几像素,
 * 仍能正确归到原来那一侧。
 */
export function nearestSide(center: Point, kind: NodeKind, p: Point): Side {
  let best: Side = SIDES[0]
  let bestD = Number.POSITIVE_INFINITY
  for (const side of SIDES) {
    const a = anchorPoint(center, kind, side)
    const d = (a.x - p.x) ** 2 + (a.y - p.y) ** 2
    if (d < bestD) {
      best = side
      bestD = d
    }
  }
  return best
}

/**
 * 从 a 看向 b 时,a 朝向 b 的那一侧。横向距离更大走左右,否则走上下
 * (含正上/正下与重合的情况:重合时视为 b 在下方)。
 */
export function facingSide(a: Point, b: Point): Side {
  const dx = b.x - a.x
  const dy = b.y - a.y
  if (Math.abs(dx) > Math.abs(dy)) return dx > 0 ? 'right' : 'left'
  return dy >= 0 ? 'bottom' : 'top'
}
