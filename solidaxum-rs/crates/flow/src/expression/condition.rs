use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use app_core::error::{AppError, AppResult};
use regex::Regex;
use rhai::{Dynamic, Engine, EvalAltResult, Scope};
use serde_json::Value;

use crate::{model::skip::Skip, resolver::UserResolver};

/// 条件归一化用常量正则——模式为编译期字面量,编译一次全局复用,
/// 避免每次网关求值重复编译。
static VARIADIC_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)(hasRole|hasRoleKeys)\(([^()]*)\)")
        .expect("常量正则模式必合法")
});
static STRIP_HASH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(^|[^"])#(\w+)"#).expect("常量正则模式必合法")
});
static STRIP_LONG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(^|[^\w"'])([0-9]+)L\b"#).expect("常量正则模式必合法")
});

/// SpEL 求值上下文：进入网关展开前由 async 调用层预热的角色快照。
///
/// rhai 是同步引擎，DB 查询必须在求值前完成 —— 快照预取的是**输入**
/// （用户当前的角色集合），答案仍由表达式现场计算；角色新增/授权变更后
/// 下次过网关自然生效，流程定义无需修改（对齐 Java `@flow.hasRole` 的
/// selectRolesByUserId 现查语义）。
#[derive(Debug, Clone, Default)]
pub struct SpelContext {
    /// user_id → 该用户拥有的 role_id 列表
    pub user_roles: Arc<HashMap<i64, Vec<i64>>>,
    /// user_id → 该用户拥有的 role_key 列表(hasRoleKeys 用)
    pub user_role_keys: Arc<HashMap<i64, Vec<String>>>,
}

impl SpelContext {
    pub fn new(user_roles: HashMap<i64, Vec<i64>>) -> Self {
        Self {
            user_roles: Arc::new(user_roles),
            ..Default::default()
        }
    }
}

/// 求值条件表达式（无角色快照；含 hasRole 的条件会因「未预热」报错）。
pub fn eval_condition(
    expr: &str,
    variable: &HashMap<String, Value>,
) -> AppResult<bool> {
    eval_condition_with(expr, variable, &SpelContext::default())
}

/// 从跳转条件中扫描 `hasRole(<首参>, ...)` / `hasRoleKeys(<首参>, ...)` 的
/// 用户（变量名或字面量 ID），经 resolver 现查其角色集合，构建求值上下文。
///
/// 在 async 调用层、进入网关展开（`get_next_by_check_gateway`）前调用。
/// 条件里没有这两个函数时不触发任何查询。
pub async fn prefetch_spel_context(
    resolver: &dyn UserResolver,
    skips: &[Skip],
    variable: &HashMap<String, Value>,
) -> SpelContext {
    let id_users = collect_fn_first_args(skips, variable, "hasRole");
    let key_users = collect_fn_first_args(skips, variable, "hasRoleKeys");
    let mut ctx = SpelContext::default();
    if !id_users.is_empty() {
        let mut roles = resolver.user_role_ids(&id_users).await;
        // 无角色的用户也占位：预热为空 → 判 false 而非报「未预热」
        for id in &id_users {
            roles.entry(*id).or_default();
        }
        ctx.user_roles = Arc::new(roles);
    }
    if !key_users.is_empty() {
        let mut keys = resolver.user_role_keys(&key_users).await;
        for id in &key_users {
            keys.entry(*id).or_default();
        }
        ctx.user_role_keys = Arc::new(keys);
    }
    ctx
}

/// 收集指定函数(hasRole/hasRoleKeys)首参引用的用户 ID:
/// 字面量直接取,变量名从 variable 解析(数字或数字字符串)
fn collect_fn_first_args(
    skips: &[Skip],
    variable: &HashMap<String, Value>,
    fn_name: &str,
) -> Vec<i64> {
    let re = Regex::new(&format!(r"{fn_name}\(\s*#?(\w+)"))
        .expect("fn_name 为调用方常量,拼接出的正则必合法");
    let mut user_ids: Vec<i64> = Vec::new();
    for skip in skips {
        let Some(cond) = skip.skip_condition.as_deref() else {
            continue;
        };
        if !cond.contains(fn_name) {
            continue;
        }
        for cap in re.captures_iter(cond) {
            let token = &cap[1];
            let id = token.parse::<i64>().ok().or_else(|| {
                match variable.get(token) {
                    Some(Value::Number(n)) => n.as_i64(),
                    Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
                    _ => None,
                }
            });
            if let Some(id) = id
                && !user_ids.contains(&id)
            {
                user_ids.push(id);
            }
        }
    }
    user_ids
}

