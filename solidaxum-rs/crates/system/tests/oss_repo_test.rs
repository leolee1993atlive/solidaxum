//! oss / oss_config 仓储集成测试(全豁免)。
//!
//! 覆盖:
//! 1. **全豁免判例**:resolver 给出 DeptIn 时,oss / oss_config 读照见
//!    全部行 —— SysOssMapper / SysOssConfigMapper 均无 @DataPermission
//!    (文件记录与桶配置都是全局资源,可见性不经数据范围);
//! 2. 写路径语义:insert 审计盖章(principal)、物理删 0 行 → NotFound、
//!    change_status 的全表 N + 目标 Y 事务半边(in-tx 原语)。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::{
    middleware::extractors::CurrentUser, model::login_user::LoginUser,
    vo::page::PageQuery,
};
use app_data::{
    DataError, DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::setup_db;
use system::oss::{
    model::oss::{OssConfigQueryBo, OssQueryBo, SysOss},
    repository::{
        oss_config_repo::{self as oss_config_entity, OssConfigRepository},
        oss_repo::{self as oss_entity, OssRepository},
    },
};

fn test_user() -> CurrentUser {
    CurrentUser(LoginUser {
        user_id: 1,
        dept_id: Some(100),
        username: "test".to_string(),
        nick_name: Some("测试".to_string()),
        user_type: Some("sys_user".to_string()),
        client_id: "client".to_string(),
        client_key: None,
        device_type: None,
        permissions: HashSet::new(),
        roles: HashSet::new(),
        login_ip: None,
        login_date: None,
    })
}

fn repos_system(
    ds: DataSource,
) -> (Arc<DataSource>, OssRepository, OssConfigRepository) {
    let ds = Arc::new(ds);
    (
        ds.clone(),
        OssRepository::new(ds.clone()),
        OssConfigRepository::new(ds),
    )
}

fn repos_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, OssRepository, OssConfigRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (
        ds.clone(),
        OssRepository::new(ds.clone()),
        OssConfigRepository::new(ds),
    )
}

fn scoped_principal() -> Principal {
    Principal {
        user_id: 301,
        dept_id: Some(101),
    }
}

fn empty_oss_query() -> OssQueryBo {
    OssQueryBo {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        file_name: None,
        original_name: None,
        file_suffix: None,
        service: None,
    }
}

fn empty_config_query() -> OssConfigQueryBo {
    OssConfigQueryBo {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        config_key: None,
        bucket_name: None,
        status: None,
    }
}

/// 范围种子:oss 行带 create_dept 区分(5003 本部门 101 / 5004 越权 102)。
/// 免盖章种子:显式 create_dept 原样落库(insert_unstamped)。
async fn seed_scope_oss(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, file_name, suffix, create_dept) in
        [(5003, "c.png", ".png", 101), (5004, "d.jpg", ".jpg", 102)]
    {
        ctx.insert_unstamped(oss_entity::ActiveModel {
            oss_id: Set(id),
            file_name: Set(file_name.into()),
            original_name: Set(file_name.into()),
            file_suffix: Set(suffix.into()),
            url: Set(format!("/oss/{file_name}")),
            service: Set("minio".into()),
            create_dept: Set(Some(create_dept)),
            ..Default::default()
        })
        .await
        .expect("种范围 oss 行失败");
    }
}

/// **oss 全豁免判例**:DeptIn([101]) 下,越权部门(create_dept=102)的
/// 文件记录对按主键读 / 按 id 集读 / 分页读全部照常可见 ——
/// SysOssMapper 无 @DataPermission;漏标 without_row_scope 会 fail-closed
/// 报 Unsupported,本用例变红。
#[tokio::test]
async fn exempt_oss_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_oss(&ds).await;
    let (ds, oss_repo, _) = repos_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(scoped_principal());

    // select_by_id:越权行照见
    assert!(
        oss_repo.select_by_id(&ctx, &5004).await.unwrap().is_some(),
        "越权部门的文件记录对按主键读照常可见"
    );

    // select_by_ids:全量返回(5001/5002 种子 + 5003/5004)
    let rows = oss_repo
        .select_by_ids(&ctx, &[5001, 5002, 5003, 5004])
        .await
        .unwrap();
    assert_eq!(rows.len(), 4, "按 id 集读不得被范围砍行");

    // select_page:total 全量口径(种子 2 + 补充 2)
    let page = oss_repo
        .select_page(&ctx, &empty_oss_query(), 1, 10, "oss_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 4, "分页 total 须是全量计数(全豁免)");
}

/// **oss 写路径**:insert 审计五列由漏斗按 principal 盖章
/// (模型层 new_upload 的手工盖章被漏斗覆盖,同值来源变化)。
#[tokio::test]
async fn oss_insert_stamps_audit_from_principal() {
    let (ds, oss_repo, _) = repos_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(101),
    });

    let oss = SysOss::new_upload(
        "x.png".to_string(),
        "x.png".to_string(),
        "png".to_string(),
        "/oss/x.png".to_string(),
        "minio".to_string(),
        &test_user(),
    );
    let id = oss.oss_id.unwrap();
    oss_repo.insert(&ctx, &oss).await.unwrap();

    let loaded = oss_repo
        .select_by_id(&ctx, &id)
        .await
        .unwrap()
        .expect("插入后应可读回");
    assert_eq!(
        loaded.base_entity.create_by,
        Some(42),
        "create_by = principal(漏斗盖章)"
    );
    assert!(
        loaded.base_entity.create_time.is_some(),
        "create_time 由漏斗盖"
    );
}

