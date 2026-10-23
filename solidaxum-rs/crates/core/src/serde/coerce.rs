//! 类型感知的强制转换反序列化器。
//!
//! 出站时超出 JS 安全整数范围的整数(雪花 ID)被序列化为字符串以保精度
//! (见 [`crate::vo::response`]),因此前端回传时这些字段可能是字符串。
//! serde 默认的 `i64` 反序列化不接受字符串,而 Java/Jackson 默认会做
//! 类型驱动的强制转换。本模块用一个包裹 [`serde_json::Value`] 的
//! [`Deserializer`] 复刻该行为:**仅当目标类型是数字、而 JSON 值是字符串时**
//! 才 parse,其余一律按原样委托。转换会递归进入数组、对象等嵌套结构,
//! 因此 `Vec<i64>`、`Option<i64>`、嵌套对象中的整数都自动覆盖。
//!
//! 真正的字符串字段不受影响(只有当目标类型请求数字时才尝试 parse)。

use serde::de::{
    self, Deserializer, IntoDeserializer, MapAccess, SeqAccess, Visitor,
    value::StringDeserializer,
};
use serde_json::Value;

/// 包裹 [`serde_json::Value`],为数字字段提供"字符串→数字"强制转换。
pub struct Coerce(pub Value);

/// 为整数/浮点反序列化方法生成实现:JSON 字符串则 parse,否则委托给底层 Value。
macro_rules! coerce_number {
    ($method:ident, $ty:ty, $visit:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::String(s) => {
                    let n: $ty = s.trim().parse().map_err(de::Error::custom)?;
                    visitor.$visit(n)
                }
                other => other.$method(visitor),
            }
        }
    };
}

impl<'de> Deserializer<'de> for Coerce {
    type Error = serde_json::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            // 容器:包裹子元素以让强制转换递归向下传播
            Value::Array(arr) => visitor.visit_seq(SeqCoerce {
                iter: arr.into_iter(),
            }),
            Value::Object(map) => visitor.visit_map(MapCoerce {
                iter: map.into_iter(),
                value: None,
            }),
            // 标量:委托给 serde_json::Value 自身的反序列化器
            other => other.deserialize_any(visitor),
        }
    }

    coerce_number!(deserialize_i8, i8, visit_i8);
    coerce_number!(deserialize_i16, i16, visit_i16);
    coerce_number!(deserialize_i32, i32, visit_i32);
    coerce_number!(deserialize_i64, i64, visit_i64);
    coerce_number!(deserialize_i128, i128, visit_i128);
    coerce_number!(deserialize_u8, u8, visit_u8);
    coerce_number!(deserialize_u16, u16, visit_u16);
    coerce_number!(deserialize_u32, u32, visit_u32);
    coerce_number!(deserialize_u64, u64, visit_u64);
    coerce_number!(deserialize_u128, u128, visit_u128);
    coerce_number!(deserialize_f32, f32, visit_f32);
    coerce_number!(deserialize_f64, f64, visit_f64);

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    // 容器经由 deserialize_any 处理,确保子元素被 Coerce 包裹
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    // 无嵌套子元素的标量/枚举:直接委托给底层 Value(不影响强制转换语义)
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_enum(name, variants, visitor)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_bool(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_char(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_str(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_string(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_bytes(visitor)
    }

    fn deserialize_byte_buf<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_byte_buf(visitor)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_unit(visitor)
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_unit_struct(name, visitor)
    }

    fn deserialize_identifier<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_identifier(visitor)
    }

    fn deserialize_ignored_any<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_ignored_any(visitor)
    }
}

/// 数组访问器:逐元素包裹为 [`Coerce`]。
struct SeqCoerce {
    iter: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for SeqCoerce {
    type Error = serde_json::Error;

    fn next_element_seed<T>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(v) => seed.deserialize(Coerce(v)).map(Some),
            None => Ok(None),
        }
    }
}

/// 对象访问器:键按字符串处理,值逐个包裹为 [`Coerce`]。
struct MapCoerce {
    iter: serde_json::map::IntoIter,
    value: Option<Value>,
}

impl<'de> MapAccess<'de> for MapCoerce {
    type Error = serde_json::Error;

    fn next_key_seed<K>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((k, v)) => {
                self.value = Some(v);
                let key_de: StringDeserializer<serde_json::Error> =
                    k.into_deserializer();
                seed.deserialize(key_de).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let v = self
            .value
            .take()
            .expect("next_value_seed called before next_key_seed");
        seed.deserialize(Coerce(v))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    fn coerce_from<T: serde::de::DeserializeOwned>(s: &str) -> T {
        let v: Value = serde_json::from_str(s).unwrap();
        T::deserialize(Coerce(v)).unwrap()
    }

    #[derive(Deserialize, PartialEq, Debug)]
    #[serde(rename_all = "camelCase")]
    struct UserBo {
        user_id: i64,
        dept_id: Option<i64>,
        post_ids: Option<Vec<i64>>,
        nick_name: String,
        status: Option<String>,
    }

    #[test]
    fn coerces_string_ids_at_all_shapes() {
        // user_id/dept_id 为字符串,post_ids 数组里也是字符串
        let bo: UserBo = coerce_from(
            r#"{
                "userId": "1761100000000000000",
                "deptId": "100",
                "postIds": ["1761100000000000002", "3"],
                "nickName": "张三",
                "status": "0"
            }"#,
        );
        assert_eq!(bo.user_id, 1_761_100_000_000_000_000);
        assert_eq!(bo.dept_id, Some(100));
        assert_eq!(bo.post_ids, Some(vec![1_761_100_000_000_000_002, 3]));
        assert_eq!(bo.nick_name, "张三");
        assert_eq!(bo.status, Some("0".to_string()));
    }

    #[test]
    fn accepts_plain_numbers_too() {
        let bo: UserBo = coerce_from(
            r#"{"userId": 123, "deptId": 456, "postIds": [1,2], "nickName": "x"}"#,
        );
        assert_eq!(bo.user_id, 123);
        assert_eq!(bo.dept_id, Some(456));
        assert_eq!(bo.post_ids, Some(vec![1, 2]));
        assert_eq!(bo.status, None);
    }

    #[test]
    fn null_option_is_none() {
        let bo: UserBo =
            coerce_from(r#"{"userId":"1","deptId":null,"nickName":"x"}"#);
        assert_eq!(bo.dept_id, None);
        assert_eq!(bo.post_ids, None);
    }

    #[test]
    fn real_string_fields_untouched() {
        // 纯数字的字符串字段(如昵称/状态)不应被转成数字
        let bo: UserBo =
            coerce_from(r#"{"userId":1,"nickName":"12345","status":"1"}"#);
        assert_eq!(bo.nick_name, "12345");
        assert_eq!(bo.status, Some("1".to_string()));
    }

    #[test]
    fn nested_object_coerced() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Outer {
            inner: UserBo,
        }
        let o: Outer = coerce_from(
            r#"{"inner":{"userId":"1761100000000000003","nickName":"x"}}"#,
        );
        assert_eq!(o.inner.user_id, 1_761_100_000_000_000_003);
    }
}