/// 求值条件表达式，如 "eq@@amount|1000"
///
/// 支持的 SpEL 子集（spel@@ 前缀）：
/// - 数值 / 字符串 / 布尔比较：>、>=、<、<=、==、!=
/// - 逻辑组合：&&、||、括号
/// - 变量引用：amount 或 #amount（Spring SpEL 风格的前导 # 会被去掉）
/// - 字面量：数字（兼容 Java Long 后缀，116L 按 116 处理）、字符串、true/false
/// - 函数：hasRole(userId, roleId) / hasRole(userId, [roleId, ...])，
///   任一角色命中即 true（OR），角色数据来自 SpelContext 预热快照
///
/// 不支持：T() 类型引用、任意方法调用、集合操作等完整 Spring SpEL 特性。
///
/// 空条件视为 true；变量不存在或不支持的语法返回 Err（对齐 Java FlowException），
/// 由网关展开层向上传播，避免分支被静默吞掉。
pub fn eval_condition_with(
    expr: &str,
    variable: &HashMap<String, Value>,
    spel: &SpelContext,
) -> AppResult<bool> {
    if expr.is_empty() {
        return Ok(true);
    }

    // spel@@ 前缀：用 rhai 引擎求值
    if let Some(script) = expr.strip_prefix("spel@@") {
        return eval_spel(script.trim(), variable, spel);
    }

    //参考实现默认分支边(v53 f41db736 网关原样数据):
    // `default@@${spel}` —— 条件照常求值,与其余 eq/ne 边的互斥由数据
    // 保证(aeCategory 各分支值域不相交),求值 true 即走该边。
    if let Some(rest) = expr.strip_prefix("default@@") {
        return eval_spel(rest.trim(), variable, spel);
    }

    if let Some((prefix, rest)) = expr.split_once("@@")
        && let Some((var_name, cmp_value)) = rest.split_once('|')
    {
        let var_val = variable.get(var_name.trim()).ok_or_else(|| {
            AppError::Internal(format!("条件变量不存在: {}", var_name.trim()))
        })?;
        let var_str = match var_val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => var_val.to_string(),
        };

        return Ok(match prefix {
            "eq" => compare_eq(&var_str, cmp_value.trim()),
            "ne" => !compare_eq(&var_str, cmp_value.trim()),
            "gt" => compare_gt(&var_str, cmp_value.trim()),
            "ge" => compare_ge(&var_str, cmp_value.trim()),
            "lt" => compare_lt(&var_str, cmp_value.trim()),
            "le" => compare_le(&var_str, cmp_value.trim()),
            "like" => var_str.contains(cmp_value.trim()),
            "notLike" => !var_str.contains(cmp_value.trim()),
            _ => {
                return Err(AppError::Internal(format!(
                    "不支持的条件算子: {prefix}"
                )));
            }
        });
    }
    Err(AppError::Internal(format!("无法解析的条件表达式: {expr}")))
}

