//! 定向公告、已读回执与消息盒契约(Task 3 移植)。
#![cfg(feature = "test-util")]

use admin::testkit::{
    TestApp, assert_ok, super_admin, user_in_dept, user_with_perms,
};
use app_data::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use axum::http::StatusCode;
use serde_json::{Value, json};
use system::notice::repository::{
    notice_read_repo::{
        Column as ReadColumn, Entity as ReadEntity, Model as ReadModel,
    },
    notice_repo::{
        ActiveModel as NoticeActiveModel, Column as NoticeColumn,
        Entity as NoticeEntity,
    },
};

fn user_id_301() -> app_core::model::login_user::LoginUser {
    user_with_perms(&[])
}

fn user_id_777() -> app_core::model::login_user::LoginUser {
    user_in_dept(777, 101, &[])
}

async fn create_notice(
    app: &TestApp,
    title: &str,
    target_type: &str,
    user_ids: Option<&str>,
) -> i64 {
    let mut body = json!({
        "noticeTitle": title,
        "noticeType": "1",
        "noticeContent": "正文",
        "status": "0",
        "targetType": target_type,
    });
    if let Some(ids) = user_ids {
        body["userIds"] = json!(ids);
    }
    let (status, resp) =
        app.post("/system/notice", body, Some(&super_admin())).await;
    assert_ok(status, &resp);
    find_notice_id(app, title).await
}

async fn find_notice_id(app: &TestApp, title: &str) -> i64 {
    let ds = app.state.infra.data_source();
    let ctx = ds.system_context();
    ctx.select(NoticeEntity::find().filter(NoticeColumn::NoticeTitle.eq(title)))
        .without_row_scope()
        .one()
        .await
        .expect("查询公告失败")
        .unwrap_or_else(|| panic!("未找到公告: {title}"))
        .notice_id
}

async fn read_rows(app: &TestApp, notice_id: i64) -> Vec<ReadModel> {
    let ds = app.state.infra.data_source();
    let ctx = ds.system_context();
    ctx.select(ReadEntity::find().filter(ReadColumn::NoticeId.eq(notice_id)))
        .without_row_scope()
        .all()
        .await
        .expect("查询回执失败")
}

async fn my_rows(
    app: &TestApp,
    user: &app_core::model::login_user::LoginUser,
) -> Vec<Value> {
    let (status, body) = app
        .get("/system/notice/my?pageNum=1&pageSize=100", Some(user))
        .await;
    assert_ok(status, &body);
    body["data"]["rows"].as_array().cloned().unwrap_or_default()
}

fn id_i64(v: &Value) -> i64 {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or_default()
}

fn row_with_id(rows: &[Value], id: i64) -> Option<&Value> {
    rows.iter().find(|r| id_i64(&r["noticeId"]) == id)
}

#[tokio::test]
async fn published_targeted_notice_visible_only_to_target_users() {
    let app = TestApp::new().await;
    let id =
        create_notice(&app, "定向公告-仅301", "USER", Some("301,302")).await;

    // 草稿不可见
    assert!(
        row_with_id(&my_rows(&app, &user_id_301()).await, id).is_none(),
        "未发布公告不应对用户可见"
    );

    let (status, body) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    // 目标用户可见,且默认未读
    let rows = my_rows(&app, &user_id_301()).await;
    let row = row_with_id(&rows, id).expect("目标用户应看到定向公告");
    assert_eq!(row["readFlag"], "0");
    assert_eq!(row["starFlag"], "0");
    assert!(
        row_with_id(&my_rows(&app, &user_id_777()).await, id).is_none(),
        "非目标用户不应看到定向公告"
    );

    // 发布时为 USER 目标补建回执行
    let reads = read_rows(&app, id).await;
    assert_eq!(reads.len(), 2, "应为两个目标用户各建一条回执");
    assert!(reads.iter().all(|r| r.read_flag == "0"));
}

