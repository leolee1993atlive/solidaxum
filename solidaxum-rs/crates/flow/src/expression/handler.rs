use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// 求值办理人表达式 ${var}
/// 返回 None 表示不是 ${} 表达式，调用方应原样保留
pub fn eval_handler(
    expr: &str,
    variable: &HashMap<String, Value>,
) -> Option<Vec<String>> {
    let expr = expr.trim();
    if !expr.starts_with("${") || !expr.ends_with('}') {
        return None;
    }
    let var_name = &expr[2..expr.len() - 1];
    let val = variable.get(var_name)?;
    match val {
        // 逗号分隔字符串按多人拆分(对齐 Java StringUtils.str2List:
        // trim + 去空),如 coorUserIds="1001,1002"
        Value::String(s) => Some(
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect(),
        ),
        Value::Array(arr) => Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
        ),
        _ => None,
    }
}

/// 段内 ${var} 替换：用于 `deptrole:${evDept}:121` 这类设计符内嵌变量。
/// 只替换已存在的变量（数字/字符串）；缺失变量原样保留。
fn substitute_vars(segment: &str, variable: &HashMap<String, Value>) -> String {
    if !segment.contains("${") {
        return segment.to_string();
    }
    let re = regex::Regex::new(r"\$\{(\w+)\}").expect("variable pattern");
    re.replace_all(segment, |caps: &regex::Captures| {
        match variable.get(&caps[1]) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => caps[0].to_string(),
        }
    })
    .to_string()
}

/// 办理人值规范化：字符串按逗号拆分(trim + 去空),数组逐项取
/// 字符串/数字(对齐 Java StringUtils.str2List)。
pub fn value_to_id_list(val: &Value) -> Vec<String> {
    match val {
        Value::String(s) => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect(),
        Value::Number(n) => vec![n.to_string()],
        _ => vec![],
    }
}

/// 读取弹窗选人变量 `{status}:{node_code}`(对齐 Java
/// WorkflowGlobalListener.processTaskPermission),status 为 pass/back。
pub fn status_assignee(
    variables: &HashMap<String, Value>,
    node_code: &str,
    status: &str,
) -> Option<Vec<String>> {
    let val = variables.get(&format!("{status}:{node_code}"))?;
    let ids = value_to_id_list(val);
    if ids.is_empty() { None } else { Some(ids) }
}

/// 消费弹窗选人变量:命中返回用户ID列表;pass 消费一次即删
/// (Java:remove + removeVariables),back 保留供后续重复退回。
pub fn take_status_assignee(
    variables: &mut HashMap<String, Value>,
    node_code: &str,
    status: &str,
) -> Option<Vec<String>> {
    let ids = status_assignee(variables, node_code, status)?;
    if status == "pass" {
        variables.remove(&format!("{status}:{node_code}"));
    }
    Some(ids)
}

/// 解析 permission_flag（按 @@ 拆分），求值每一项，去重（保留插入顺序）
///
/// 每项支持四种形态：
/// - `orElse(primary|fallback)`：主表达式解析为空时取兜底（对齐参考实现
///   Flow.orElse；与 @@ 并集不同，主办理人算得出时不并入兜底人）
/// - 整段 `${var}`：取变量值（字符串 → 逗号拆分，数组 → 多个）；
///   变量缺失/为空则**丢弃**（对齐 Java evalVariable 的 nonNull 过滤）
/// - 内嵌 `${var}` 的设计符：`deptrole:${evDept}:121` → `deptrole:101:121`
/// - 字面量：原样保留
pub fn resolve_handlers(
    permission_flag: &str,
    variable: &HashMap<String, Value>,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for part in permission_flag.split("@@") {
        for c in resolve_segment(part, variable) {
            if seen.insert(c.clone()) {
                result.push(c);
            }
        }
    }
    result
}

/// 解析单个 @@ 分段为办理人列表（空 Vec = 解析不出,丢弃）
fn resolve_segment(
    segment: &str,
    variable: &HashMap<String, Value>,
) -> Vec<String> {
    let part = segment.trim();
    // orElse(primary|fallback):主为空取兜底,支持嵌套
    if let Some(inner) = part
        .strip_prefix("orElse(")
        .and_then(|s| s.strip_suffix(')'))
        && let Some((primary, fallback)) = inner.split_once('|')
    {
        let r = resolve_segment(primary, variable);
        return if r.is_empty() {
            resolve_segment(fallback, variable)
        } else {
            r
        };
    }
    if let Some(resolved) = eval_handler(part, variable) {
        return resolved;
    }
    // 整段 ${var} 但变量缺失 → 空(Java:DefaultHandlerStrategy 返回 null
    // 被 nonNull 过滤)
    if part.starts_with("${") && part.ends_with('}') {
        return vec![];
    }
    vec![substitute_vars(part, variable)]
}

