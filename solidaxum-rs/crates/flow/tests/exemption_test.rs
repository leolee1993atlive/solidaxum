//! 豁免读判例(monitor/menu 判例同款):受限 resolver 下照见全量 ——
//! warm-flow/ruoyi-workflow Mapper 零 @DataPermission,工作流数据非行级;
//! `@TableLogic` 存活过滤不受豁免影响。
//!
//! 本判例钉在实体级 select 上:policy 接线(CONSTRAINED=false +
//! soft_delete 声明)端到端生效即绿。
use std::sync::Arc;

use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, EntityTrait},
    testing::{FixedResolver, sqlite_memory_empty},
};
use flow::repository::entity::flow_category;

async fn ds_seeded() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_category::Entity).await.unwrap();
    // 种子面:flow_category 无审计列可盖,insert 直落
    let ctx = ds.system_context();
    let row = |id: i64, name: &str, del: &str| flow_category::ActiveModel {
        category_id: Set(id),
        parent_id: Set(0),
        category_name: Set(name.to_string()),
        order_num: Set(0),
        del_flag: Set(Some(del.to_string())),
        ..Default::default()
    };
    ctx.insert(row(1, "请假流程", "0")).await.unwrap();
    ctx.insert(row(2, "报销流程", "0")).await.unwrap();
    ctx.insert(row(3, "已删分类", "2")).await.unwrap(); // 已软删,不入全量
    ds
}

#[tokio::test]
async fn flow_category_exempt_read_sees_all_alive_rows_under_limited_scope() {
    // system 判例惯用法:先建库种子,再 with_resolver(monitor exemption_test:114)
    let ds = ds_seeded()
        .await
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))));
    let ctx = ds.context(Principal {
        user_id: 8, // SelfOnly(7) 下的"别人",受辖实体对它应不可见
        dept_id: None,
    });
    let rows = ctx
        .select(flow_category::Entity::find())
        .without_row_scope()
        .all()
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "受限范围下豁免读照见全量存活行;已删行仍被 @TableLogic 过滤"
    );
}
