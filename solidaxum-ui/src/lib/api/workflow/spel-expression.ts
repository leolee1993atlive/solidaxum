/**
 * SPEL 表达式实时预览 —— 逐字移植 Vue 的 updateViewSpel
 * (RuoYi-Vue-Plus-Ui/src/views/workflow/spel/index.vue:340-381)。
 *
 * 抽成纯函数的理由:四条分支各有各的输出形状(空串 / ${x} / 提示文案 / #{@c.m(#p)}),
 * 写错了不会报错 —— 只是存下一个永远不会执行的表达式,而流程跑到该节点才炸。
 */

/** 「组件或方法没填全」时 Vue 直接把这句话塞进 viewSpel 字段(不是 placeholder)。 */
export const SPEL_INCOMPLETE_HINT = '请填写组件名称和方法名'

export interface SpelParts {
  componentName?: string | null
  methodName?: string | null
  methodParams?: string | null
}

/** 按 `,` 切分、逐个 trim、丢弃空串。 */
function splitParams(raw: string): string[] {
  return raw
    .split(',')
    .map((p) => p.trim())
    .filter((p) => p.length > 0)
}

export function buildViewSpel({ componentName, methodName, methodParams }: SpelParts): string {
  const comp = (componentName ?? '').trim()
  const method = (methodName ?? '').trim()
  const paramStr = (methodParams ?? '').trim()

  // 分支 1:什么都没填
  if (!comp && !method && !paramStr) return ''

  // 分支 2:只填了参数,且恰好一个 → 变量取值形式(注意是 $ 不是 #)
  if (!comp && !method && paramStr) {
    const list = splitParams(paramStr)
    if (list.length === 1) return `\${${list[0]}}`
  }

  // 分支 3:组件或方法缺一 → 提示文案入值
  if (!comp || !method) return SPEL_INCOMPLETE_HINT

  // 分支 4:方法调用形式;无参时括号仍在
  const list = paramStr ? splitParams(paramStr) : []
  const paramPart = list.length > 0 ? `(${list.map((p) => `#${p}`).join(',')})` : '()'
  return `#{@${comp}.${method}${paramPart}}`
}

/**
 * 是否可提交。后端 `SpelAddBo.view_spel` 是 String 且带
 * `#[validate(length(min = 1))]`(dto.rs:823-824),空串必被拒;
 * 提示文案虽非空但不是表达式,同样不该落库。
 */
export function isSubmittableSpel(viewSpel: string): boolean {
  return viewSpel.length > 0 && viewSpel !== SPEL_INCOMPLETE_HINT
}
