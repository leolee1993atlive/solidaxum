//! 人类可读字节数的 serde 反序列化。
//!
//! 支持两种写法:
//! - 纯数字(字节): `upload_body_limit = 10485760`
//! - 字符串(带单位,大小写不敏感): `upload_body_limit = "10MB"` / `"1GB"` / `"512KB"`
//!
//! 单位支持: B / K(=KiB=1024) / M / G,以及 KB/MB/GB、KiB/MiB/GiB,
//! 二进制与十进制在此一律按 1024 进制(K/M/G = 1024/1024²/1024³)。

use serde::{Deserialize, Deserializer};

/// 反序列化字节数: 接受 `u64`/整数(原样)或 `&str`(解析单位)。
pub fn deserialize_size_bytes<'de, D>(
    deserializer: D,
) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Num(u64),
        Str(String),
    }

    match Raw::deserialize(deserializer)? {
        Raw::Num(n) => Ok(n as usize),
        Raw::Str(s) => parse_size(&s).map_err(Error::custom),
    }
}

/// 解析 `"10MB"` / `"1GB"` / `"512KB"` / `"10485760"` 形式的字符串为字节数。
pub fn parse_size(input: &str) -> Result<usize, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("大小不能为空".to_string());
    }

    // 找到数字部分的结尾。
    let bytes = s.as_bytes();
    let mut split = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if !(b.is_ascii_digit()) {
            split = i;
            break;
        }
    }

    let (num_part, unit_part) = s.split_at(split);
    let n: u64 = num_part.parse().map_err(|_| {
        format!("无法解析大小 '{input}',支持纯数字(字节)或 10MB/1GB/512KB")
    })?;

    let mul = match unit_part.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1u64,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        other => {
            return Err(format!(
                "未知大小单位 '{other}',支持 B/K/M/G(及 KB/MB/GB、KiB/MiB/GiB)"
            ));
        }
    };

    n.checked_mul(mul)
        .map(|v| v as usize)
        .ok_or_else(|| format!("大小 '{input}' 溢出 usize"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_units() {
        assert_eq!(parse_size("10MB").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("512KB").unwrap(), 512 * 1024);
    }

    #[test]
    fn parses_plain_number_as_bytes() {
        assert_eq!(parse_size("10485760").unwrap(), 10 * 1024 * 1024);
    }

    #[test]
    fn case_insensitive_and_optional_b() {
        assert_eq!(parse_size("10m").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size("10mb").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size("3Mib").unwrap(), 3 * 1024 * 1024);
        assert_eq!(parse_size("2g").unwrap(), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        assert_eq!(parse_size("  10MB  ").unwrap(), 10 * 1024 * 1024);
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(parse_size("10xyz").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_size("").is_err());
    }

    #[test]
    fn deserializer_accepts_integer() {
        #[derive(serde::Deserialize)]
        struct Cfg {
            #[serde(deserialize_with = "deserialize_size_bytes")]
            v: usize,
        }
        let c: Cfg = serde_json::from_str(r#"{"v":10485760}"#).unwrap();
        assert_eq!(c.v, 10 * 1024 * 1024);
    }

    #[test]
    fn deserializer_accepts_string() {
        #[derive(serde::Deserialize)]
        struct Cfg {
            #[serde(deserialize_with = "deserialize_size_bytes")]
            v: usize,
        }
        let c: Cfg = serde_json::from_str(r#"{"v":"10MB"}"#).unwrap();
        assert_eq!(c.v, 10 * 1024 * 1024);
    }
}
