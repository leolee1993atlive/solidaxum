//! 测试脚手架。
//!
//! 这三个实体同时是**受限 re-export 的验证锚**(设计 11 节风险 1 的缓解措施原文:
//! "第 1 步就用一个真实实体(取 sys_user)验证展开")。它们全部只
//! `use crate::sea_orm::*` —— 编译得过,就等于证明了业务 crate 不需要 sea-orm
//! 依赖也能定义实体(设计 4.4)。
//!
//! # 对外消费契约(test-util 面)
//!
//! 业务 crate 在 `[dev-dependencies]` 里写 `app_data = { features =
//! ["test-util"] }` 后,消费面就是:
//!
//! - `sqlite_memory_empty()` + `DataSource::create_table::<E>()` —— **自带实体
//!   建表的那一对**(admin/src/testkit.rs 的 13 张 flow 表也走这条路);
//! - `DataContext::insert_unstamped` —— **免盖章种子**:审计列
//!   取 ActiveModel 显式 `Set` 的值原样落库,种"create_by=7 的越权行"全靠它;
//!   默认 feature 下不存在(trybuild `no_insert_unstamped` 守着门控);
//! - `FixedResolver` —— 免搭业务范围计算的固定 `ScopeFilter`;
//! - `sandbox`/`sys_user`/`dict_entry` + `sandbox_row`/`seed` —— app-data 自己
//!   的夹具,业务 crate 需要可控实体时也可复用;
//! - `SelectQuery::into_sql(DbBackend)`(配合受限 re-export 的 `DbBackend`)
//!   —— 设计 9.1 的"只 build 不连库"SQL 断言。
//!
//! 对应的**外部消费者视角回归**在 `tests/runtime_read_test.rs` /
//! `tests/runtime_write_test.rs` —— 那两个文件刻意不出现任何对 sea-orm
//! crate 本身的依赖。

/// **从 `system/src/user/repository/user_repo.rs` 原样复制的真实实体,一个字段都不许简化。**
/// 它带着玩具实体没有的东西:`column_name` 属性、19 个字段、`create_dept`、
/// 完整审计四列、软删除列。它编不过,38 个业务实体就编不过。
/// 这里是复制不是依赖:app-data 不认识 system crate(范围声明)。
pub mod sys_user {
    // `{self, *}` 里的 `self` 不能省 —— 少了它,宏展开的 `sea_orm::…` 解析不到本模块(受限面接管失效)。
    use crate::sea_orm::{self, *};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "sys_user")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub user_id: i64,
        pub dept_id: Option<i64>,
        pub user_name: String,
        pub nick_name: String,
        pub user_type: Option<String>,
        pub email: Option<String>,
        #[sea_orm(column_name = "phone_number")]
        pub phone_number: Option<String>,
        pub gender: Option<String>,
        pub avatar: Option<i64>,
        pub password: String,
        pub status: String,
        pub del_flag: String,
        pub login_ip: Option<String>,
        pub login_date: Option<DateTimeUtc>,
        pub create_dept: Option<i64>,
        pub create_by: Option<i64>,
        pub create_time: Option<DateTimeUtc>,
        pub update_by: Option<i64>,
        pub update_time: Option<DateTimeUtc>,
        pub remark: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 受约束实体的最小形状:部门列 / 归属人列 / 软删除列 / 审计四列各一。
/// 运行时用例都跑在它上面 —— 字段少,断言的 SQL 串短。
pub mod sandbox {
    use crate::sea_orm::{self, *};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "sandbox")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub dept_id: Option<i64>,
        pub owner_id: Option<i64>,
        pub del_flag: String,
        pub create_by: Option<i64>,
        pub create_time: Option<DateTimeUtc>,
        pub update_by: Option<i64>,
        pub update_time: Option<DateTimeUtc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// `CONSTRAINED = false` 的代表:字典表,无任何策略列。
/// `remark` 用 `column_type = "Text"` 顺带覆盖 notice/translation 那类实体走的
/// `ColumnType`/`StringLen` 展开路径。
pub mod dict_entry {
    use crate::sea_orm::{self, *};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "dict_entry")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub label: String,
        #[sea_orm(column_type = "Text")]
        pub remark: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 软删守卫的测试形态:CONSTRAINED=false 但
