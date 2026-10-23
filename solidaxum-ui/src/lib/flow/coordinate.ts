import type { Point } from './types.js'

/** 节点 coordinate 解析结果:中心点 + 可选的标签绝对位。 */
export interface NodeCoord {
  center: Point
  text?: Point
}

/** 边 coordinate 解析结果:折线点列(≥ 2 点)+ 可选的标签绝对位。 */
export interface EdgeCoord {
  points: Point[]
  text?: Point
}

function parsePoint(s: string): Point | null {
  const [xs, ys, ...rest] = s.split(',')
  if (rest.length > 0 || xs === undefined || ys === undefined) return null
  const x = Number(xs.trim())
  const y = Number(ys.trim())
  // 老设计器可能写出 "NaN,NaN"(边标签对象存在但无坐标),按无效点处理。
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null
  return { x: Math.round(x), y: Math.round(y) }
}

/** `|` 之后的标签段;解析失败(含 NaN)视为没有标签位。 */
function parseText(seg: string | undefined): Point | undefined {
  if (seg === undefined) return undefined
  return parsePoint(seg) ?? undefined
}

/**
 * 解析节点坐标串 `"cx,cy"` / `"cx,cy|tx,ty"`。
 * 空串、null 或中心点无法解析 → null(调用方走自动布局)。
 */
export function parseNodeCoord(s: string | null | undefined): NodeCoord | null {
  if (!s) return null
  const [pos, textSeg] = s.split('|')
  const center = parsePoint(pos ?? '')
  if (!center) return null
  const text = parseText(textSeg)
  return text ? { center, text } : { center }
}

/**
 * 解析边坐标串 `"x1,y1;...;xn,yn"` / `"...|tx,ty"`。
 * 无效的点被丢弃;有效点不足 2 个 → null(调用方走自动路由)。
 */
export function parseEdgeCoord(s: string | null | undefined): EdgeCoord | null {
  if (!s) return null
  const [pts, textSeg] = s.split('|')
  const points = (pts ?? '')
    .split(';')
    .map(parsePoint)
    .filter((p): p is Point => p !== null)
  if (points.length < 2) return null
  const text = parseText(textSeg)
  return text ? { points, text } : { points }
}

function fmt(p: Point): string {
  if (!Number.isFinite(p.x) || !Number.isFinite(p.y)) {
    throw new Error(`坐标不是有限数: ${p.x},${p.y}`)
  }
  return `${Math.round(p.x)},${Math.round(p.y)}`
}

/** 节点坐标串。标签段总是写出,LogicFlow 两段式解析本就兼容。 */
export function serializeNodeCoord(center: Point, text: Point): string {
  return `${fmt(center)}|${fmt(text)}`
}

/** 边坐标串:完整点列 + 标签段。 */
export function serializeEdgeCoord(points: Point[], text: Point): string {
  if (points.length < 2) {
    throw new Error(`边至少需要 2 个点,收到 ${points.length} 个`)
  }
  return `${points.map(fmt).join(';')}|${fmt(text)}`
}
