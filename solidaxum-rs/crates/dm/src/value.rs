//! `sea_query::Value` 到 ODBC 入参的映射。
//!
//! 覆盖基座实际用到的类型。未覆盖的类型**显式报错**而非静默转 NULL ——
//! 静默丢数据的故障比编译期/运行期报错难排查得多。
//!
//! **时间类型走 ODBC 的定长结构**(`SQL_C_TYPE_TIMESTAMP` 等)而不是文本:
//! C 类型与 SQL 类型一一对应,不经驱动的字符串解析,没有格式与地区设置的余地。
//! 时区约定是一律以 **UTC 墙钟**写入,与读路径的 `DmGetable for DateTime<Utc>`
//! (把达梦的无时区文本按 UTC 解释)严格对称 —— 两侧必须同时改,否则时间会整体偏移。

use chrono::{
    DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike,
};
use odbc_api::{
    DataType, IntoParameter, Nullable,
    parameter::{InputParameter, WithDataType},
    sys,
};
use sea_query::Value;

use crate::DmError;

/// 达梦 `DATETIME` 的默认小数秒精度为 6(微秒),`sql/dm` 的建表脚本用的就是裸
/// `datetime`。按 6 声明可避免驱动因精度不符发出截断警告。
const TIMESTAMP_PRECISION: i16 = 6;

pub fn to_param(v: &Value) -> Result<Box<dyn InputParameter>, DmError> {
    Ok(match v {
        // Option<T>(T: Pod) 有通用的 IntoParameter 实现,空值即 NULL
        Value::TinyInt(x) => Box::new(x.into_parameter()),
        Value::SmallInt(x) => Box::new(x.into_parameter()),
        Value::Int(x) => Box::new(x.into_parameter()),
        Value::BigInt(x) => Box::new(x.into_parameter()),
        Value::Bool(x) => {
            Box::new(x.map(|b| if b { 1i8 } else { 0i8 }).into_parameter())
        }
        Value::Float(x) => Box::new(x.into_parameter()),
        Value::Double(x) => Box::new(x.into_parameter()),
        // 达梦没有无符号整数类型,一律加宽到 BIGINT。u64 超出 i64 的部分无法无损
        // 表达,报错而非回绕 —— 回绕成负数是最难查的一类数据错误。
        Value::TinyUnsigned(x) => Box::new(x.map(i64::from).into_parameter()),
        Value::SmallUnsigned(x) => Box::new(x.map(i64::from).into_parameter()),
        Value::Unsigned(x) => Box::new(x.map(i64::from).into_parameter()),
        Value::BigUnsigned(x) => {
            let widened = x
                .map(|u| {
                    i64::try_from(u).map_err(|_| {
                        DmError::ValueOutOfRange(format!(
                            "u64 {u} 超出达梦 BIGINT 的表示范围"
                        ))
                    })
                })
                .transpose()?;
            Box::new(widened.into_parameter())
        }
        // 必须转成 Option<String> 再 into_parameter:Linux 下 &str 映射为借用型的
        // VarCharSlice,装进 'static 的 Box<dyn InputParameter> 会过不了编译;
        // Option<String> 映射为持有所有权的 VarWCharBox(宽字符)。
        Value::String(x) => {
            Box::new(x.as_deref().map(str::to_owned).into_parameter())
        }
        Value::Char(x) => Box::new(x.map(|c| c.to_string()).into_parameter()),
        Value::Bytes(x) => {
            Box::new(x.as_deref().map(<[u8]>::to_vec).into_parameter())
        }
        // JSON 以文本写入:达梦无原生 JSON 列类型,基座的 def_json 等列本就是 CLOB/VARCHAR。
        Value::Json(x) => {
            Box::new(x.as_deref().map(|j| j.to_string()).into_parameter())
        }
        Value::Uuid(x) => Box::new(x.map(|u| u.to_string()).into_parameter()),
        // ---- 时间:统一折成 UTC 墙钟再交给 ODBC 的定长结构 ----
        Value::ChronoDateTime(x) => timestamp_param(*x),
        Value::ChronoDateTimeUtc(x) => {
            timestamp_param(x.map(|dt| dt.naive_utc()))
        }
        Value::ChronoDateTimeLocal(x) => timestamp_param(x.map(to_utc_naive)),
        Value::ChronoDateTimeWithTimeZone(x) => {
            timestamp_param(x.map(to_utc_naive))
        }
        Value::ChronoDate(x) => date_param(*x),
        Value::ChronoTime(x) => time_param(*x),
        other => {
            return Err(DmError::UnsupportedValue(format!("{other:?}")));
        }
    })
}