/// 带软删列。workspace 真实实体没有这个组合(已核实:关联表全 None、
/// 业务表全受辖),它是 `delete_where` 第二个 fail-closed 分支
/// (批量物理删绕过 @TableLogic 存活语义)的覆盖锚 —— 防御纵深也得
/// 有测试钉住,否则守卫和死代码无法区分。
pub mod soft_dict {
    use crate::sea_orm::{self, *};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "soft_dict")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub del_flag: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 关联表的最小形状(语义取自 `system/src/relation/user_role.rs`):
/// 复合主键 (user_id, role_id)、无软删列、无审计列、不受辖。
/// `delete_where` 原语的唯一动机就是这类表的 delete-then-insert 模式
/// (复合主键进不了 `delete(id)` 的单列签名),测试也用真实形状。
pub mod rel_link {
    use crate::sea_orm::{self, *};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "rel_link")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub user_id: i64,
        #[sea_orm(primary_key, auto_increment = false)]
        pub role_id: i64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use crate::{
    DataSource,
    error::DataError,
    policy::{AuditColumns, DataPolicy},
    principal::{Principal, ScopeFilter, ScopeResolver},
    sea_orm::ActiveValue::Set,
};

/// 空的 SQLite 内存库 DataSource,**一张表都不建**。
///
/// 业务 crate 用它 + `DataSource::create_table` 建自己的表 —— 这就是设计 9.4
/// 说的 `admin/src/testkit.rs` DB 半部下沉点(那边现在是 `crates/admin/src/testkit.rs:636`
/// 起对 13 张 flow 表逐个 `create_table_from_entity`)。**不要**把建表清单写死在这里,
/// 否则业务侧经 test-util 建不了自己的表。
pub async fn sqlite_memory_empty() -> DataSource {
    DataSource::from_connection(sqlite_memory_conn().await)
}

/// 裸连接形态的 SQLite 内存库构造(crate 私有):裸连接不离开本 crate,
/// 唯一消费者是 [`sqlite_memory_empty`]。
/// 关键语义保留:内存库必须单连接,否则不同连接看到不同的库
/// (同 admin testkit 的既有处置,事务用例尤其依赖这一点)。
async fn sqlite_memory_conn() -> sea_orm::DatabaseConnection {
    use sea_orm::{ConnectOptions, Database};
    let mut opt = ConnectOptions::new("sqlite::memory:");
    opt.max_connections(1).min_connections(1);
    Database::connect(opt).await.expect("sqlite 内存库连接")
}

/// app-data 自己那三张固定表的便捷版。业务 crate 别用这个,用
/// `sqlite_memory_empty()` + `create_table`。
pub async fn sqlite_memory() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(sandbox::Entity)
        .await
        .expect("建 sandbox 表");
    ds.create_table(dict_entry::Entity)
        .await
        .expect("建 dict_entry 表");
    ds.create_table(sys_user::Entity)
        .await
        .expect("建 sys_user 表");
    ds.create_table(rel_link::Entity)
        .await
        .expect("建 rel_link 表");
    ds.create_table(soft_dict::Entity)
        .await
        .expect("建 soft_dict 表");
    ds
}

/// 返回固定 `ScopeFilter` 的 resolver,免得每个用例都去搭一套业务范围计算。
pub struct FixedResolver(pub ScopeFilter);

#[async_trait::async_trait]
impl ScopeResolver for FixedResolver {
    async fn resolve(&self, _p: &Principal) -> Result<ScopeFilter, DataError> {
        Ok(self.0.clone())
    }
}

/// 构造一行 sandbox 的 ActiveModel(审计列留 NotSet,交给漏斗盖章)。
pub fn sandbox_row(
    id: i64,
    dept_id: Option<i64>,
    owner_id: Option<i64>,
) -> sandbox::ActiveModel {
    sandbox::ActiveModel {
        id: Set(id),
        dept_id: Set(dept_id),
        owner_id: Set(owner_id),
        del_flag: Set("0".into()),
        ..Default::default()
    }
}

/// 种子数据。**走 `system_context()` 的 insert,不绕漏斗** —— 设计 9.4 要求
/// "种子数据与断言全部经 DataContext / TxnContext / system_context()"。
/// `system_context` 跳过行级范围,所以可以种别人部门的行;`insert` 不过滤软删除,
/// 所以可以种 `del_flag = "2"` 的已删行。两种越权/已删场景都种得出来,不需要后门。
pub async fn seed(ds: &DataSource, rows: Vec<(i64, i64, i64, &str)>) {
    let ctx = ds.system_context();
    for (id, dept_id, owner_id, del_flag) in rows {
        let mut am = sandbox_row(id, Some(dept_id), Some(owner_id));
        am.del_flag = Set(del_flag.into());
        ctx.insert(am).await.expect("种子插入");
    }
}

