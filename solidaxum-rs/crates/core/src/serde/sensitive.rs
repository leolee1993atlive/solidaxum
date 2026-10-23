use serde::Serializer;

pub fn mask_phone(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    if len <= 7 {
        return s.to_string();
    }
    let mut result = String::with_capacity(len);
    result.extend(chars.iter().take(3));
    result.push_str("****");
    result.extend(chars.iter().skip(len - 4));
    result
}

pub fn mask_id_card(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    if len <= 7 {
        return s.to_string();
    }
    let mut result = String::with_capacity(len);
    result.extend(chars.iter().take(3));
    for _ in 0..len - 7 {
        result.push('*');
    }
    result.extend(chars.iter().skip(len - 4));
    result
}

pub fn mask_email(s: &str) -> String {
    match s.split_once('@') {
        Some((local, domain)) => {
            if local.is_empty() {
                return s.to_string();
            }
            let first = local.chars().next().expect("local 非空已由守卫保证");
            format!("{}***@{}", first, domain)
        }
        None => s.to_string(),
    }
}

pub fn mask_bank_card(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    if len <= 8 {
        return s.to_string();
    }
    let mut result = String::with_capacity(len);
    result.extend(chars.iter().take(4));
    result.push_str("****");
    result.extend(chars.iter().skip(len - 4));
    result
}

pub fn mask_chinese_name(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    if len == 0 {
        return s.to_string();
    }
    if len == 1 {
        return "*".to_string();
    }
    let mut result = String::with_capacity(len);
    result.push(chars[0]);
    for _ in 1..len - 1 {
        result.push('*');
    }
    if len > 2 {
        result.push(chars[len - 1]);
    } else {
        result.push('*');
    }
    result
}

pub fn serialize_sensitive_phone<S: Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(v) if v.is_empty() => serializer.serialize_str(""),
        Some(v) => serializer.serialize_str(&mask_phone(v)),
        None => serializer.serialize_none(),
    }
}

pub fn serialize_sensitive_email<S: Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(v) if v.is_empty() => serializer.serialize_str(""),
        Some(v) => serializer.serialize_str(&mask_email(v)),
        None => serializer.serialize_none(),
    }
}

pub fn serialize_sensitive_id_card<S: Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(v) if v.is_empty() => serializer.serialize_str(""),
        Some(v) => serializer.serialize_str(&mask_id_card(v)),
        None => serializer.serialize_none(),
    }
}

pub fn serialize_sensitive_bank_card<S: Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(v) if v.is_empty() => serializer.serialize_str(""),
        Some(v) => serializer.serialize_str(&mask_bank_card(v)),
        None => serializer.serialize_none(),
    }
}

pub fn serialize_sensitive_chinese_name<S: Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(v) if v.is_empty() => serializer.serialize_str(""),
        Some(v) => serializer.serialize_str(&mask_chinese_name(v)),
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    #[derive(Serialize)]
    struct TestVo {
        #[serde(serialize_with = "serialize_sensitive_phone")]
        phone: Option<String>,
        #[serde(serialize_with = "serialize_sensitive_email")]
        email: Option<String>,
        #[serde(serialize_with = "serialize_sensitive_id_card")]
        id_card: Option<String>,
        #[serde(serialize_with = "serialize_sensitive_bank_card")]
        bank_card: Option<String>,
        #[serde(serialize_with = "serialize_sensitive_chinese_name")]
        name: Option<String>,
    }

    #[test]
    fn test_mask_phone() {
        assert_eq!(mask_phone("13812345678"), "138****5678");
        assert_eq!(mask_phone("138****1234"), "138****1234");
        assert_eq!(mask_phone("1234567"), "1234567");
    }

    #[test]
    fn test_mask_id_card() {
        assert_eq!(mask_id_card("110101199001011234"), "110***********1234");
    }

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("test@example.com"), "t***@example.com");
        assert_eq!(mask_email("t@example.com"), "t***@example.com");
    }

    #[test]
    fn test_mask_bank_card() {
        assert_eq!(mask_bank_card("6222021234567891234"), "6222****1234");
        assert_eq!(mask_bank_card("6222021234561234"), "6222****1234");
    }

    #[test]
    fn test_mask_chinese_name() {
        assert_eq!(mask_chinese_name("张三"), "张*");
        assert_eq!(mask_chinese_name("张三丰"), "张*丰");
        assert_eq!(mask_chinese_name("欧阳锋"), "欧*锋");
    }

    #[test]
    fn test_serialize_with_masking() {
        let vo = TestVo {
            phone: Some("13812345678".to_string()),
            email: Some("test@example.com".to_string()),
            id_card: Some("110101199001011234".to_string()),
            bank_card: Some("6222021234567891234".to_string()),
            name: Some("张三".to_string()),
        };
        let json = serde_json::to_string(&vo).unwrap();
        assert!(json.contains("138****5678"));
        assert!(json.contains("t***@example.com"));
        assert!(json.contains("110***********1234"));
        assert!(json.contains("6222****1234"));
        assert!(json.contains("张*"));
    }

    #[test]
    fn test_serialize_none_and_empty() {
        let vo = TestVo {
            phone: None,
            email: Some("".to_string()),
            id_card: None,
            bank_card: None,
            name: None,
        };
        let json = serde_json::to_string(&vo).unwrap();
        assert_eq!(
            json,
            r#"{"phone":null,"email":"","id_card":null,"bank_card":null,"name":null}"#
        );
    }
}