/// 带时区的时间统一折算到 UTC 墙钟,与读路径按 UTC 解释无时区文本的约定对称。
fn to_utc_naive<Tz: TimeZone>(dt: DateTime<Tz>) -> NaiveDateTime {
    dt.naive_utc()
}

fn timestamp_param(dt: Option<NaiveDateTime>) -> Box<dyn InputParameter> {
    let value = match dt {
        Some(dt) => Nullable::new(sys::Timestamp {
            year: dt.year() as i16,
            month: dt.month() as u16,
            day: dt.day() as u16,
            hour: dt.hour() as u16,
            minute: dt.minute() as u16,
            second: dt.second() as u16,
            fraction: subsec_nanos_for_dm(&dt),
        }),
        None => Nullable::null(),
    };
    Box::new(WithDataType::new(
        value,
        DataType::Timestamp {
            precision: TIMESTAMP_PRECISION,
        },
    ))
}

fn date_param(d: Option<NaiveDate>) -> Box<dyn InputParameter> {
    let value = match d {
        Some(d) => Nullable::new(sys::Date {
            year: d.year() as i16,
            month: d.month() as u16,
            day: d.day() as u16,
        }),
        None => Nullable::null(),
    };
    Box::new(WithDataType::new(value, DataType::Date))
}

fn time_param(t: Option<NaiveTime>) -> Box<dyn InputParameter> {
    let value = match t {
        // SQL_TIME 不带小数秒,精度声明为 0
        Some(t) => Nullable::new(sys::Time {
            hour: t.hour() as u16,
            minute: t.minute() as u16,
            second: t.second() as u16,
        }),
        None => Nullable::null(),
    };
    Box::new(WithDataType::new(value, DataType::Time { precision: 0 }))
}

/// ODBC 的 `fraction` 字段以**纳秒**计。截到微秒与 `TIMESTAMP_PRECISION` 对齐,
/// 免得驱动因超出列精度而报截断。chrono 在闰秒上会返回 ≥ 1e9,先夹住。
fn subsec_nanos_for_dm(dt: &NaiveDateTime) -> u32 {
    let nanos = dt.nanosecond().min(999_999_999);
    (nanos / 1_000) * 1_000
}

pub fn to_params(
    values: &[Value],
) -> Result<Vec<Box<dyn InputParameter>>, DmError> {
    values.iter().map(to_param).collect()
}

#[cfg(test)]
mod tests {
    use sea_query::Value;

    use super::*;

    #[test]
    fn maps_all_types_used_by_the_base() {
        // 基座实际用到的类型:雪花 ID(i64)、字符串、char(1) 标志位、时间、布尔、空值
        let values = vec![
            Value::BigInt(Some(1957203961908830209)),
            Value::String(Some("达梦".to_owned())),
            Value::String(None),
            Value::Int(Some(-1)),
            Value::Bool(Some(true)),
        ];
        let params = to_params(&values).expect("映射失败");
        assert_eq!(params.len(), 5);
    }

    #[test]
    fn rejects_unsupported_type_loudly() {
        // 未覆盖的类型必须报错,不能静默转成 NULL —— 静默丢数据比报错难查得多。
        // 基座不用 ActiveEnum(见设计说明),Value::Enum 因此刻意不覆盖。
        let v = Value::Enum(sea_query::OptionEnum::None("Test".into()));
        let err = to_param(&v).err().expect("不支持的类型应当报错");
        assert!(
            err.to_string().contains("不支持"),
            "错误信息应指明类型: {err}"
        );
    }