#[tokio::test]
async fn all_notice_visible_to_everyone_and_pushes_sse_broadcast() {
    let app = TestApp::new().await;
    let id = create_notice(&app, "全体公告", "ALL", None).await;

    let mut rx = app.push_hub.subscribe();
    let (status, body) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    let msg =
        tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("未收到公告 SSE 广播")
            .expect("SSE 通道关闭");
    assert!(msg.to_user_id.is_none(), "ALL 公告应广播给所有用户");
    assert_eq!(msg.msg_type, "notice");
    assert_eq!(id_i64(&msg.data.as_ref().unwrap()["noticeId"]), id);

    assert!(
        row_with_id(&my_rows(&app, &user_id_777()).await, id).is_some(),
        "ALL 公告应对任意登录用户可见"
    );
    // ALL 公告不预建回执行
    assert!(read_rows(&app, id).await.is_empty());
}

#[tokio::test]
async fn mark_read_is_idempotent_and_reflected_in_my_list() {
    let app = TestApp::new().await;
    let id = create_notice(&app, "已读幂等公告", "ALL", None).await;
    let (status, body) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    let user = user_id_301();
    for _ in 0..2 {
        let (status, body) = app
            .put(&format!("/system/notice/{id}/read"), json!({}), Some(&user))
            .await;
        assert_ok(status, &body);
    }

    let reads = read_rows(&app, id).await;
    assert_eq!(reads.len(), 1, "重复标记已读不得产生多行");
    assert_eq!(reads[0].read_flag, "1");
    assert!(reads[0].read_time.is_some());

    let rows = my_rows(&app, &user).await;
    let row = row_with_id(&rows, id).expect("公告应可见");
    assert_eq!(row["readFlag"], "1");
    assert!(row["readTime"].is_string(), "已读时间应回填: {row}");
}

#[tokio::test]
async fn read_all_marks_every_visible_notice() {
    let app = TestApp::new().await;
    let all_id = create_notice(&app, "全部已读-全体", "ALL", None).await;
    let target_id =
        create_notice(&app, "全部已读-定向", "USER", Some("301")).await;
    let foreign_id =
        create_notice(&app, "全部已读-他人", "USER", Some("777")).await;
    for id in [all_id, target_id, foreign_id] {
        let (status, body) = app
            .post(
                &format!("/system/notice/{id}/publish"),
                json!({}),
                Some(&super_admin()),
            )
            .await;
        assert_ok(status, &body);
    }

    let user = user_id_301();
    let (status, body) = app
        .put("/system/notice/readAll", json!({}), Some(&user))
        .await;
    assert_ok(status, &body);

    let read_user_ids: Vec<i64> = read_rows(&app, all_id)
        .await
        .iter()
        .map(|r| r.user_id)
        .collect();
    assert_eq!(read_user_ids, vec![301]);
    assert_eq!(read_rows(&app, target_id).await.len(), 1);
    assert!(
        read_rows(&app, foreign_id)
            .await
            .iter()
            .all(|r| r.user_id != 301),
        "全部已读不得越过可见性给他人公告建回执"
    );

    for id in [all_id, target_id] {
        let rows = my_rows(&app, &user).await;
        assert_eq!(row_with_id(&rows, id).unwrap()["readFlag"], "1");
    }
}

