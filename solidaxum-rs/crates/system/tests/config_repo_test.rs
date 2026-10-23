//! config 仓储集成测试(全豁免)。
//!
//! 覆盖两组语义:
//! 1. 基础读/写路径(经 `system_context` 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    所有读方法照见全部行 —— SysConfigMapper 无 @DataPermission,
//!    参数配置全局生效(漏判的反向钉死)。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::{
    middleware::extractors::CurrentUser, model::login_user::LoginUser,
    vo::page::PageQuery,
};
use app_data::{DataSource, Principal, ScopeFilter, testing::FixedResolver};
use common::setup_db;
use system::config::{
    model::config::{Config, ConfigCreateBo, ConfigPageQuery},
    repository::config_repo::ConfigRepository,
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

fn repo_system(ds: DataSource) -> (Arc<DataSource>, ConfigRepository) {
    let ds = Arc::new(ds);
    (ds.clone(), ConfigRepository::new(ds))
}

fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, ConfigRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (ds.clone(), ConfigRepository::new(ds))
}

fn empty_query() -> ConfigPageQuery {
    ConfigPageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        config_name: None,
        config_key: None,
        config_type: None,
    }
}

#[tokio::test]
async fn test_select_by_key() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let config = repo
        .select_by_key(&ctx, "sys.user.initPassword")
        .await
        .unwrap()
        .expect("配置不存在");
    assert_eq!(config.config_value, "123456");
}

#[tokio::test]
async fn test_select_page() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let page = repo
        .select_page(&ctx, &empty_query(), 1, 10, "config_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.rows.len(), 2);
}

#[tokio::test]
async fn test_insert_and_delete() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(100),
    });
    let user = test_user();

    let bo = ConfigCreateBo {
        config_name: "测试配置".to_string(),
        config_key: "test.config.key".to_string(),
        config_value: "test_value".to_string(),
        config_type: Some("Y".to_string()),
        remark: None,
    };
    let config = Config::from_create_bo(bo, &user);
    let id = config.config_id.unwrap();

    repo.insert(&ctx, &config).await.unwrap();
    let loaded = repo
        .select_by_id(&ctx, &id)
        .await
        .unwrap()
        .expect("应可读回");
    assert_eq!(
        loaded.base_entity.create_by,
        Some(1),
        "create_by = principal(漏斗盖章)"
    );

    repo.delete_by_id(&ctx, &id).await.unwrap();
    assert!(repo.select_by_id(&ctx, &id).await.unwrap().is_none());
}

/// 范围用例的补充种子:3(create_dept=101)与 4(create_dept=102)——
/// 若参数配置被误施加行级范围,DeptIn([101]) 下 4 号行与 NULL 种子行会消失。
/// 免盖章种子:显式 create_dept/create_by 原样落库(insert_unstamped)。
async fn seed_scope_configs(ds: &DataSource) {
    use app_data::sea_orm::ActiveValue::Set;
    use system::config::repository::config_repo as config_entity;
    let ctx = ds.system_context();
    for (id, name, key, value, create_dept, create_by) in [
        (3, "本部门参数", "scoped.same.dept", "a", 101, 7),
        (4, "越权部门参数", "scoped.other.dept", "b", 102, 8),
    ] {
        ctx.insert_unstamped(config_entity::ActiveModel {
            config_id: Set(id),
            config_name: Set(name.into()),
            config_key: Set(key.into()),
            config_value: Set(value.into()),
            config_type: Set("N".into()),
            create_dept: Set(Some(create_dept)),
            create_by: Set(Some(create_by)),
            ..Default::default()
        })
        .await
        .expect("种范围参数行失败");
    }
}

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,参数配置读必须照见
/// 全部行(含 create_dept=102 的越权行与 create_dept=NULL 的种子行)——
/// SysConfigMapper 无 @DataPermission,参数配置全局生效;若任何方法漏标
/// `without_row_scope`,CONSTRAINED=false 的驱动会 fail-closed 报
/// Unsupported,或(误加范围列时)静默砍行,两者都会在此变红。
#[tokio::test]
async fn exempt_config_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_configs(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // select_by_key / select_by_id:越权行照见(键名唯一性读同口径)
    assert!(
        repo.select_by_key(&ctx, "scoped.other.dept")
            .await
            .unwrap()
            .is_some(),
        "越权部门的参数行对键名读照常可见"
    );
    assert!(
        repo.select_by_id(&ctx, &4).await.unwrap().is_some(),
        "越权部门的参数行对详情读照常可见"
    );

    // select_page:total 不被范围砍小(种子 2 + 补充 2)
    let page = repo
        .select_page(&ctx, &empty_query(), 1, 10, "config_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 4, "分页 total 须是全量计数(全豁免)");

    // select_list(导出)同口径
    let list = repo
        .select_list(&ctx, &empty_query(), "config_id asc")
        .await
        .unwrap();
    assert_eq!(list.len(), 4, "导出全量读同豁免");
}
