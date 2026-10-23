//! translation 服务集成测试(全豁免)。
//!
//! 覆盖两组语义:
//! 1. 基础翻译路径(经 DataSource 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    翻译读照见全部行 —— 翻译数据非行级(Java 侧 @Translation 处理器
//!    走无注解批量查询,无 Mapper 对应物),漏判的反向钉死。

mod common;

use std::sync::Arc;

use app_data::{DataSource, Principal, ScopeFilter, testing::FixedResolver};
use common::setup_db;
use system::translation::service::translation_service::TranslationService;

/// 服务 + 无 resolver 的 DataSource(system_context 语义)。
fn svc_system(ds: DataSource) -> Arc<DataSource> {
    Arc::new(ds)
}

/// 服务 + 固定 ScopeFilter 的 DataSource(范围用例)。
fn svc_scoped(ds: DataSource, sf: ScopeFilter) -> Arc<DataSource> {
    Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))))
}

#[tokio::test]
async fn test_user_id_to_name() {
    let ds = svc_system(setup_db().await);
    let svc = TranslationService::without_dict_cache();
    let ctx = ds.system_context();

    // 用户 301=u_all, 302=u_custom 存在; 999 不存在
    let map = svc.user_id_to_name(&ctx, &[301, 302, 999]).await.unwrap();
    assert_eq!(map.get(&301).unwrap(), "u_all");
    assert_eq!(map.get(&302).unwrap(), "u_custom");
    assert!(!map.contains_key(&999), "不存在的用户ID不应返回");
}

#[tokio::test]
async fn test_dept_id_to_name() {
    let ds = svc_system(setup_db().await);
    let svc = TranslationService::without_dict_cache();
    let ctx = ds.system_context();

    // 部门 100=总部, 101=技术部 存在; 999 不存在
    let map = svc.dept_id_to_name(&ctx, &[100, 101, 999]).await.unwrap();
    assert_eq!(map.get(&100).unwrap(), "总部");
    assert_eq!(map.get(&101).unwrap(), "技术部");
    assert!(!map.contains_key(&999), "不存在的部门ID不应返回");
}

#[tokio::test]
async fn test_empty_ids() {
    let ds = svc_system(setup_db().await);
    let svc = TranslationService::without_dict_cache();
    let ctx = ds.system_context();

    let user_map = svc.user_id_to_name(&ctx, &[]).await.unwrap();
    assert!(user_map.is_empty(), "空ID列表应返回空Map");

    let dept_map = svc.dept_id_to_name(&ctx, &[]).await.unwrap();
    assert!(dept_map.is_empty(), "空ID列表应返回空Map");
}

/// translation_service.rs:124 — `SELECT oss_id, url FROM sys_oss WHERE oss_id IN ({})`
/// (类别 C,此前唯一缺口:user/dept 已有测试,oss 没有)
#[tokio::test]
async fn test_oss_id_to_url() {
    let ds = svc_system(setup_db().await);
    let svc = TranslationService::without_dict_cache();
    let ctx = ds.system_context();

    // seed: oss 5001 → /oss/a.png, 5002 → /oss/b.jpg;9999 不存在
    let map = svc.oss_id_to_url(&ctx, &[5001, 5002, 9999]).await.unwrap();
    assert_eq!(map.get(&5001).unwrap(), "/oss/a.png");
    assert_eq!(map.get(&5002).unwrap(), "/oss/b.jpg");
    assert!(!map.contains_key(&9999), "不存在的 oss_id 不应返回");

    // 空 ID 列表早退
    let map = svc.oss_id_to_url(&ctx, &[]).await.unwrap();
    assert!(map.is_empty(), "空ID列表应返回空Map");
}

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,翻译读必须照见全部
/// 行 —— 翻译数据非行级:Java 侧 @Translation(USER_ID_TO_NAME 等)处理器
/// 走无注解的批量 id 查询,无 Mapper 对应物;若漏标
/// `without_row_scope`,sys_user/sys_dept(CONSTRAINED=true)会被行级范围
/// 砍行,越权部门的用户/部门名解析缺失,在此变红。
#[tokio::test]
async fn exempt_translation_reads_see_all_rows_despite_scope() {
    let ds = svc_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let svc = TranslationService::without_dict_cache();
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // 401 在 dept 100(越权部门)、403/404 在 103/104(子部门的兄弟分支)
    // —— DeptIn([101]) 的字面语义下这些都不可见,翻译必须全见
    let user_map = svc
        .user_id_to_name(&ctx, &[301, 401, 403, 404])
        .await
        .unwrap();
    assert_eq!(user_map.len(), 4, "用户名翻译不得被部门范围收窄(全豁免)");
    assert_eq!(user_map.get(&401).unwrap(), "target_hq");

    // 部门翻译同口径:100(父)与 103/104(兄弟分支)照常解析
    let dept_map = svc
        .dept_id_to_name(&ctx, &[100, 101, 103, 104])
        .await
        .unwrap();
    assert_eq!(dept_map.len(), 4, "部门名翻译不得被范围收窄(全豁免)");
    assert_eq!(dept_map.get(&100).unwrap(), "总部");

    // oss 翻译(豁免实体)不受影响
    let oss_map = svc.oss_id_to_url(&ctx, &[5001, 5002]).await.unwrap();
    assert_eq!(oss_map.len(), 2, "oss 翻译照常全见");

    // 单值便捷面同口径
    assert_eq!(
        svc.oss_id_to_url_single(&ctx, Some(5001)).await.unwrap(),
        Some("/oss/a.png".to_string())
    );
    assert_eq!(
        svc.oss_id_to_url_single(&ctx, None).await.unwrap(),
        None,
        "None 入参早退"
    );
}
