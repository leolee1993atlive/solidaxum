// 未 impl DataPolicy 的实体不得进窄 API —— 漏声明策略 = 编译错误(设计 6.3)。
// 顺带:本文件用受限 re-export 定义实体,编译到"缺 DataPolicy"那一步
// 才报错,就说明实体定义本身是通的。
//
// 注意:随附 .stderr 是**默认 feature** 下 bless 的(TRYBUILD=overwrite,
// cargo test -p app_data --test compile_fail)。若开 --features test-util,
// rustc 的 E0277 help 会多列一段 "the following other types implement trait
// DataPolicy"(src/testing.rs 的三个 impl 被编进来),stderr 逐字比对会 mismatch
// —— 跑 trybuild 请用默认 feature(与既有 4 个用例同一口径)。
use app_data::sea_orm::*;
use app_data::DataContext;

mod no_policy {
    use app_data::sea_orm::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "no_policy")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn should_not_compile(ctx: &DataContext<'_>) {
    let _ = ctx.select(no_policy::Entity::find()).all().await;
}

fn main() {}