/// 处理 next_handler 覆盖/追加
pub fn apply_next_handler(
    resolved: Vec<String>,
    next_handler: &Option<Vec<String>>,
    append: bool,
) -> Vec<String> {
    if let Some(next) = next_handler {
        if next.is_empty() {
            return resolved;
        }
        let mut result = if append { resolved } else { Vec::new() };
        let mut seen: HashSet<String> = result.iter().cloned().collect();
        for h in next {
            if seen.insert(h.clone()) {
                result.push(h.clone());
            }
        }
        result
    } else {
        resolved
    }
}

/// 实例级设计符前缀(冒号结尾天然与 preStep: 区分,无需考虑匹配顺序)
const INSTANCE_PREFIXES: [&str; 4] =
    ["sameStep:", "preStepApper:", "preStepParent:", "preStep:"];

/// 将解析后的办理人表达式展开为实际用户 ID
/// (含 role:/dept:/user:/post:/deptrole:/deptrolekey: 设计符解析与
/// sameStep:/preStep:/preStepApper:/preStepParent:/initiator/initiatorParent
/// 实例级设计符解析;后者需要 inst_ctx,缺失时丢弃并 warn)
pub async fn expand_handlers(
    resolver: &dyn crate::resolver::UserResolver,
    handlers: Vec<String>,
    inst_ctx: Option<&crate::resolver::InstanceAssigneeContext>,
) -> Vec<String> {
    let mut plain = Vec::new();
    let mut designators = Vec::new();
    let mut instance_designators = Vec::new();
    for h in handlers {
        if h.starts_with("role:")
            || h.starts_with("dept:")
            || h.starts_with("user:")
            || h.starts_with("post:")
            || h.starts_with("deptrole:")
            || h.starts_with("deptrolekey:")
        {
            designators.push(h);
        } else if INSTANCE_PREFIXES.iter().any(|p| h.starts_with(p))
            || h == "initiator"
            || h == "initiatorParent"
        {
            match inst_ctx {
                Some(_) => instance_designators.push(h),
                None => tracing::warn!(
                    handler = %h,
                    "实例级办理人设计符缺少实例上下文,已丢弃"
                ),
            }
        } else {
            plain.push(h);
        }
    }
    if !designators.is_empty() {
        for id in resolver.resolve_designators(&designators).await {
            plain.push(id.to_string());
        }
    }
    if !instance_designators.is_empty()
        && let Some(ctx) = inst_ctx
    {
        for id in resolver
            .resolve_instance_designators(&instance_designators, ctx)
            .await
        {
            plain.push(id.to_string());
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::resolver::UserResolver;

    fn vars(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn resolve_handlers_substitutes_var_inside_designator() {
        // 对齐参考实现 `#{@flow.deptRole(#evDept, 121L)}` → deptrole:${evDept}:121
        let v = vars(&[("evDept", serde_json::json!(101))]);
        assert_eq!(
            resolve_handlers("deptrole:${evDept}:121", &v),
            vec!["deptrole:101:121".to_string()]
        );
    }

    #[test]
    fn resolve_handlers_whole_var_still_expands_list() {
        // 整段 ${var} 语义不变:数组展开为多个办理人
        let v = vars(&[("handlers", serde_json::json!(["7", "8"]))]);
        assert_eq!(
            resolve_handlers("${handlers}", &v),
            vec!["7".to_string(), "8".to_string()]
        );
    }

    #[test]
    fn resolve_handlers_comma_string_splits() {
        // 逗号分隔字符串按多人处理(对齐 Java StringUtils.str2List,
        //参考实现 coorUserIds="id1,id2" 场景)
        let v = vars(&[("coorUserIds", serde_json::json!("1001, 1002"))]);
        assert_eq!(
            resolve_handlers("${coorUserIds}", &v),
            vec!["1001".to_string(), "1002".to_string()]
        );
        // 单值无逗号不受影响
        let v = vars(&[("leader", serde_json::json!("7"))]);
        assert_eq!(resolve_handlers("${leader}", &v), vec!["7".to_string()]);
    }

    #[test]
    fn resolve_handlers_mixed_segments_and_missing_var() {
        let v = vars(&[("evDept", serde_json::json!(101))]);
        // @@ 分段:字面量设计符原样、内联变量替换;缺失变量的整段 ${}
        // 丢弃(对齐 Java evalVariable 的 nonNull 过滤)
        assert_eq!(
            resolve_handlers("role:114@@deptrole:${evDept}:121@@${leader}", &v),
            vec!["role:114".to_string(), "deptrole:101:121".to_string(),]
        );
    }

    /// 记录收到的 designators / 实例级设计符调用的 mock resolver
    #[derive(Default)]
    struct RecordingResolver {
        calls: Mutex<Vec<String>>,
        /// (designators, instance_id, node_code)
        instance_calls: Mutex<Vec<(Vec<String>, i64, String)>>,
    }

    #[async_trait::async_trait]
    impl UserResolver for RecordingResolver {
        async fn user_names(&self, _ids: &[i64]) -> HashMap<i64, String> {
            HashMap::new()
        }

        async fn resolve_designators(
            &self,
            designators: &[String],
        ) -> Vec<i64> {
            self.calls
                .lock()
                .unwrap()
                .extend(designators.iter().cloned());
            vec![42]
        }

        async fn resolve_instance_designators(
            &self,
            designators: &[String],
            ctx: &crate::resolver::InstanceAssigneeContext,
        ) -> Vec<i64> {
            self.instance_calls.lock().unwrap().push((
                designators.to_vec(),
                ctx.instance_id,
                ctx.target_node_code.clone(),
            ));
            vec![43]
        }
    }

    #[tokio::test]
    async fn expand_handlers_routes_deptrole_to_resolver() {
        let rec = RecordingResolver::default();
        let out = expand_handlers(
            &rec,
            vec!["deptrole:101:121".to_string(), "7".to_string()],
            None,
        )
        .await;
        assert_eq!(
            rec.calls.lock().unwrap().clone(),
            vec!["deptrole:101:121".to_string()],
            "deptrole: 设计符应交给 resolver 展开"
        );
        assert!(out.contains(&"42".to_string()));
        assert!(out.contains(&"7".to_string()), "裸用户 ID 应原样保留");
    }

    #[tokio::test]
    async fn expand_handlers_routes_deptrolekey_to_resolver() {
        let rec = RecordingResolver::default();
        let out = expand_handlers(
            &rec,
            vec!["deptrolekey:101:deptLeader".to_string(), "7".to_string()],
            None,
        )
        .await;
        assert_eq!(
            rec.calls.lock().unwrap().clone(),
            vec!["deptrolekey:101:deptLeader".to_string()],
            "deptrolekey: 设计符应交给 resolver 展开(此前被当裸字符串透传)"
        );
        assert!(out.contains(&"7".to_string()), "裸用户 ID 应原样保留");
    }

    #[tokio::test]
    async fn expand_handlers_routes_instance_scoped_designators() {
        let rec = RecordingResolver::default();
        let ctx = crate::resolver::InstanceAssigneeContext {
            instance_id: 9001,
            target_node_code: "ae_ana".to_string(),
            variable: HashMap::new(),
            current: None,
        };
        let _ = expand_handlers(
            &rec,
            vec![
                "preStep:ae_area_audit".to_string(),
                "sameStep:ae_dept_audit".to_string(),
                "preStepApper:ae_area_audit".to_string(),
                "preStepParent:ae_dept_audit".to_string(),
                "initiator".to_string(),
                "initiatorParent".to_string(),
                "7".to_string(),
            ],
            Some(&ctx),
        )
        .await;
        let calls = rec.instance_calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1, "实例级设计符应聚合成一次 resolver 调用");
        assert_eq!(calls[0].1, 9001);
        assert_eq!(calls[0].2, "ae_ana");
        assert_eq!(calls[0].0.len(), 6, "6 种形态全部路由进 resolver");
    }

    #[tokio::test]
    async fn expand_handlers_drops_instance_designators_without_ctx() {
        let rec = RecordingResolver::default();
        let out =
            expand_handlers(&rec, vec!["preStep:x".to_string()], None).await;
        assert!(
            out.is_empty(),
            "无实例上下文时实例级设计符应丢弃(warn 日志)"
        );
        assert!(rec.instance_calls.lock().unwrap().is_empty());
    }

    // ─── orElse(primary, fallback):主办理人为空时兜底 ───
    // 对齐参考实现 Flow.orElse:与 @@ 并集语义不同,主办理人算得出时
    // 不并入兜底人

    #[test]
    fn or_else_uses_primary_when_resolvable() {
        let v = vars(&[("deptLeader", serde_json::json!("u9"))]);
        assert_eq!(
            resolve_handlers("orElse(${deptLeader}|role:114)", &v),
            vec!["u9".to_string()],
            "主表达式有值时不并入兜底"
        );
    }

    #[test]
    fn or_else_falls_back_when_primary_empty() {
        let v = vars(&[]);
        // ${deptLeader} 变量缺失 → 兜底 role:114
        assert_eq!(
            resolve_handlers("orElse(${deptLeader}|role:114)", &v),
            vec!["role:114".to_string()]
        );
        // 变量存在但为空串 → 同样兜底
        let v2 = vars(&[("deptLeader", serde_json::json!(""))]);
        assert_eq!(
            resolve_handlers("orElse(${deptLeader}|role:114)", &v2),
            vec!["role:114".to_string()]
        );
    }
}
