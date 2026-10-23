/**
 * 审批节点 nodeRatio 的编解码。
 *
 * 引擎侧语义(crates/flow/src/engine/vote.rs、enums.rs CooperateType):
 *   "0" / 空  → 或签;"100" → 会签;0<r<100 → 票签通过率;
 *   "passCount=N" / "rejectCount=N" → 固定票数;可带 "@@sequence" 后缀表示依次审批。
 * Java 设计器还会写 default@@/spel@@/snel@@ 形式的策略表达式,当前引擎不认,
 * 这里按 raw 原样保留、不提供编辑。
 */

/** 协作方式:1 或签 / 2 票签 / 3 会签 */
export type CollaborativeWay = '1' | '2' | '3'

export type RatioType = 'passRatio' | 'passCount' | 'rejectCount' | 'raw'

export interface NodeRatio {
  way: CollaborativeWay
  /** 仅 way === '2' 有意义 */
  type: RatioType
  /** passRatio 的百分数 / passCount、rejectCount 的人数 / raw 的整串 */
  value: string
  sequence: boolean
}

const SEQ = '@@sequence'

export function parseNodeRatio(s: string | null | undefined): NodeRatio {
  const full = (s ?? '').trim()
  const sequence = full.endsWith(SEQ)
  const body = sequence ? full.slice(0, -SEQ.length) : full
  if (body === '') return { way: '1', type: 'passRatio', value: '', sequence }
  const fixed = /^(passCount|rejectCount)=(.*)$/.exec(body)
  if (fixed) {
    return { way: '2', type: fixed[1] as RatioType, value: fixed[2], sequence }
  }
  if (/^\d+(\.\d+)?$/.test(body)) {
    // 导入件里或签写成 "0.000":数值归到或签/会签,与 LogicFlow 版 parseFloat 判定一致;
    // 引擎对 "0.000" 与 "0" 的拒绝路径语义略异(vote.rs 里 0.000 落入百分比分支),
    // 但归一化只在用户改动该字段时发生,与 Java 设计器的保存行为相同
    const n = Number(body)
    if (n === 0) return { way: '1', type: 'passRatio', value: '', sequence }
    if (n === 100) return { way: '3', type: 'passRatio', value: '', sequence }
    return { way: '2', type: 'passRatio', value: body, sequence }
  }
  return { way: '2', type: 'raw', value: body, sequence }
}

export function formatNodeRatio(r: NodeRatio): string {
  let body: string
  if (r.way === '1') body = '0'
  else if (r.way === '3') body = '100'
  else if (r.type === 'passCount' || r.type === 'rejectCount') {
    body = `${r.type}=${r.value.trim()}`
  } else body = r.value.trim()
  return r.sequence ? `${body}${SEQ}` : body
}

/** 切换协作方式时的默认值:票签默认 50%,与 LogicFlow 版一致。 */
export function ratioForWay(way: CollaborativeWay): NodeRatio {
  return {
    way,
    type: 'passRatio',
    value: way === '2' ? '50' : '',
    sequence: false,
  }
}

/** 票签策略的取值校验;返回错误文案,合法返回 null。规则对齐 between.vue 的 validators。 */
export function validateNodeRatio(r: NodeRatio): string | null {
  if (r.way !== '2') return null
  const v = r.value.trim()
  switch (r.type) {
    case 'passRatio': {
      if (v === '') return '请输入通过率'
      const n = Number(v)
      if (Number.isNaN(n)) return '请输入有效数字'
      if (n < 0.001 || n > 100) return '通过率必须在 0.001–100 之间'
      if (!/^\d+(\.\d{1,3})?$/.test(v)) return '通过率最多保留三位小数'
      return null
    }
    case 'passCount':
    case 'rejectCount':
      return /^[1-9]\d*$/.test(v) ? null : '请输入正整数'
    case 'raw':
      return v === '' ? '策略不能为空' : null
  }
}

export const RATIO_TYPE_OPTIONS: {
  value: RatioType
  label: string
  placeholder: string
}[] = [
  {
    value: 'passRatio',
    label: '通过率',
    placeholder: '请输入通过率(0.001–100)',
  },
  {
    value: 'passCount',
    label: '固定通过人数',
    placeholder: '请输入固定通过人数,正整数',
  },
  {
    value: 'rejectCount',
    label: '固定驳回人数',
    placeholder: '请输入固定驳回人数,正整数',
  },
]
