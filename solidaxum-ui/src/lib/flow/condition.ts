/**
 * 跳转条件 skipCondition 的编解码。
 *
 * 引擎支持的形式(crates/flow/src/expression/condition.rs):
 *   "<op>@@<变量>|<值>",op ∈ eq/ne/gt/ge/lt/le/like/notLike;
 *   "spel@@<表达式>"(rhai 求值的 SpEL 子集)。
 * Java 设计器还会写 default@@${...} / snel@@#{...},当前引擎会报"无法解析",
 * 这里按 raw 原样保留并在面板提示,不主动提供这两种选项。
 */

export const COMPARE_OPS = [
  { value: 'gt', label: '大于' },
  { value: 'ge', label: '大于等于' },
  { value: 'eq', label: '等于' },
  { value: 'ne', label: '不等于' },
  { value: 'lt', label: '小于' },
  { value: 'le', label: '小于等于' },
  { value: 'like', label: '包含' },
  { value: 'notLike', label: '不包含' },
] as const

export type CompareOp = (typeof COMPARE_OPS)[number]['value']

export type SkipCondition =
  | { kind: 'none' }
  | { kind: 'compare'; op: CompareOp; variable: string; value: string }
  | { kind: 'spel'; expr: string }
  | { kind: 'raw'; raw: string }

const COMPARE_SET = new Set<string>(COMPARE_OPS.map((o) => o.value))

export function parseSkipCondition(
  s: string | null | undefined
): SkipCondition {
  const full = (s ?? '').trim()
  if (full === '') return { kind: 'none' }
  const at = full.indexOf('@@')
  if (at < 0) return { kind: 'raw', raw: full }
  const type = full.slice(0, at)
  const rest = full.slice(at + 2)
  if (type === 'spel') return { kind: 'spel', expr: rest }
  if (COMPARE_SET.has(type)) {
    const bar = rest.indexOf('|')
    if (bar < 0) return { kind: 'raw', raw: full }
    return {
      kind: 'compare',
      op: type as CompareOp,
      variable: rest.slice(0, bar),
      value: rest.slice(bar + 1),
    }
  }
  return { kind: 'raw', raw: full }
}

/** 写回 skipCondition;none → null(后端字段可空)。 */
export function formatSkipCondition(c: SkipCondition): string | null {
  switch (c.kind) {
    case 'none':
      return null
    case 'compare':
      return `${c.op}@@${c.variable.trim()}|${c.value.trim()}`
    case 'spel':
      return `spel@@${c.expr.trim()}`
    case 'raw':
      return c.raw
  }
}

/** 返回错误文案,合法返回 null。 */
export function validateSkipCondition(c: SkipCondition): string | null {
  switch (c.kind) {
    case 'none':
      return null
    case 'compare':
      if (c.variable.trim() === '') return '请输入条件变量名'
      if (c.value.trim() === '') return '请输入比较值'
      return null
    case 'spel': {
      const e = c.expr.trim()
      if (e === '') return '请输入 spel 表达式'
      // 引擎用 rhai 直接求值,Java 风格的 #{...} 包裹会解析失败(condition.rs eval_spel)
      if (/^#\{[\s\S]*\}$/.test(e))
        return '不要用 #{ } 包裹,直接写 #leaveDays > 2 这样的表达式'
      return null
    }
    case 'raw':
      return '当前引擎不支持该条件写法(default/snel),请改用比较条件或 spel'
  }
}