    /// 基座实体实际会绑定的 `Value` 变体全集。
    ///
    /// 少覆盖任何一个,达梦上对应的写操作就会在运行时报 `UnsupportedValue` ——
    /// 而 e2e 测试全部使用字面量 SQL、从不绑定参数,抓不到这类缺口。
    /// 变体清单对应实体里的字段类型:`DateTimeUtc` / `NaiveDateTime`(几乎每张表的
    /// create_time、update_time)、`serde_json::Value`(流程定义 def_json)、
    /// `Vec<u8>`、`u32`/`u64`、`Uuid`。
    #[test]
    fn covers_every_value_variant_the_base_binds() {
        let naive = chrono::NaiveDate::from_ymd_opt(2026, 8, 13)
            .unwrap()
            .and_hms_milli_opt(9, 28, 24, 212)
            .unwrap();

        let cases: Vec<(&str, Value)> = vec![
            ("ChronoDateTime", Value::ChronoDateTime(Some(naive))),
            (
                "ChronoDateTimeUtc",
                Value::ChronoDateTimeUtc(Some(naive.and_utc())),
            ),
            (
                "ChronoDateTimeLocal",
                Value::ChronoDateTimeLocal(Some(naive.and_utc().into())),
            ),
            (
                "ChronoDateTimeWithTimeZone",
                Value::ChronoDateTimeWithTimeZone(Some(naive.and_utc().into())),
            ),
            ("ChronoDate", Value::ChronoDate(Some(naive.date()))),
            ("ChronoTime", Value::ChronoTime(Some(naive.time()))),
            (
                "Json",
                Value::Json(Some(Box::new(serde_json::json!({"a": 1})))),
            ),
            ("Bytes", Value::Bytes(Some(vec![1, 2, 3]))),
            ("TinyUnsigned", Value::TinyUnsigned(Some(1))),
            ("SmallUnsigned", Value::SmallUnsigned(Some(1))),
            ("Unsigned", Value::Unsigned(Some(1))),
            ("BigUnsigned", Value::BigUnsigned(Some(1))),
            ("Uuid", Value::Uuid(Some(uuid::Uuid::nil()))),
        ];

        let missing: Vec<&str> = cases
            .iter()
            .filter(|(_, v)| to_param(v).is_err())
            .map(|(name, _)| *name)
            .collect();
        assert!(
            missing.is_empty(),
            "以下变体未覆盖,达梦上写这些列会运行时报错: {missing:?}"
        );
    }

    /// 每个变体的 NULL 形态同样要能绑定 —— 基座的时间列大多是 `Option<..>`,
    /// 未设值时走的正是 NULL 分支。
    #[test]
    fn covers_null_form_of_every_variant() {
        let cases: Vec<(&str, Value)> = vec![
            ("ChronoDateTime", Value::ChronoDateTime(None)),
            ("ChronoDateTimeUtc", Value::ChronoDateTimeUtc(None)),
            ("ChronoDateTimeLocal", Value::ChronoDateTimeLocal(None)),
            (
                "ChronoDateTimeWithTimeZone",
                Value::ChronoDateTimeWithTimeZone(None),
            ),
            ("ChronoDate", Value::ChronoDate(None)),
            ("ChronoTime", Value::ChronoTime(None)),
            ("Json", Value::Json(None)),
            ("Bytes", Value::Bytes(None)),
            ("BigUnsigned", Value::BigUnsigned(None)),
            ("Uuid", Value::Uuid(None)),
        ];

        let missing: Vec<&str> = cases
            .iter()
            .filter(|(_, v)| to_param(v).is_err())
            .map(|(name, _)| *name)
            .collect();
        assert!(
            missing.is_empty(),
            "以下变体的 NULL 形态未覆盖: {missing:?}"
        );
    }

    /// `u64` 超过 `i64::MAX` 时无法无损转成达梦的 BIGINT。必须报错,
    /// 不能静默回绕成负数 —— 那是最难查的一类数据错误。
    #[test]
    fn rejects_u64_that_does_not_fit_in_bigint() {
        let v = Value::BigUnsigned(Some(u64::MAX));
        let err = to_param(&v).err().expect("溢出必须报错");
        assert!(
            err.to_string().contains("超出"),
            "错误信息应指明溢出: {err}"
        );
    }
}
