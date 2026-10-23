// 外部 crate 视角的真实实体验证:本文件由 trybuild 编译成一个只依赖 app_data
// 的独立 crate —— extern prelude 里没有 sea-orm。它能编过,就等于证明业务
// crate 不加 sea-orm 依赖也能定义实体(设计 11 节风险 1 关闭)。
//
// 派生宏展开引用相对路径 `sea_orm::prelude::…`,必须把 `sea_orm` 这个名字
// 绑定到 `app_data::sea_orm` —— 绑定发生在实体模块内部的
// `use app_data::sea_orm::{self, *};`(设计 4.4;业务 crate 同理,在定义实体的
// 模块里写这一句就够)。文件顶部这行 glob 供 main 里调用 EntityName 等能力。
use app_data::sea_orm::*;

/// 从 `system/src/user/repository/user_repo.rs` 原样复制的真实实体,
/// 与 src/testing.rs 的 sys_user 逐字相同。
mod sys_user {
    use app_data::sea_orm::{self, *};

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

fn main() {
    // 引用一下实体,证明 EntityTrait 等路径真的可用,不是"编过但没用"。
    let _ = sys_user::Entity.table_name();
}
