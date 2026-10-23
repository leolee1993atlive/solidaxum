//! 数据层错误。刻意没有 Forbidden —— 行级范围拒绝一律呈现为 NotFound,
//! 不泄露存在性(设计 8 节)。
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataError {
    #[error("数据不存在")]
    NotFound,
    #[error("数据冲突:唯一键已存在")]
    Conflict,
    #[error(transparent)]
    Backend(#[from] sea_orm::DbErr),
    /// 方言层/join 逃生舱遇到无法表达(或未声明)的构造。fail-closed 用。
    #[error("数据层不支持该构造: {0}")]
    Unsupported(String),
}

impl DataError {
    /// 唯一键冲突判定,由写路径(insert/update)接入。
    ///
    /// **结构化判定,不猜错误文本**:`DbErr::sql_err()` 按驱动自己的错误码分类
    /// (MySQL 1022/1062/1169/1586、Postgres SQLSTATE 23505、SQLite 1555/2067),
    /// 见 `vendor/sea-orm/src/error.rs:252`。
    ///
    /// 曾用的 `s.contains("1062")` 已删除:本项目主键是 18-19 位雪花 ID,而 MySQL
    /// 的约束错误会把出错的值原样回显 —— 一条外键错误的文本里只要碰巧含 "1062"
    /// 就会被误判成 Conflict,把 500 说成 409,并把排查方向带偏。同理不再匹配
    /// "UNIQUE" / "Duplicate entry":这两条已被 sql_err() 的错误码判定覆盖,
    /// 且覆盖得更准(不受错误文本措辞与语言影响)。
    ///
    /// 达梦(ODBC)不经 sqlx,fork 把 `DmError` 拍平成 `query_err(String)`
    /// (`vendor/sea-orm/src/driver/dm.rs:54`),`sql_err()` 对它恒为 `None`,
    /// 因此达梦的唯一键冲突落在 `Backend`(达梦的中文报错不匹配任何文本
    /// 模式)。精确判定属 `dialect` 层的职责(达梦语句级唯一冲突的
    /// SQLSTATE 是 23000,见 `crates/dm/src/pool.rs:376`)。
    pub(crate) fn classify_unique(err: sea_orm::DbErr) -> Self {
        match err.sql_err() {
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_)) => {
                Self::Conflict
            }
            _ => Self::Backend(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归:裸数字子串匹配曾把无关错误判成 Conflict。雪花 ID 里出现 "1062"
    /// 这四位数字完全是常态,不能作为唯一键冲突的判据。
    #[test]
    fn unrelated_error_containing_1062_is_not_a_conflict() {
        let err = sea_orm::DbErr::Custom(
            "Cannot add or update a child row: a foreign key constraint fails \
             (`dept_id` = 1906211062883000001)"
                .to_owned(),
        );
        assert!(
            matches!(DataError::classify_unique(err), DataError::Backend(_)),
            "外键错误里含 1062 不得被判成唯一键冲突"
        );
    }

    /// 同理:错误文本里出现 UNIQUE / Duplicate entry 字样也不再是判据 ——
    /// 判据只有驱动的错误码。
    #[test]
    fn unrelated_error_mentioning_unique_is_not_a_conflict() {
        let err = sea_orm::DbErr::Custom(
            "syntax error near CREATE UNIQUE INDEX".to_owned(),
        );
        assert!(matches!(
            DataError::classify_unique(err),
            DataError::Backend(_)
        ));
    }
}
