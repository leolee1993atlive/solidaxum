//! client 仓储集成测试(全豁免)。
//!
//! 覆盖三组语义:
//! 1. 基础 CRUD/分页/改状态(经 `system_context` 桥接;软删过滤由漏斗照施);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    所有读方法照见全部行 —— SysClientMapper 无 @DataPermission,
//!    认证客户端是全局配置(漏判的反向钉死);
//! 3. **软删换轨**:delete_by_id 是 del_flag 的一次 update(post 同款),
//!    已删行不可见、重复删 → NotFound。

mod common;

use std::sync::Arc;

use app_core::{model::entity::BaseEntity, vo::page::PageQuery};
use app_data::{
    DataError, DataSource, Principal, ScopeFilter, testing::FixedResolver,
};
use common::setup_db;
use system::client::{
    model::client::{ClientPageQuery, SysClient},
    repository::client_repo::ClientRepository,
};

fn sample_client() -> SysClient {
    SysClient {
        id: Some(9001),
        client_id: "test_client".to_string(),
        client_key: "test_key".to_string(),
        client_secret: "test_secret".to_string(),
        grant_type: "password,sms".to_string(),
        device_type: Some("pc".to_string()),
        access_path: None,
        ip_whitelist: None,
        active_timeout: 1800,
        timeout: 7200,
        status: "0".to_string(),
        del_flag: Some("0".to_string()),
        base_entity: BaseEntity::new(1),
    }
}

fn repo_system(ds: DataSource) -> (Arc<DataSource>, ClientRepository) {
    let ds = Arc::new(ds);
    (ds.clone(), ClientRepository::new(ds))
}

fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, ClientRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (ds.clone(), ClientRepository::new(ds))
}

fn query_all() -> ClientPageQuery {
    ClientPageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        client_key: None,
        client_secret: None,
        status: None,
    }
}

#[tokio::test]
async fn test_crud_client() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(100),
    });

    let mut client = sample_client();
    repo.insert(&ctx, &client).await.unwrap();

    let found = repo.select_by_id(&ctx, &9001).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.client_id, "test_client");
    assert!(found.supports_grant_type("password"));
    assert!(!found.supports_grant_type("implicit"));

    let by_key = repo.select_by_client_key(&ctx, "test_key").await.unwrap();
    assert_eq!(by_key.unwrap().id, Some(9001));

    client.client_key = "updated_key".to_string();
    client.base_entity.update(1);
    repo.update_by_id(&ctx, &client, &9001).await.unwrap();
    let updated = repo.select_by_id(&ctx, &9001).await.unwrap().unwrap();
    assert_eq!(updated.client_key, "updated_key");

    repo.delete_by_id(&ctx, &9001).await.unwrap();
    assert!(repo.select_by_id(&ctx, &9001).await.unwrap().is_none());
}

#[tokio::test]
async fn test_select_page() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();
    repo.insert(&ctx, &sample_client()).await.unwrap();

    let data = repo
        .select_page(&ctx, &query_all(), 1, 10, "id asc")
        .await
        .unwrap();
    assert!(!data.rows.is_empty());
}

#[tokio::test]
async fn test_change_status() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(100),
    });

    repo.insert(&ctx, &sample_client()).await.unwrap();
    // update_by 由漏斗按 principal 盖章(原 update_by 参数已删)
    repo.change_status(&ctx, 9001, "1").await.unwrap();

    let found = repo.select_by_id(&ctx, &9001).await.unwrap().unwrap();
    assert_eq!(found.status, "1");
    assert_eq!(
        found.base_entity.update_by,
        Some(1),
        "change_status 的 update_by = principal(漏斗盖章)"
    );
}

#[tokio::test]
async fn test_select_page_filters_by_client_secret() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    repo.insert(&ctx, &sample_client()).await.unwrap();
    let mut other = sample_client();
    other.id = Some(9002);
    other.client_key = "other_key".to_string();
    other.client_secret = "other_secret".to_string();
    repo.insert(&ctx, &other).await.unwrap();

    let all = repo
        .select_page(&ctx, &query_all(), 1, 10, "id asc")
        .await
        .unwrap();
    assert_eq!(all.total, 2, "全量应返回 2 条");

    let mut filtered = query_all();
    filtered.client_secret = Some("test_secret".to_string());
    let data = repo
        .select_page(&ctx, &filtered, 1, 10, "id asc")
        .await
        .unwrap();
    assert_eq!(data.total, 1, "按 clientSecret 精确过滤应只剩 1 条");
    assert_eq!(data.rows[0].client_key, "test_key");
}

// ─── 全豁免判例 + 软删换轨 ───

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,客户端读必须照见全部
/// 行 —— SysClientMapper 无 @DataPermission,认证客户端是全局配置;
/// 若任何方法漏标 `without_row_scope`,CONSTRAINED=false 的驱动会
/// fail-closed 报 Unsupported,或(误加范围列时)静默砍行,两者都会
/// 在此变红。
#[tokio::test]
async fn exempt_client_reads_see_all_rows_despite_scope() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // 种两行:create_dept 归属互不相同;豁免读必须两行都见
    let mut mine = sample_client();
    mine.id = Some(9001);
    mine.base_entity = BaseEntity::new(7);
    let mut other = sample_client();
    other.id = Some(9002);
    other.client_key = "other_key".to_string();
    other.client_id = "other_client".to_string();
    repo.insert(&ctx, &mine).await.unwrap();
    repo.insert(&ctx, &other).await.unwrap();

    let page = repo
        .select_page(&ctx, &query_all(), 1, 10, "id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 2, "分页 total 须是全量计数(全豁免)");

    assert!(
        repo.select_by_client_id(&ctx, "other_client")
            .await
            .unwrap()
            .is_some(),
        "登录路径的按 client_id 读必须照见全部行(全豁免)"
    );
    assert!(
        repo.select_by_client_key(&ctx, "other_key")
            .await
            .unwrap()
            .is_some(),
        "唯一性读不得被范围砍小"
    );
    assert!(
        repo.select_by_id(&ctx, &9002).await.unwrap().is_some(),
        "越权部门的客户端对详情读照常可见"
    );

    let list = repo
        .select_list(&ctx, &query_all(), "id asc")
        .await
        .unwrap();
    assert_eq!(list.len(), 2, "导出全量读同豁免");
}

/// **软删换轨**:delete_by_id 是 del_flag 的一次 update(对齐 Java
/// @TableLogic deleteById;post delete_by_id 同款)—— audit 盖
/// update_*,已删行对所有读不可见(漏斗软删过滤),重复删 → NotFound。
#[tokio::test]
async fn soft_delete_hides_row_and_repeat_delete_not_found() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    repo.insert(&ctx, &sample_client()).await.unwrap();
    repo.delete_by_id(&ctx, &9001).await.unwrap();

    // 已删行:各读面均不可见(漏斗软删过滤)
    assert!(repo.select_by_id(&ctx, &9001).await.unwrap().is_none());
    assert!(
        repo.select_by_client_key(&ctx, "test_key")
            .await
            .unwrap()
            .is_none(),
        "已删客户端对唯一性读不可见"
    );
    let page = repo
        .select_page(&ctx, &query_all(), 1, 10, "id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 0, "已删客户端不入分页");

    // 重复删:软删 update 再无存活行 → 0 行 → NotFound
    let err = repo.delete_by_id(&ctx, &9001).await.unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "重复软删应 NotFound,实际: {err:?}"
    );
}
