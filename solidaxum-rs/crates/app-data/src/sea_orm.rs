//! 受限 re-export(设计 4.2/4.3):业务 crate `use app_data::sea_orm::*` 能定义
//! 实体、拼完整 Select,却命名不出任何可执行查询的类型 —— 那三个名字被剔除了。
//! 派生宏展开走相对路径 `sea_orm::…`,由本模块接管解析(设计 4.4)。
//!
//! 禁止改成 `pub use sea_orm;` —— 那等于把后门一起 re-export。防线 =
//! 受限面手工枚举不含 sqlx 这类名字(路径 E0433 编译错);trybuild 守
//! 其他面(compile_fail 家族,无 sqlx 专门 case)。
//!
//! 增删本清单的唯一判据:**这个名字能不能让业务执行一条语句?** 能就不许加。
//! 错误类型、列/表标识、条件与语句构造件都不能,所以都在。

// 关系语法糖(BelongsTo/HasMany/HasOne):prelude 经 entity::compound
// 转出这三项,2.0.2 未提升到 sea_orm 根,故从 compound 取,
// 名字仍导出在本模块根。语法糖只声明关系形状,不构成执行能力。
// 方言标签:`SelectQuery::into_sql(backend)` 的参数类型,业务 crate 在
// test-util 下做 SQL 断言必须能命名它。它只是个枚举标签 —— build 返回 String,
// 不构成执行能力(执行仍需 ConnectionTrait/DatabaseConnection,二者仍被剔除)。
// 门控与 into_sql 完全一致:默认面保持最小,与 trybuild stderr 的基线一致。
#[cfg(any(test, feature = "test-util"))]
pub use sea_orm::DbBackend;
pub use sea_orm::entity::compound::{BelongsTo, HasMany, HasOne};
// with-chrono / with-json 的类型别名(实体字段用)。
// NaiveDateTime 供 sys_job/sys_job_log 字段用(NaiveDateTime 类型
// 别名,不能执行语句,判据同 Date/DateTimeUtc)。上游 prelude 未以
// `NaiveDateTime` 原名导出 —— 它在那里叫 `ChronoDateTime`
// (chrono::NaiveDateTime 的 re-export,与 `DateTime` 同型),按原名取回。
pub use sea_orm::entity::prelude::{
    ChronoDateTime as NaiveDateTime, Date, DateTime, DateTimeUtc,
    DateTimeWithTimeZone, Json, Time,
};
// prelude 的 error::* —— 错误类型不构成执行能力,派生宏展开会引用它
// (DbErr / TryGetError / ColumnFromStrErr 都在这一族里)
pub use sea_orm::error::*;
/// sea-query 全量(Alias/Func/IntoIden 等)。给查询构造用;它本身没有连接概念。
pub use sea_orm::sea_query;
// prelude 转出的 sea_query 项(实体定义与列引用要用)。
// 2.0.2 里 Expr 与 IntoCondition 都不在 sea_orm 根命名空间
// (Expr 仅 entity::prelude 转出,IntoCondition 仅 sea_query 根有),
// 故与 DynIden/RcOrArc/SeaRc/StringLen 一并从 sea_query 根取 —— 名字仍导出在本模块根。
pub use sea_orm::sea_query::{
    DynIden, Expr, IntoCondition, RcOrArc, SeaRc, StringLen,
};
/// 嵌入式 strum。`EnumIter` 展开引用 `sea_orm::strum::IntoEnumIterator`,
/// 而枚举遍历不构成执行能力。
pub use sea_orm::strum;
pub use sea_orm::{
    // 关系语法糖
    ActiveBelongsTo,
    // ---- entity::prelude 全量,已剔除 ConnectionTrait / DatabaseConnection / DbConn ----
    ActiveEnum,
    ActiveHasMany,
    ActiveHasOne,

    ActiveModelBehavior,
    ActiveModelTrait,
    // ActiveModel 构造:ActiveValue 34 处(Set 32 / NotSet 24 走 ActiveValue::{Set,NotSet})
    ActiveValue,
    ColumnDef,
    ColumnTrait,
    ColumnType,
    ColumnTypeTrait,
    // ---- prelude 里没有、但业务/派生宏必须有的构造件 ----
    // 条件与表达式:Condition 15 处、Expr 7 处;ExprTrait 是 Expr 上 .eq/.like/.is_in
    // 的方法来源(2.0 起必须在作用域,见 vendor/sea-orm/CLAUDE.md)
    Condition,
    // admin 装配层用它给 DataSource::connect 传参;它自己不能连接
    // (connect 是 Database 的关联函数,database/mod.rs:177)
    ConnectOptions,
    CursorTrait,
    // 宏(macros feature,workspace 已开)
    DeriveActiveEnum,
    DeriveActiveModel,
    DeriveActiveModelBehavior,
    DeriveActiveModelEx,
    DeriveArrowSchema,
    DeriveColumn,
    DeriveDisplay,
    DeriveEntity,
    DeriveEntityModel,
    DeriveIden,
    DeriveIntoActiveModel,
    DeriveModel,
    DeriveModelEx,
    DerivePartialModel,
    DerivePrimaryKey,
    DeriveRelatedEntity,
    DeriveRelation,
    DeriveValueType,
    EntityName,
    EntityTrait,
    EnumIter,
    ExprTrait,
    ForeignKeyAction,
    FromJsonQueryResult,
    FromQueryResult,
    Iden,
    IdenStatic,
    IntoActiveModel,
    // 列枚举遍历 / 部分模型投影
    Iterable,
    // 查询构造:QueryOrder 29 处、QuerySelect 12 处、JoinType 7 处、Order 23 处
    JoinType,
    Linked,
    LoaderTrait,
    ModelTrait,
    Order,
    PaginatorTrait,
    PrimaryKeyArity,
    PrimaryKeyToColumn,
    PrimaryKeyTrait,
    QueryFilter,
    QueryOrder,
    QueryResult,
    QuerySelect,
    QueryTrait,
    Related,
    RelatedSelfVia,
    RelationDef,
    RelationTrait,
    Select,
    SelectExt,
    // DeriveEntityModel 的 ActiveModel 展开引用 sea_orm::TryIntoModel;
    // 它只做 ActiveModel→Model 的 try_into,不构成执行能力
    TryIntoModel,
    Value,
};

