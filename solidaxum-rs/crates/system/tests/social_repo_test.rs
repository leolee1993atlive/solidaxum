//! social 仓储集成测试(全豁免)。
//!
//! 覆盖两组语义:
//! 1. 基础读/写路径(经 `system_context` 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    所有读方法照见全部行 —— SysSocialMapper 无 @DataPermission
//!    (policy.rs 落 CONSTRAINED=false,勿"见 user_id 列加回 owner");
//!    软删行为:del_flag='2' 后读不可见,二次软删 → NotFound
//!    (对齐 Java unlockSocial 的 R.fail 分支,AuthController:161)。

mod common;

use std::sync::Arc;

use app_data::{
    DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::setup_db;
use system::social::repository::social_repo::{
    self as social_entity, SocialRepository,
};

async fn seed_social(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, user_id, source, open_id) in [
        (7001, 1, "wechat", "openid_1"),
        (7002, 1, "weibo", "openid_2"),
        (7003, 2, "qq", "openid_3"),
    ] {
        ctx.insert_unstamped(social_entity::ActiveModel {
            id: Set(id),
            user_id: Set(user_id),
            source: Set(Some(source.into())),
            open_id: Set(Some(open_id.into())),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("种 social 行失败");
    }
}

fn repo_system(ds: DataSource) -> (Arc<DataSource>, SocialRepository) {
    let ds = Arc::new(ds);
    (ds.clone(), SocialRepository::new(ds))
}

fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, SocialRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (ds.clone(), SocialRepository::new(ds))
}

#[tokio::test]
async fn test_select_by_user_id() {
    let ds = setup_db().await;
    seed_social(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    let list = repo.select_by_user_id(&ctx, 1).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn test_select_by_id() {
    let ds = setup_db().await;
    seed_social(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    let found = repo.select_by_id(&ctx, 7001).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().source, Some("wechat".to_string()));

    let missing = repo.select_by_id(&ctx, 9999).await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_soft_delete_by_id_for_user() {
    let ds = setup_db().await;
    seed_social(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    repo.soft_delete_by_id_for_user(&ctx, 7001, 1)
        .await
        .unwrap();

    let list = repo.select_by_user_id(&ctx, 1).await.unwrap();
    assert_eq!(list.len(), 1);

    let missing = repo.select_by_id(&ctx, 7001).await.unwrap();
    assert!(missing.is_none());
}

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,social 读必须照见全部
/// 行(含归属其他用户的绑定行)—— SysSocialMapper 无 @DataPermission,
/// 三方绑定在 Java 侧本就不受数据权限约束;若任何方法漏标
/// `without_row_scope`,CONSTRAINED=false 的驱动会 fail-closed 报
/// Unsupported,本用例变红。
#[tokio::test]
async fn exempt_social_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_social(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // select_by_user_id:他人(user 2)的绑定行照常可见
    let others = repo.select_by_user_id(&ctx, 2).await.unwrap();
    assert_eq!(others.len(), 1, "user_id=2 的绑定行不得被范围砍掉");

    // select_by_id:任何绑定行照常可见
    assert!(
        repo.select_by_id(&ctx, 7003).await.unwrap().is_some(),
        "按主键读不得受范围影响"
    );
}

/// **软删钉测**:软删后行对本模块读不可见(@TableLogic 存活语义,漏斗
/// 照施);二次软删 → NotFound(0 行),不得静默 Ok ——
/// 对齐 Java AuthController.unlockSocial:161 的 rows ? R.ok : R.fail
/// (Java deleteWithValidById 返回受影响行数,0 行即失败)。
#[tokio::test]
async fn soft_delete_hides_row_and_repeat_delete_not_found() {
    use app_data::DataError;

    let ds = setup_db().await;
    seed_social(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    repo.soft_delete_by_id_for_user(&ctx, 7001, 1)
        .await
        .unwrap();

    // 已删行不可再软删(漏斗 update 的软删存活过滤 → 0 行 → NotFound)
    let err = repo
        .soft_delete_by_id_for_user(&ctx, 7001, 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "二次软删必须 NotFound,实际: {err:?}"
    );

    // 非本人绑定 → 读不到 → NotFound(user_id 验属在先读半边)
    let err = repo
        .soft_delete_by_id_for_user(&ctx, 7002, 2)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "非本人绑定的解绑必须 NotFound,实际: {err:?}"
    );
}