fn is_numeric(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

fn as_f64(s: &str) -> Option<f64> {
    s.parse::<f64>().ok()
}

fn compare_eq(var: &str, val: &str) -> bool {
    if is_numeric(val) && is_numeric(var) {
        as_f64(var) == as_f64(val)
    } else {
        var == val
    }
}

fn compare_gt(var: &str, val: &str) -> bool {
    if is_numeric(val) && is_numeric(var) {
        as_f64(var).zip(as_f64(val)).is_some_and(|(a, b)| a > b)
    } else {
        false
    }
}

fn compare_ge(var: &str, val: &str) -> bool {
    if is_numeric(val) && is_numeric(var) {
        as_f64(var).zip(as_f64(val)).is_some_and(|(a, b)| a >= b)
    } else {
        false
    }
}

fn compare_lt(var: &str, val: &str) -> bool {
    if is_numeric(val) && is_numeric(var) {
        as_f64(var).zip(as_f64(val)).is_some_and(|(a, b)| a < b)
    } else {
        false
    }
}

fn compare_le(var: &str, val: &str) -> bool {
    if is_numeric(val) && is_numeric(var) {
        as_f64(var).zip(as_f64(val)).is_some_and(|(a, b)| a <= b)
    } else {
        false
    }
}

/// rhai Dynamic → i64（变量注入为 FLOAT、字面量为 INT，两者都收）
fn dyn_i64(d: &Dynamic) -> Option<i64> {
    if let Ok(i) = d.as_int() {
        Some(i)
    } else if let Ok(f) = d.as_float() {
        Some(f as i64)
    } else if let Ok(s) = d.clone().into_string() {
        // 变量以数字字符串注入(如服务端注入 initiator="9101"),Java SpEL
        // 会自动强转,rhai 不转 —— 这里兼容
        s.trim().parse::<i64>().ok()
    } else {
        None
    }
}

/// hasRole 次参：单个角色 ID 或角色 ID 数组
fn dyn_role_ids(d: &Dynamic) -> Option<Vec<i64>> {
    if d.is_array() {
        let arr = d.clone().into_array().ok()?;
        arr.iter().map(dyn_i64).collect()
    } else {
        dyn_i64(d).map(|i| vec![i])
    }
}

/// hasRoleKeys 次参：单个 roleKey 或 roleKey 数组
fn dyn_str_list(d: &Dynamic) -> Option<Vec<String>> {
    if d.is_array() {
        let arr = d.clone().into_array().ok()?;
        arr.iter().map(|v| v.clone().into_string().ok()).collect()
    } else {
        d.clone().into_string().ok().map(|s| vec![s])
    }
}

/// 用 rhai 引擎求值 spel@@ 表达式
/// 将 variable 中的值注入 rhai Scope，返回 bool 结果；求值失败返回 Err
fn eval_spel(
    expr: &str,
    variable: &HashMap<String, Value>,
    spel: &SpelContext,
) -> AppResult<bool> {
    // Spring SpEL 模板包装剥离(v53 原样数据两种形态):
    // `#{@flow.hasRole(#initiator, 102L)}` / `${aeCategory == '1'}`
    // → 内层布尔表达式。
    let expr = expr.trim();
    let expr = if (expr.starts_with("#{") && expr.ends_with('}'))
        || (expr.starts_with("${") && expr.ends_with('}'))
    {
        expr[2..expr.len() - 1].trim()
    } else {
        expr
    };
    // Java SpEL 的 `@flow.` 接收者(rhai 无此语法,函数直接注册)
    let expr = expr.replace("@flow.", "");
    // Java SpEL 单引号字符串字面量 → 双引号(rhai 单引号是 char 字面量,
    // 与字符串比较类型不合);v53 条件均为简单比较,无含引号字符串
    let expr = expr.replace('\'', "\"");
    // 变参调用归一化:v53 原样数据 `hasRole(#initiator, 102L, 103L, ...)`
    // -> `hasRole(initiator, [102, 103, ...])`(注册的函数是二参形态,
    // 次参为数组)。hasRoleKeys 同形。
    let expr = VARIADIC_CALL_RE
        .replace_all(&expr, |caps: &regex::Captures| {
            let Some((first, rest)) = caps[2].split_once(',') else {
                return caps[0].to_string();
            };
            let rest = rest.trim();
            // 已是数组形态(hasRole(u, [a, b]))原样保留
            if rest.starts_with('[') {
                return caps[0].to_string();
            }
            // 变参标量形态(hasRole(u, a, b, c)) → 数组
            format!("{}({}, [{}])", &caps[1], first.trim(), rest)
        })
        .to_string();
    // 支持 Spring SpEL 的 #var 写法：仅去掉变量引用前导 '#'，
    // 但保留字符串字面量中的 '#'（如 "#urgent"）——
    // 只在 '#' 前面是 行首/空格/运算符 等非引号字符时才视为变量前缀。
    let expr = STRIP_HASH_RE.replace_all(&expr, "$1$2").to_string();
    // Java Long 字面量后缀兼容：116L → 116（同样不触碰字符串字面量，
    // 便于直接迁移 Java 侧既有 skip_condition）
    let expr = STRIP_LONG_RE.replace_all(&expr, "$1$2").to_string();
    let mut engine = Engine::new();

    // hasRole(userId, roleId | [roleId, ...])：任一命中即 true（OR），
    // 对齐参考实现 `@flow.hasRole`（CollUtil.containsAny）。
    // 用户不在预热快照 → Err：未预热是调用层配置错误，不允许静默走错分支。
    let snapshot = spel.user_roles.clone();
    engine.register_fn(
        "hasRole",
        move |user: Dynamic,
              roles: Dynamic|
              -> Result<bool, Box<EvalAltResult>> {
            let uid = dyn_i64(&user).ok_or_else(|| -> Box<EvalAltResult> {
                format!("hasRole 首参必须是用户ID,实际: {user}").into()
            })?;
            let owned = snapshot.get(&uid).ok_or_else(|| -> Box<EvalAltResult> {
                format!(
                    "hasRole 用户 {uid} 的角色未预热(调用层需先 prefetch_spel_context)"
                )
                .into()
            })?;
            let want = dyn_role_ids(&roles).ok_or_else(
                || -> Box<EvalAltResult> {
                    format!("hasRole 次参必须是角色ID或数组,实际: {roles}")
                        .into()
                },
            )?;
            Ok(want.iter().any(|r| owned.contains(r)))
        },
    );

    // hasRoleKeys(userId, "key" | ["key", ...]):任一命中即 true,
    // 对齐参考实现 `@flow.hasRoleKeys`(roleKey 跨环境通用)。
    let keys_snapshot = spel.user_role_keys.clone();
    engine.register_fn(
        "hasRoleKeys",
        move |user: Dynamic,
              keys: Dynamic|
              -> Result<bool, Box<EvalAltResult>> {
            let uid = dyn_i64(&user).ok_or_else(|| -> Box<EvalAltResult> {
                format!("hasRoleKeys 首参必须是用户ID,实际: {user}").into()
            })?;
            let owned =
                keys_snapshot.get(&uid).ok_or_else(|| -> Box<EvalAltResult> {
                    format!(
                        "hasRoleKeys 用户 {uid} 的角色未预热(调用层需先 prefetch_spel_context)"
                    )
                    .into()
                })?;
            let want = dyn_str_list(&keys).ok_or_else(
                || -> Box<EvalAltResult> {
                    format!("hasRoleKeys 次参必须是角色key或数组,实际: {keys}")
                        .into()
                },
            )?;
            Ok(want.iter().any(|k| owned.contains(k)))
        },
    );

    let mut scope = Scope::new();

    for (key, val) in variable {
        match val {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    scope.push(key.clone(), i as f64);
                } else if let Some(f) = n.as_f64() {
                    scope.push(key.clone(), f);
                }
            }
            Value::String(s) => {
                scope.push(key.clone(), s.clone());
            }
            Value::Bool(b) => {
                scope.push(key.clone(), *b);
            }
            _ => {}
        }
    }

    engine
        .eval_with_scope::<bool>(&mut scope, &expr)
        .map_err(|e| {
            AppError::Internal(format!("SpEL 表达式求值失败 [{expr}]: {e}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(key: &str, val: serde_json::Value) -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert(key.to_string(), val);
        m
    }

    #[test]
    fn eq_numeric() {
        assert!(
            eval_condition("eq@@amount|1000", &ctx("amount", 1000.into()))
                .unwrap()
        );
        assert!(
            !eval_condition("eq@@amount|1000", &ctx("amount", 999.into()))
                .unwrap()
        );
    }

    #[test]
    fn spel_simple_comparison() {
        assert!(
            eval_condition(
                "spel@@amount > 1000",
                &ctx("amount", serde_json::json!(1500))
            )
            .unwrap()
        );
        assert!(
            !eval_condition(
                "spel@@amount > 1000",
                &ctx("amount", serde_json::json!(500))
            )
            .unwrap()
        );
    }

    #[test]
    fn spel_string_equality() {
        assert!(
            eval_condition(
                "spel@@status == \"approved\"",
                &ctx("status", serde_json::json!("approved"))
            )
            .unwrap()
        );
    }

    #[test]
    fn spel_supports_hash_var_syntax() {
        // Spring SpEL 写法 #amount
        assert!(
            eval_condition(
                "spel@@#amount > 1000",
                &ctx("amount", serde_json::json!(1500))
            )
            .unwrap()
        );
    }

    #[test]
    fn spel_template_hash_flow_wrapper_unwrapped() {
        // v53 原样形态:spel@@#{@flow.hasRole(#initiator, 101L)}
        let vars = ctx("initiator", serde_json::json!(9104));
        let spel = SpelContext::new([(9104i64, vec![101i64])].into());
        assert!(
            eval_condition_with(
                "spel@@#{@flow.hasRole(#initiator, 101L)}",
                &vars,
                &spel
            )
            .unwrap()
        );
        assert!(
            !eval_condition_with(
                "spel@@#{@flow.hasRole(#initiator, 102L, 103L)}",
                &vars,
                &spel
            )
            .unwrap()
        );
    }

    #[test]
    fn default_prefix_edge_evaluates_spel_template() {
        // v53 f41db736 默认分支边:default@@${aeCategory == '6' || aeCategory == '12'}
        assert!(
            eval_condition(
                "default@@${aeCategory == '6' || aeCategory == '12'}",
                &ctx("aeCategory", serde_json::json!("6"))
            )
            .unwrap()
        );
        assert!(
            !eval_condition(
                "default@@${aeCategory == '6' || aeCategory == '12'}",
                &ctx("aeCategory", serde_json::json!("1"))
            )
            .unwrap()
        );
    }

    #[test]
    fn unknown_variable_is_error_not_silent() {
        // 未知变量应 Err（对齐 Java FlowException），而非静默 None/false
        assert!(eval_condition("spel@@unknown > 0", &HashMap::new()).is_err());
        assert!(eval_condition("gt@@missing|1", &HashMap::new()).is_err());
    }

    #[test]
    fn empty_condition_is_true() {
        assert!(eval_condition("", &HashMap::new()).unwrap());
    }

    #[test]
    fn spel_hash_in_string_literal_preserved() {
        // 字符串字面量中的 # 不应被当作变量前缀去掉
        let vars = ctx("status", serde_json::json!("#urgent"));
        assert!(
            eval_spel("status == \"#urgent\"", &vars, &SpelContext::default())
                .unwrap()
        );
    }

    #[test]
    fn op_strategy_still_works() {
        assert!(
            eval_condition("eq@@amount|1000", &ctx("amount", 1000.into()))
                .unwrap()
        );
        assert!(
            !eval_condition("eq@@amount|1000", &ctx("amount", 999.into()))
                .unwrap()
        );
    }

    // ─── hasRole:对齐参考实现 `spel@@#{@flow.hasRole(#initiator, 116L)}` ───

    fn spel_ctx(pairs: &[(i64, &[i64])]) -> SpelContext {
        let m: HashMap<i64, Vec<i64>> =
            pairs.iter().map(|(u, r)| (*u, r.to_vec())).collect();
        SpelContext::new(m)
    }

    #[test]
    fn spel_has_role_single_role() {
        let vars = ctx("initiator", serde_json::json!(7));
        let yes = spel_ctx(&[(7, &[116, 114])]);
        assert!(
            eval_condition_with("spel@@hasRole(#initiator, 116)", &vars, &yes)
                .unwrap()
        );
        assert!(
            !eval_condition_with("spel@@hasRole(#initiator, 121)", &vars, &yes)
                .unwrap()
        );
    }

    #[test]
    fn spel_has_role_array_any_match() {
        // 多角色语义 = 任一命中(对齐 Java CollUtil.containsAny)
        let vars = ctx("initiator", serde_json::json!(7));
        let c = spel_ctx(&[(7, &[115])]);
        assert!(
            eval_condition_with(
                "spel@@hasRole(#initiator, [121, 115])",
                &vars,
                &c
            )
            .unwrap()
        );
        assert!(
            !eval_condition_with(
                "spel@@hasRole(#initiator, [121, 114])",
                &vars,
                &c
            )
            .unwrap()
        );
    }

    #[test]
    fn spel_has_role_java_long_suffix() {
        // Java SpEL 字面量 116L 兼容(迁移既有流程定义免改写)
        let vars = ctx("initiator", serde_json::json!(7));
        let c = spel_ctx(&[(7, &[116])]);
        assert!(
            eval_condition_with("spel@@hasRole(#initiator, 116L)", &vars, &c)
                .unwrap()
        );
    }

    #[test]
    fn spel_has_role_unpreheated_user_errors() {
        let vars = ctx("initiator", serde_json::json!(7));
        // 用户不在预热快照 → Err(未预热是配置错误,不允许静默走错分支)
        assert!(
            eval_condition_with(
                "spel@@hasRole(#initiator, 116)",
                &vars,
                &SpelContext::default()
            )
            .is_err()
        );
        // 已预热但无角色 → false,不报错(对齐 Java 查不到角色列表)
        let c = spel_ctx(&[(7, &[])]);
        assert!(
            !eval_condition_with("spel@@hasRole(#initiator, 116)", &vars, &c)
                .unwrap()
        );
    }

    // ─── 预热:从跳转条件扫描 hasRole 首参,现查角色快照 ───

    struct FakeRoles(HashMap<i64, Vec<i64>>);

    #[async_trait::async_trait]
    impl crate::resolver::UserResolver for FakeRoles {
        async fn user_names(&self, _ids: &[i64]) -> HashMap<i64, String> {
            HashMap::new()
        }

        async fn resolve_designators(&self, _d: &[String]) -> Vec<i64> {
            Vec::new()
        }

        async fn user_role_ids(
            &self,
            user_ids: &[i64],
        ) -> HashMap<i64, Vec<i64>> {
            user_ids
                .iter()
                .filter_map(|u| self.0.get(u).map(|r| (*u, r.clone())))
                .collect()
        }
    }

    fn skip_with_cond(cond: &str) -> crate::model::skip::Skip {
        crate::model::skip::Skip {
            id: None,
            definition_id: 1,
            now_node_code: "a".into(),
            now_node_type: None,
            next_node_code: "b".into(),
            next_node_type: None,
            node_id: None,
            skip_name: None,
            skip_type: None,
            skip_condition: Some(cond.to_string()),
            coordinate: None,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            del_flag: None,
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn prefetch_collects_has_role_users() {
        let skips = vec![
            skip_with_cond("spel@@hasRole(#initiator, 116)"),
            skip_with_cond("spel@@hasRole(leader, 121) && hasRole(5, 114)"),
            skip_with_cond("eq@@leaveDays|2"), // 无 hasRole,不参与
        ];
        let mut vars = HashMap::new();
        vars.insert("initiator".to_string(), serde_json::json!(7));
        vars.insert("leader".to_string(), serde_json::json!(9));
        let resolver = FakeRoles(
            [(7, vec![116]), (9, vec![121]), (5, vec![114])]
                .into_iter()
                .collect(),
        );
        let c = prefetch_spel_context(&resolver, &skips, &vars).await;
        // 变量名与字面量首参都被收集
        assert!(c.user_roles.contains_key(&7));
        assert!(c.user_roles.contains_key(&9));
        assert!(c.user_roles.contains_key(&5));
        // 预热后求值联动:true / false / 数组形式
        assert!(
            eval_condition_with("spel@@hasRole(#initiator, 116)", &vars, &c)
                .unwrap()
        );
        assert!(
            !eval_condition_with("spel@@hasRole(#initiator, 121)", &vars, &c)
                .unwrap()
        );
        assert!(
            eval_condition_with("spel@@hasRole(5, [116, 114])", &vars, &c)
                .unwrap()
        );
        // 引用缺失变量(未预热) → Err
        assert!(
            eval_condition_with("spel@@hasRole(#ghost, 116)", &vars, &c)
                .is_err()
        );
    }

    #[tokio::test]
    async fn prefetch_without_has_role_stays_empty() {
        let skips = vec![skip_with_cond("gt@@leaveDays|2")];
        let resolver = FakeRoles(HashMap::new());
        let c = prefetch_spel_context(&resolver, &skips, &HashMap::new()).await;
        assert!(c.user_roles.is_empty());
    }

    // ─── hasRoleKeys:roleKey 版(跨环境通用,角色 ID 各环境不同 key 不变) ───

    #[test]
    fn spel_has_role_keys_any_match() {
        let vars = ctx("initiator", serde_json::json!(7));
        let c = SpelContext {
            user_role_keys: std::sync::Arc::new(
                [(7, vec!["dept_leader".to_string()])].into_iter().collect(),
            ),
            ..Default::default()
        };
        assert!(
            eval_condition_with(
                "spel@@hasRoleKeys(#initiator, \"dept_leader\")",
                &vars,
                &c
            )
            .unwrap()
        );
        // 数组形式,任一命中
        assert!(
            eval_condition_with(
                "spel@@hasRoleKeys(#initiator, [\"admin\", \"dept_leader\"])",
                &vars,
                &c
            )
            .unwrap()
        );
        assert!(
            !eval_condition_with(
                "spel@@hasRoleKeys(#initiator, [\"admin\", \"boss\"])",
                &vars,
                &c
            )
            .unwrap()
        );
        // 未预热的用户 → Err(与 hasRole 同款 fail-loud)
        assert!(
            eval_condition_with(
                "spel@@hasRoleKeys(#initiator, \"admin\")",
                &ctx("initiator", serde_json::json!(8)),
                &c
            )
            .is_err()
        );
    }

    struct FakeRoleKeys {
        ids: HashMap<i64, Vec<i64>>,
        keys: HashMap<i64, Vec<String>>,
    }

    #[async_trait::async_trait]
    impl crate::resolver::UserResolver for FakeRoleKeys {
        async fn user_names(&self, _ids: &[i64]) -> HashMap<i64, String> {
            HashMap::new()
        }

        async fn resolve_designators(&self, _d: &[String]) -> Vec<i64> {
            Vec::new()
        }

        async fn user_role_ids(
            &self,
            user_ids: &[i64],
        ) -> HashMap<i64, Vec<i64>> {
            user_ids
                .iter()
                .filter_map(|u| self.ids.get(u).map(|r| (*u, r.clone())))
                .collect()
        }

        async fn user_role_keys(
            &self,
            user_ids: &[i64],
        ) -> HashMap<i64, Vec<String>> {
            user_ids
                .iter()
                .filter_map(|u| self.keys.get(u).map(|r| (*u, r.clone())))
                .collect()
        }
    }

    #[tokio::test]
    async fn prefetch_collects_has_role_keys_users() {
        let skips = vec![skip_with_cond(
            "spel@@hasRoleKeys(#initiator, \"dept_leader\")",
        )];
        let mut vars = HashMap::new();
        vars.insert("initiator".to_string(), serde_json::json!(7));
        let resolver = FakeRoleKeys {
            ids: HashMap::new(),
            keys: [(7, vec!["dept_leader".to_string()])].into_iter().collect(),
        };
        let c = prefetch_spel_context(&resolver, &skips, &vars).await;
        assert!(c.user_role_keys.contains_key(&7));
        assert!(c.user_roles.is_empty(), "纯 hasRoleKeys 不查角色ID");
        assert!(
            eval_condition_with(
                "spel@@hasRoleKeys(#initiator, \"dept_leader\")",
                &vars,
                &c
            )
            .unwrap()
        );
    }
}