impl DataPolicy for sandbox::Entity {
    fn dept_column() -> Option<sandbox::Column> {
        Some(sandbox::Column::DeptId)
    }
    fn owner_column() -> Option<sandbox::Column> {
        Some(sandbox::Column::OwnerId)
    }
    fn soft_delete() -> Option<(sandbox::Column, &'static str)> {
        Some((sandbox::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<AuditColumns<sandbox::ActiveModel>> {
        use crate::sea_orm::{ActiveValue::Set, DateTimeUtc};
        Some(AuditColumns {
            create_dept: None, // sandbox 无此列
            create_by: Some(|m: &mut sandbox::ActiveModel, v: i64| {
                m.create_by = Set(Some(v))
            }),
            create_time: Some(
                |m: &mut sandbox::ActiveModel, v: DateTimeUtc| {
                    m.create_time = Set(Some(v))
                },
            ),
            update_by: Some(|m: &mut sandbox::ActiveModel, v: i64| {
                m.update_by = Set(Some(v))
            }),
            update_time: Some(
                |m: &mut sandbox::ActiveModel, v: DateTimeUtc| {
                    m.update_time = Set(Some(v))
                },
            ),
        })
    }
}

/// 真实实体的真实声明:归属人列就是主键本身(`sys_user` 的"自己"= user_id),
/// 这正是 RuoYi `SELF` 档在 sys_user 上的落法。
impl DataPolicy for sys_user::Entity {
    fn dept_column() -> Option<sys_user::Column> {
        Some(sys_user::Column::DeptId)
    }
    fn owner_column() -> Option<sys_user::Column> {
        Some(sys_user::Column::UserId)
    }
    fn soft_delete() -> Option<(sys_user::Column, &'static str)> {
        Some((sys_user::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<AuditColumns<sys_user::ActiveModel>> {
        use crate::sea_orm::{ActiveValue::Set, DateTimeUtc};
        Some(AuditColumns {
            create_dept: Some(|m: &mut sys_user::ActiveModel, v: i64| {
                m.create_dept = Set(Some(v))
            }),
            create_by: Some(|m: &mut sys_user::ActiveModel, v: i64| {
                m.create_by = Set(Some(v))
            }),
            create_time: Some(
                |m: &mut sys_user::ActiveModel, v: DateTimeUtc| {
                    m.create_time = Set(Some(v))
                },
            ),
            update_by: Some(|m: &mut sys_user::ActiveModel, v: i64| {
                m.update_by = Set(Some(v))
            }),
            update_time: Some(
                |m: &mut sys_user::ActiveModel, v: DateTimeUtc| {
                    m.update_time = Set(Some(v))
                },
            ),
        })
    }
}

/// 字典表:全局表,不受行级约束,也没有任何策略列。
impl DataPolicy for dict_entry::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<dict_entry::Column> {
        None
    }
    fn owner_column() -> Option<dict_entry::Column> {
        None
    }
    fn soft_delete() -> Option<(dict_entry::Column, &'static str)> {
        None
    }
    fn audit_columns() -> Option<AuditColumns<dict_entry::ActiveModel>> {
        None
    }
}

/// 软删豁免形态(见 soft_dict 模块注):无范围列、不受辖,但声明软删 ——
/// `delete_where` 对它必须走**第二个** fail-closed 守卫。
impl DataPolicy for soft_dict::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<soft_dict::Column> {
        None
    }
    fn owner_column() -> Option<soft_dict::Column> {
        None
    }
    fn soft_delete() -> Option<(soft_dict::Column, &'static str)> {
        Some((soft_dict::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<AuditColumns<soft_dict::ActiveModel>> {
        None
    }
}

/// 关联表(设计 6.3:关联键不是归属,user_id/dept_id 全 None)—— 与
/// system 侧 `relation/*` 的 DataPolicy 声明同构,`delete_where` 只对
/// 这类实体放行。
impl DataPolicy for rel_link::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<rel_link::Column> {
        None
    }
    fn owner_column() -> Option<rel_link::Column> {
        None
    }
    fn soft_delete() -> Option<(rel_link::Column, &'static str)> {
        None
    }
    fn audit_columns() -> Option<AuditColumns<rel_link::ActiveModel>> {
        None
    }
}
