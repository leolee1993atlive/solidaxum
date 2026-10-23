import type { Point, Side } from './types.js'

/** 边离开节点时先垂直外伸的距离,对齐 LogicFlow PolylineEdge 的 offset。 */
export const ROUTE_OFFSET = 20

const OUTWARD: Readonly<Record<Side, Point>> = {
  top: { x: 0, y: -1 },
  right: { x: 1, y: 0 },
  bottom: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
}

function isVertical(side: Side): boolean {
  return side === 'top' || side === 'bottom'
}

function collinear(a: Point, b: Point, c: Point): boolean {
  return (a.x === b.x && b.x === c.x) || (a.y === b.y && b.y === c.y)
}

/**
 * 去掉重复点与共线的中间点。折线只有横竖段,所以"共线"就是三点同 x 或同 y;
 * 回头的点(先外伸再折返)也在此被吸收,画出来本就是同一条线。
 */
export function simplifyOrthogonal(points: Point[]): Point[] {
  const out: Point[] = []
  for (const p of points) {
    const last = out[out.length - 1]
    if (last && last.x === p.x && last.y === p.y) continue
    out.push(p)
    while (
      out.length >= 3 &&
      collinear(out[out.length - 3], out[out.length - 2], out[out.length - 1])
    ) {
      out.splice(out.length - 2, 1)
    }
  }
  return out
}

/**
 * 两锚点之间的正交折线:各自沿所在侧外伸 offset,再用一段中线相连。
 * 输出是整数角点,首点 = src、末点 = tgt。
 */
export function orthogonalRoute(
  src: Point,
  srcSide: Side,
  tgt: Point,
  tgtSide: Side,
  offset = ROUTE_OFFSET
): Point[] {
  const p1 = {
    x: src.x + OUTWARD[srcSide].x * offset,
    y: src.y + OUTWARD[srcSide].y * offset,
  }
  const q1 = {
    x: tgt.x + OUTWARD[tgtSide].x * offset,
    y: tgt.y + OUTWARD[tgtSide].y * offset,
  }
  let mids: Point[] = []
  if (p1.x !== q1.x && p1.y !== q1.y) {
    if (isVertical(srcSide)) {
      const midY = Math.round((p1.y + q1.y) / 2)
      mids = [
        { x: p1.x, y: midY },
        { x: q1.x, y: midY },
      ]
    } else {
      const midX = Math.round((p1.x + q1.x) / 2)
      mids = [
        { x: midX, y: p1.y },
        { x: midX, y: q1.y },
      ]
    }
  }
  const raw = [src, p1, ...mids, q1, tgt].map((p) => ({
    x: Math.round(p.x),
    y: Math.round(p.y),
  }))
  return simplifyOrthogonal(raw)
}

function dist(a: Point, b: Point): number {
  return Math.hypot(b.x - a.x, b.y - a.y)
}

/** 折线按弧长的中点(不取整,给标签定位用)。 */
export function midpoint(points: Point[]): Point {
  if (points.length === 0) throw new Error('midpoint: 点列为空')
  if (points.length === 1) return { ...points[0] }
  let total = 0
  for (let i = 1; i < points.length; i++)
    total += dist(points[i - 1], points[i])
  if (total === 0) return { ...points[0] }
  let remain = total / 2
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1]
    const b = points[i]
    const d = dist(a, b)
    if (remain <= d) {
      const t = d === 0 ? 0 : remain / d
      return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
    }
    remain -= d
  }
  return { ...points[points.length - 1] }
}