/// 派生宏展开引用 `sea_orm::prelude::{DeriveEntity, EntityName, ColumnTypeTrait,
/// EnumIter, DeriveColumn, ColumnTrait, ColumnDef, ColumnType, ForeignKeyAction}`
/// (sea-orm-macros 2.0.2 的 entity_model.rs / relation.rs 展开输出)。
///
/// **禁止**改成 `pub use sea_orm::entity::prelude;` —— 真 prelude 里带着被剔除的
/// `ConnectionTrait` / `DatabaseConnection` / `DbConn`,整模块转出等于开后门。
/// 这里只镜像根清单里已有的名字,不新增任何能力。
pub mod prelude {
    pub use super::{
        ColumnDef, ColumnTrait, ColumnType, ColumnTypeTrait, DeriveColumn,
        DeriveEntity, EntityName, EnumIter, ForeignKeyAction,
    };
}

/// 派生宏展开引用 `sea_orm::entity::{EntityName, EntityTrait, RelationTrait,
/// RelationDef}`(entity.rs / model.rs / relation.rs 展开输出)。
/// 同 [`prelude`]:只镜像根清单里已有的名字。
pub mod entity {
    pub use super::{EntityName, EntityTrait, RelationDef, RelationTrait};
}

/// 派生宏展开(`DeriveEntityModel` 的 ActiveModel 部分)引用
/// `sea_orm::value::{DefaultActiveValue, DefaultActiveValueNone, DefaultActiveValueNotSet}`。
/// 三个都是"字段缺省值来源"trait,不构成执行能力。
pub mod value {
    pub use sea_orm::value::{
        DefaultActiveValue, DefaultActiveValueNone, DefaultActiveValueNotSet,
    };
}

// sqlx 错误类型三别名(core localizer 的错误码翻译用)。判据:纯数据
// 枚举/结构(错误码 + 消息文本),**不能执行语句**,故进受限面;**严禁**
// 转出整个 `sea_orm::sqlx` 模块 —— sqlx 能构造连接池,是真实执行能力。
// 那道门没有 sqlx 专门 trybuild case:防线 = 受限面手工枚举不含 sqlx
// 名字,业务路径即 E0433 编译错;trybuild 守其他面(`no_database` 家族
// 守的是 sea-orm 侧执行名字)。
pub use sea_orm::sqlx::{
    Error as SqlxError, mysql::MySqlDatabaseError, postgres::PgDatabaseError,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// 判例:三别名经受限面可命名,且与 sqlx 本尊类型同一
    /// (恒等闭包到 fn 指针的强转只在类型完全一致时成立)—— core
    /// localizer 换 import 后 downcast / 匹配继续工作靠这一点。
    /// MySqlDatabaseError / PgDatabaseError 无法廉价构造,同一性由
    /// re-export 语义保证,这里钉住"名字能从受限面解析出来"。
    #[test]
    fn sqlx_error_aliases_nameable_and_type_identical() {
        const _: fn(SqlxError) -> sea_orm::sqlx::Error = |e| e;
        const _: fn(
            MySqlDatabaseError,
        ) -> sea_orm::sqlx::mysql::MySqlDatabaseError = |e| e;
        const _: fn(
            PgDatabaseError,
        ) -> sea_orm::sqlx::postgres::PgDatabaseError = |e| e;
        let _: Option<SqlxError> = None;
        let _: Option<MySqlDatabaseError> = None;
        let _: Option<PgDatabaseError> = None;
    }
}
