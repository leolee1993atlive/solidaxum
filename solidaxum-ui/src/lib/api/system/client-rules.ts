/**
 * 客户端白名单规则串的切分/合并。
 *
 * 两端分隔符不一致:
 *   Vue getRuleList 切 /[\n,;]+/(换行/逗号/分号),字段提示与 placeholder 都按换行示例;
 *   Rust split_comma_list 只切 ','(client.rs:123-130)。
 * 于是:录入面按 Vue 的三分隔符解析,提交前统一 join 成逗号串,
 * 后端才切得开、accessPathList/ipWhitelistList 才正确。
 */

const SEPARATORS = /[\n,;]+/

/** 文本 → 规则数组。空输入返回空数组(语义:放行全部)。 */
export function parseRuleText(text?: string | null): string[] {
  if (!text) return []
  return text
    .split(SEPARATORS)
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
}

/** 规则数组 → 提交用的逗号串。空数组返回空串(保持字段存在,不发 undefined)。 */
export function toRuleText(rules: string[]): string {
  return rules.join(',')
}

/**
 * 列表回显用:优先取后端已切好的数组,没有才回退切原串。
 * 对齐 Vue getRuleList(index.vue:360-368)。
 */
export function resolveRuleList(list?: string[] | null, raw?: string | null): string[] {
  if (Array.isArray(list) && list.length > 0) return list
  return parseRuleText(raw)
}
