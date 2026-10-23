mod common;

use std::sync::Arc;

use app_data::sea_orm::ColumnTrait;
use common::setup_db;
use system::{
    post::repository::post_repo::PostRepository,
    relation::{user_post, user_role},
    role::repository::role_repo::RoleRepository,
    user::repository::user_repo::UserRepository,
};

#[tokio::test]
async fn test_insert_and_delete_user_roles() {
    let ds = Arc::new(setup_db().await);
    // 仓储无状态,ctx 一律经测试自有 ds 的 system_context 构造。
    let repo = UserRepository::new();
    // select_roles_by_user_id 是豁免方法
    // (Mapper:89 无注解),经 system_context 过桥。
    let role_repo = RoleRepository::new();
    let ctx = ds.system_context();
    let role_ctx = ds.system_context();

    // 用户 307 无角色关联 (seed 中无记录)
    let roles = role_repo
        .select_roles_by_user_id(&role_ctx, 307)
        .await
        .unwrap();
    assert!(roles.is_empty(), "用户 307 初始应无角色");

    // 插入角色 200, 201(关联表 DataPolicy 全 None → 豁免,ctx.insert 逐行)
    repo.insert_user_roles(&ctx, 307, &[200, 201])
        .await
        .unwrap();
    let roles = role_repo
        .select_roles_by_user_id(&role_ctx, 307)
        .await
        .unwrap();
    assert_eq!(roles.len(), 2, "插入后应有 2 个角色关联");

    // 删除所有角色:经漏斗的豁免实体批量删原语
    // delete_where(user_role 复合主键 + 全 None 豁免,恰在其面内)。
    use app_data::sea_orm::Condition;
    ctx.delete_where::<user_role::Entity>(
        Condition::all().add(user_role::Column::UserId.eq(307)),
    )
    .await
    .unwrap();
    let roles = role_repo
        .select_roles_by_user_id(&role_ctx, 307)
        .await
        .unwrap();
    assert!(roles.is_empty(), "删除后应无角色关联");

    // 重新插入单个角色
    repo.insert_user_roles(&ctx, 307, &[205]).await.unwrap();
    let roles = role_repo
        .select_roles_by_user_id(&role_ctx, 307)
        .await
        .unwrap();
    assert_eq!(roles.len(), 1, "重新插入后应有 1 个角色");
    assert_eq!(roles[0].role_key, "role_dept_self");
}

#[tokio::test]
async fn test_insert_and_delete_user_posts() {
    let ds = Arc::new(setup_db().await);
    // 同上,ctx 经测试自有 ds 的 system_context 构造。
    let repo = UserRepository::new();
    // 本用例只测 user_post 关联(Java 豁免路径),
    // 经 system_context 过桥。
    let post_repo = PostRepository::new();
    let ctx = ds.system_context();
    let post_ctx = ds.system_context();

    // 用户 301 无岗位关联 (seed 中无记录)
    let posts = post_repo.select_by_user_id(&post_ctx, 301).await.unwrap();
    assert!(posts.is_empty(), "用户 301 初始应无岗位");

    // 插入岗位(关联表豁免,ctx.insert 逐行)
    repo.insert_user_posts(&ctx, 301, &[1761200000000000001])
        .await
        .unwrap();
    let posts = post_repo.select_by_user_id(&post_ctx, 301).await.unwrap();
    assert_eq!(posts.len(), 1, "插入后应有 1 个岗位");

    // 删除所有岗位(同上:经漏斗 delete_where)
    use app_data::sea_orm::Condition;
    ctx.delete_where::<user_post::Entity>(
        Condition::all().add(user_post::Column::UserId.eq(301)),
    )
    .await
    .unwrap();
    let posts = post_repo.select_by_user_id(&post_ctx, 301).await.unwrap();
    assert!(posts.is_empty(), "删除后应无岗位");

    // 重新插入两个岗位
    repo.insert_user_posts(
        &ctx,
        301,
        &[1761200000000000001, 1761200000000000002],
    )
    .await
    .unwrap();
    let posts = post_repo.select_by_user_id(&post_ctx, 301).await.unwrap();
    assert_eq!(posts.len(), 2, "重新插入后应有 2 个岗位");
}

#[tokio::test]
async fn test_count_users_by_post_id() {
    let ds = Arc::new(setup_db().await);
    let repo = UserRepository::new();
    let ctx = ds.system_context();

    // 岗位 ceo 关联用户 302 (seed)
    let count = repo
        .count_users_by_post_id(&ctx, 1761200000000000001)
        .await
        .unwrap();
    assert_eq!(count, 1, "岗位 ceo 应关联 1 个用户 (302)");

    // 岗位 pm 也关联用户 302 (seed)
    let count = repo
        .count_users_by_post_id(&ctx, 1761200000000000002)
        .await
        .unwrap();
    assert_eq!(count, 1, "岗位 pm 应关联 1 个用户 (302)");

    // 不存在的岗位
    let count = repo.count_users_by_post_id(&ctx, 99999).await.unwrap();
    assert_eq!(count, 0, "不存在的岗位应返回 0");
}

#[tokio::test]
async fn test_post_select_by_user_id() {
    let ds = Arc::new(setup_db().await);
    // 同上,经 system_context 过桥(ctx 经测试自有 ds 构造)
    let repo = PostRepository::new();
    let ctx = ds.system_context();

    // 用户 302 关联 ceo 和 pm (seed)
    let posts = repo.select_by_user_id(&ctx, 302).await.unwrap();
    assert_eq!(posts.len(), 2, "用户 302 应关联 2 个岗位");
    assert_eq!(
        posts[0].post_code, "ceo",
        "第一个岗位应为 ceo (post_sort=1)"
    );
    assert_eq!(posts[1].post_code, "pm", "第二个岗位应为 pm (post_sort=2)");

    // 用户 301 无岗位关联
    let posts = repo.select_by_user_id(&ctx, 301).await.unwrap();
    assert!(posts.is_empty(), "用户 301 无岗位关联");
}