/// **oss 物理删**:已删(sys_oss 无软删列,物理)0 行 → NotFound;
/// 删除后读不可见。
#[tokio::test]
async fn oss_delete_missing_row_is_not_found() {
    let (ds, oss_repo, _) = repos_system(setup_db().await);
    let ctx = ds.system_context();

    let err = oss_repo.delete_by_id(&ctx, &9999).await.unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "删除不存在的记录必须 NotFound,实际: {err:?}"
    );

    oss_repo.delete_by_id(&ctx, &5001).await.unwrap();
    assert!(
        oss_repo.select_by_id(&ctx, &5001).await.unwrap().is_none(),
        "删除后不可见"
    );
}

/// 范围种子:oss_config 行带 create_dept 区分
/// (5103 本部门 101 / 5104 越权 102,后者 status='Y' 兼测装配读)。
/// 免盖章种子:显式 create_dept 原样落库(insert_unstamped)。
async fn seed_scope_oss_config(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, config_key, endpoint, status, create_dept) in [
        (5103, "minio", "http://minio:9000", "N", 101),
        (5104, "local", "file://data/oss", "Y", 102),
    ] {
        ctx.insert_unstamped(oss_config_entity::ActiveModel {
            oss_config_id: Set(id),
            config_key: Set(config_key.into()),
            access_key: Set("ak".into()),
            secret_key: Set("sk".into()),
            bucket_name: Set("bucket".into()),
            // prefix/domain_url/is_https/region 实体衍生 DDL 无默认值
            // (原手写 DDL 的 DEFAULT ''/'N'),种子显式给同值
            prefix: Set("".into()),
            endpoint: Set(endpoint.into()),
            domain_url: Set("".into()),
            is_https: Set("N".into()),
            region: Set("".into()),
            status: Set(status.into()),
            access_policy: Set("1".into()),
            create_dept: Set(Some(create_dept)),
            ..Default::default()
        })
        .await
        .expect("种范围 oss_config 行失败");
    }
}

/// **oss_config 全豁免判例**:DeptIn([101]) 下,越权部门的桶配置对
/// by_config_key(唯一性校验 + S3 客户端装配读)/ by_status(默认配置
/// 装配读)/ by_id / 分页全部照常可见 —— SysOssConfigMapper 无
/// @DataPermission。
#[tokio::test]
async fn exempt_oss_config_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_oss_config(&ds).await;
    let (ds, _, config_repo) = repos_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(scoped_principal());

    assert!(
        config_repo
            .select_by_config_key(&ctx, "local")
            .await
            .unwrap()
            .is_some(),
        "越权部门的配置对 config_key 读照常可见(唯一性/装配读须全量)"
    );
    assert!(
        config_repo
            .select_by_status(&ctx, "Y")
            .await
            .unwrap()
            .is_some(),
        "默认配置装配读(status=Y)不受范围影响"
    );
    assert!(
        config_repo
            .select_by_id(&ctx, &5104)
            .await
            .unwrap()
            .is_some(),
        "越权部门的配置对按主键读照常可见"
    );

    let page = config_repo
        .select_page(&ctx, &empty_config_query(), 1, 10, "oss_config_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 2, "分页 total 须是全量计数(全豁免)");
}

/// **change_status 事务半边**:全表置 N + 目标置 Y 在同一事务
/// (Java updateOssConfigStatus @Transactional ServiceImpl:214);
/// 逐行 tx.update 的审计盖章由漏斗照施(update_by = principal),
/// **两个半边都钉**(全表半边是行为变化点:Java updateCount 不触审计列)。
#[tokio::test]
async fn oss_config_change_status_updates_all_and_target_in_txn() {
    let ds = setup_db().await;
    seed_scope_oss_config(&ds).await;
    let (ds, _, config_repo) = repos_system(ds);
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(101),
    });

    ctx.transaction(|tx| {
        Box::pin(async move {
            OssConfigRepository::update_status_all_in_txn(tx, "N").await?;
            OssConfigRepository::update_status_by_id_in_txn(tx, &5104, "Y")
                .await
        })
    })
    .await
    .unwrap();

    let all_n = config_repo
        .select_by_id(&ctx, &5103)
        .await
        .unwrap()
        .expect("5103 应在");
    assert_eq!(all_n.status, "N", "全表半边:其余配置全部置 N");
    // 全表半边的审计盖章与 Java 有差异:Java 侧 `.updateCount()`
    // 不触审计列,Rust 逐行 tx.update 由漏斗盖 update_by —— 若被"优化"
    // 回裸 update_many,此断言变红(status 断言照绿,盖章静默消失)。
    assert_eq!(
        all_n.base_entity.update_by,
        Some(42),
        "全表半边同样按 principal 盖 update_by(逐行 tx.update)"
    );

    let target = config_repo
        .select_by_id(&ctx, &5104)
        .await
        .unwrap()
        .expect("5104 应在");
    assert_eq!(target.status, "Y", "目标半边:指定配置置 Y");
    assert_eq!(
        target.base_entity.update_by,
        Some(42),
        "漏斗按 principal 盖 update_by"
    );
}

/// **oss_config 物理删 0 行 → NotFound**(原实现静默 Ok;漏斗
/// delete 的 0 行语义,notice/client 删除路径同款裁定)。
#[tokio::test]
async fn oss_config_delete_missing_is_not_found() {
    let (ds, _, config_repo) = repos_system(setup_db().await);
    let ctx = ds.system_context();

    let err = config_repo.delete_by_id(&ctx, &9999).await.unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "删除不存在的配置必须 NotFound,实际: {err:?}"
    );
}