#[tokio::test]
async fn unpublished_and_foreign_notice_are_hidden_from_read_endpoints() {
    let app = TestApp::new().await;
    let draft_id = create_notice(&app, "草稿不可见", "ALL", None).await;
    let foreign_id = create_notice(&app, "他人定向", "USER", Some("777")).await;
    let (status, body) = app
        .post(
            &format!("/system/notice/{foreign_id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    let user = user_id_301();
    for id in [draft_id, foreign_id] {
        let (status, _) = app
            .put(&format!("/system/notice/{id}/read"), json!({}), Some(&user))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "不可见公告标记已读应 404");
    }
}

#[tokio::test]
async fn message_box_notice_list_carries_read_flag() {
    let app = TestApp::new().await;
    let id = create_notice(&app, "消息盒公告", "ALL", None).await;
    let (status, body) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    let user = user_id_301();
    let (status, body) = app.get("/resource/message/box", Some(&user)).await;
    assert_ok(status, &body);
    let data = &body["data"];
    for key in ["systemList", "noticeList", "workflowList"] {
        assert!(data[key].is_array(), "消息盒应含数组 {key}: {body}");
    }
    let notice = data["noticeList"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| id_i64(&n["messageId"]) == id)
        .expect("消息盒 noticeList 应含已发布公告");
    assert_eq!(notice["readFlag"], "0");
    assert_eq!(notice["category"], "notice");
    assert!(notice["path"].as_str().unwrap().contains("noticeId"));

    let (status, body) = app
        .put(&format!("/system/notice/{id}/read"), json!({}), Some(&user))
        .await;
    assert_ok(status, &body);
    let (status, body) = app.get("/resource/message/box", Some(&user)).await;
    assert_ok(status, &body);
    let notice = body["data"]["noticeList"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| id_i64(&n["messageId"]) == id)
        .unwrap();
    assert_eq!(notice["readFlag"], "1");
}

#[tokio::test]
async fn publish_requires_edit_permission() {
    let app = TestApp::new().await;
    let id = create_notice(&app, "权限公告", "ALL", None).await;
    let list_only = user_with_perms(&["system:notice:list"]);
    let (status, _) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&list_only),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// 重复发布拒绝:已发布公告再 publish 应 400(不重置 send_time、不重复广播)。
#[tokio::test]
async fn republish_published_notice_rejected() {
    let app = TestApp::new().await;
    let id = create_notice(&app, "重复发布公告", "ALL", None).await;
    let (status, body) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    let (status, body) = app
        .post(
            &format!("/system/notice/{id}/publish"),
            json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "重复发布应 400: {body}");
    assert!(
        body["msg"].as_str().unwrap_or_default().contains("已发布"),
        "{body}"
    );
}

/// orderByColumn=sendTime 必须真正按 send_time 排序。parse_order 曾缺
/// send_time 分支,落到 notice_id,页面"发送时间"排序因此无效。
#[tokio::test]
async fn my_list_orders_by_send_time_when_requested() {
    use app_data::sea_orm::ActiveValue::Set;
    use chrono::{TimeZone, Utc};

    let app = TestApp::new().await;
    let id_a = create_notice(&app, "排序-早", "ALL", None).await;
    let id_b = create_notice(&app, "排序-晚", "ALL", None).await;
    for id in [id_a, id_b] {
        let (status, body) = app
            .post(
                &format!("/system/notice/{id}/publish"),
                json!({}),
                Some(&super_admin()),
            )
            .await;
        assert_ok(status, &body);
    }

    // 直接钉死 send_time,避免两次发布同刻导致次序不确定。
    let ds = app.state.infra.data_source();
    let ctx = ds.system_context();
    let t_a = Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).unwrap();
    let t_b = Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap();
    for (id, t) in [(id_a, t_a), (id_b, t_b)] {
        ctx.update_without_row_scope(NoticeActiveModel {
            notice_id: Set(id),
            send_time: Set(Some(t)),
            ..Default::default()
        })
        .await
        .expect("钉 send_time");
    }

    let rows = my_rows_ordered(&app, &user_id_301(), "sendTime", "asc").await;
    let ids: Vec<i64> = rows.iter().map(id_i64_of_notice).collect();
    assert_eq!(ids, vec![id_a, id_b], "sendTime asc 应早→晚: {rows:?}");

    let rows = my_rows_ordered(&app, &user_id_301(), "sendTime", "desc").await;
    let ids: Vec<i64> = rows.iter().map(id_i64_of_notice).collect();
    assert_eq!(ids, vec![id_b, id_a], "sendTime desc 应晚→早: {rows:?}");
}

async fn my_rows_ordered(
    app: &TestApp,
    user: &app_core::model::login_user::LoginUser,
    column: &str,
    dir: &str,
) -> Vec<Value> {
    let (status, body) = app
        .get(
            &format!(
                "/system/notice/my?pageNum=1&pageSize=100\
                 &orderByColumn={column}&isAsc={dir}"
            ),
            Some(user),
        )
        .await;
    assert_ok(status, &body);
    body["data"]["rows"].as_array().cloned().unwrap_or_default()
}

fn id_i64_of_notice(v: &Value) -> i64 {
    id_i64(&v["noticeId"])
}
