//! notice 仓储集成测试(全豁免)。
//!
//! 覆盖两组语义:
//! 1. 基础读/写路径(经 `system_context` 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    所有读方法照见全部行 —— SysNoticeMapper 无 @DataPermission,
//!    公告是全站内容(漏判的反向钉死);create_by_name 搜索辅助读
//!    (sys_user)同豁免口径。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::{
    middleware::extractors::CurrentUser, model::login_user::LoginUser,
    vo::page::PageQuery,
};
use app_data::{
    DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::setup_db;
use system::notice::{
    model::notice::{Notice, NoticeCreateBo, NoticePageQuery},
    repository::notice_repo::{self as notice_entity, NoticeRepository},
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

fn repo_system(ds: DataSource) -> (Arc<DataSource>, NoticeRepository) {
    let ds = Arc::new(ds);
    (ds.clone(), NoticeRepository::new(ds))
}

fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, NoticeRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (ds.clone(), NoticeRepository::new(ds))
}

fn empty_query() -> NoticePageQuery {
    NoticePageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        notice_title: None,
        notice_type: None,
        status: None,
        create_by_name: None,
    }
}

#[tokio::test]
async fn test_insert_and_delete() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(100),
    });
    let user = test_user();

    let bo = NoticeCreateBo {
        notice_title: "单元测试通知".to_string(),
        notice_type: Some("1".to_string()),
        notice_content: Some("测试内容".to_string()),
        status: None,
        remark: Some("备注".to_string()),
    };
    let notice = Notice::from_create_bo(bo, &user);
    let id = notice.notice_id.unwrap();

    repo.insert(&ctx, &notice).await.unwrap();
    let loaded = repo
        .select_by_id(&ctx, &id)
        .await
        .unwrap()
        .expect("插入后应可读回");
    assert_eq!(
        loaded.base_entity.create_by,
        Some(1),
        "create_by = principal(漏斗盖章)"
    );

    repo.delete_by_id(&ctx, &id).await.unwrap();
    assert!(repo.select_by_id(&ctx, &id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_select_page() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let data = repo
        .select_page(&ctx, &empty_query(), 1, 10, "create_time desc")
        .await
        .unwrap();
    assert!(data.total >= 2);
    assert!(!data.rows.is_empty());
}

/// 范围用例的补充种子:公告行带 create_dept/create_by 区分归属
/// (1761800000000000003 create_dept=101 / ...004 create_dept=102)。
/// 免盖章种子:显式审计值原样落库(insert_unstamped)。
async fn seed_scope_notices(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, title, content, create_dept, create_by) in [
        (1761800000000000003_i64, "本部门公告", "a", 101, 301),
        (1761800000000000004_i64, "越权部门公告", "b", 102, 401),
    ] {
        ctx.insert_unstamped(notice_entity::ActiveModel {
            notice_id: Set(id),
            notice_title: Set(title.into()),
            notice_type: Set("1".into()),
            notice_content: Set(Some(content.into())),
            status: Set("0".into()),
            create_dept: Set(Some(create_dept)),
            create_by: Set(Some(create_by)),
            ..Default::default()
        })
        .await
        .expect("种范围公告行失败");
    }
}

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,公告读必须照见全部行
/// (含 create_dept=102 的越权行与 create_dept=NULL 的种子行)——
/// SysNoticeMapper 无 @DataPermission,公告是全站内容;若任何方法漏标
/// `without_row_scope`,CONSTRAINED=false 的驱动会 fail-closed 报
/// Unsupported,或(误加范围列时)静默砍行,两者都会在此变红。
#[tokio::test]
async fn exempt_notice_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_notices(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // select_by_id:越权行照见
    assert!(
        repo.select_by_id(&ctx, &1761800000000000004)
            .await
            .unwrap()
            .is_some(),
        "越权部门的公告对详情读照常可见"
    );

    // select_page:total 不被范围砍小(种子 2 + 补充 2)
    let page = repo
        .select_page(&ctx, &empty_query(), 1, 10, "notice_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 4, "分页 total 须是全量计数(全豁免)");
}

/// **create_by_name 搜索辅助读**:按用户名解析 user_id 的子查询是搜索
/// 条件而非权限(post belong_subtree 同裁定)——DeptIn([101]) 下
/// "u_all"(dept 101)与 "target_hq"(dept 100,越权部门)的名字都能解析,
/// 命中各自创建的公告。
#[tokio::test]
async fn create_by_name_search_resolves_users_despite_scope() {
    let ds = setup_db().await;
    seed_scope_notices(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // u_all(301,dept 101)创建了 ...003;target_hq(401,dept 100)创建了 ...004
    let mut q = empty_query();
    q.create_by_name = Some("u_all".to_string());
    let page = repo
        .select_page(&ctx, &q, 1, 10, "notice_id asc")
        .await
        .unwrap();
    assert_eq!(
        page.total, 1,
        "按 u_all 搜索应命中其创建的 1 条公告(解析不受范围影响)"
    );

    let mut q = empty_query();
    q.create_by_name = Some("target_hq".to_string());
    let page = repo
        .select_page(&ctx, &q, 1, 10, "notice_id asc")
        .await
        .unwrap();
    assert_eq!(
        page.total, 1,
        "越权部门用户名照常可解析,命中其创建的公告(搜索非权限)"
    );

    // 名字不存在的用户 → 空页(早退分支)
    let mut q = empty_query();
    q.create_by_name = Some("no_such_user".to_string());
    let page = repo
        .select_page(&ctx, &q, 1, 10, "notice_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 0, "无匹配用户应返回空页");
}

/// **软删用户钉测**:create_by_name 子查询经漏斗带 sys_user 的
/// del_flag='0' 存活过滤(对齐 Java @TableLogic 给
/// `userMapper.lambda().eq(...).voOne()` 的自动追加)—— 已删用户的
/// 名字不再命中,其公告不出现在结果里。若有人把子查询改回裸读
/// (漏 DelFlag),本用例变红;范围豁免用例不覆盖
/// 这一面,故单独钉死。
#[tokio::test]
async fn create_by_name_excludes_soft_deleted_users() {
    let ds = setup_db().await;
    // 种子:del_flag='1' 的用户 410(deleted_user,用户名在种子集中唯一,
    // contains 不会误伤其他用户)+ 该用户创建的一条公告(免盖章:
    // del_flag='1' 与 create_by=410 原样落库)
    let ctx = ds.system_context();
    ctx.insert_unstamped(system::user::repository::user_repo::ActiveModel {
        user_id: Set(410),
        dept_id: Set(Some(101)),
        user_name: Set("deleted_user".into()),
        nick_name: Set("已删用户".into()),
        password: Set("$2a$10$placeholder".into()),
        status: Set("0".into()),
        del_flag: Set("1".into()),
        ..Default::default()
    })
    .await
    .expect("种软删用户失败");
    ctx.insert_unstamped(notice_entity::ActiveModel {
        notice_id: Set(1761800000000000005),
        notice_title: Set("已删用户的公告".into()),
        notice_type: Set("1".into()),
        notice_content: Set(Some("x".into())),
        status: Set("0".into()),
        create_dept: Set(Some(101)),
        create_by: Set(Some(410)),
        ..Default::default()
    })
    .await
    .expect("种已删用户的公告失败");
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    // 以已删用户名搜索:子查询解析不到存活用户 → 早退空页,
    // 该用户名下的公告(存在!)不得出现
    let mut q = empty_query();
    q.create_by_name = Some("deleted_user".to_string());
    let page = repo
        .select_page(&ctx, &q, 1, 10, "notice_id asc")
        .await
        .unwrap();
    assert_eq!(
        page.total, 0,
        "已删用户的名字不得命中,其公告不得出现(漏斗软删过滤)"
    );

    // 对照:公告本体确实在库中(该 notice 对无过滤读可见,
    // 证明 0 命中是软删过滤所致而非种子缺失)
    assert!(
        repo.select_by_id(&ctx, &1761800000000000005)
            .await
            .unwrap()
            .is_some(),
        "对照断言:该公告本体存在,0 命中必须是软删过滤造成"
    );
}
