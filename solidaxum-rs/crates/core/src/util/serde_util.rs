//! serde 反序列化辅助函数。
//!
//! 出站时大整数(雪花 ID)会被序列化为字符串以保前端精度
//! (见 `vo::response`),因此前端回传时这些字段可能是字符串或数字。
//! 这里提供与 Java/Jackson 默认强制转换等价的入站能力:`i64` 字段
//! 既接受 JSON 数字,也接受数字字符串。

use serde::{Deserialize, Deserializer};

/// 反序列化 `i64`,接受 JSON 数字或数字字符串。
///
/// 用于 JSON 请求体中的 ID 字段(前端可能回传字符串形式的雪花 ID)。
/// 用法:`#[serde(deserialize_with = "de_i64_from_any")]`
pub fn de_i64_from_any<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Num(i64),
        Str(String),
    }
    match NumOrStr::deserialize(deserializer)? {
        NumOrStr::Num(n) => Ok(n),
        NumOrStr::Str(s) => s.trim().parse().map_err(serde::de::Error::custom),
    }
}

/// 反序列化 `Option<i64>`,接受 JSON 数字、数字字符串、null 或空字符串。
///
/// 空字符串与 null 均视为 `None`(对齐前端"未填写"语义)。
/// 用法:`#[serde(default, deserialize_with = "de_opt_i64_from_any")]`
pub fn de_opt_i64_from_any<'de, D>(
    deserializer: D,
) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Num(i64),
        Str(String),
    }
    match Option::<NumOrStr>::deserialize(deserializer)? {
        None => Ok(None),
        Some(NumOrStr::Num(n)) => Ok(Some(n)),
        Some(NumOrStr::Str(s)) if s.trim().is_empty() => Ok(None),
        Some(NumOrStr::Str(s)) => {
            s.trim().parse().map(Some).map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Bo {
        #[serde(deserialize_with = "de_i64_from_any")]
        id: i64,
        #[serde(default, deserialize_with = "de_opt_i64_from_any")]
        dept_id: Option<i64>,
    }

    #[test]
    fn accepts_number_and_string_ids() {
        // 字符串形式的雪花 ID
        let bo: Bo = serde_json::from_str(
            r#"{"id":"1761100000000000000","dept_id":"100"}"#,
        )
        .unwrap();
        assert_eq!(bo.id, 1_761_100_000_000_000_000);
        assert_eq!(bo.dept_id, Some(100));

        // 数字形式
        let bo: Bo =
            serde_json::from_str(r#"{"id":123,"dept_id":456}"#).unwrap();
        assert_eq!(bo.id, 123);
        assert_eq!(bo.dept_id, Some(456));
    }

    #[test]
    fn opt_treats_null_and_empty_as_none() {
        let bo: Bo =
            serde_json::from_str(r#"{"id":1,"dept_id":null}"#).unwrap();
        assert_eq!(bo.dept_id, None);

        let bo: Bo = serde_json::from_str(r#"{"id":1,"dept_id":""}"#).unwrap();
        assert_eq!(bo.dept_id, None);

        let bo: Bo = serde_json::from_str(r#"{"id":1}"#).unwrap();
        assert_eq!(bo.dept_id, None);
    }

    #[test]
    fn rejects_non_numeric_string() {
        let r: Result<Bo, _> = serde_json::from_str(r#"{"id":"abc"}"#);
        assert!(r.is_err());
    }
}
