//! 菜单级联删除回归测试。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_menu_cascade_test`
//!
//! 历史缺陷(porting bug):`delete_menu_cascade` 对传入的每个 id 调
//! `menu_repo.has_child()`,丢失 Java 原版 `.in(parentId, ids).notIn(menuId, ids)`
//! 的排除语义(SysMenuServiceImpl.java:268-273)——子节点同批在删除集合内不应
//! 算阻碍。结果:任何非叶子节点即使父+子孙同批传入也被 400
//! 「存在子菜单,不允许删除」拒绝,cascade 端点形同虚设。
//! 本文件锁定修复:父+子同批 cascade 删除必须成功且两者均消失。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, code, super_admin};
use app_core::model::login_user::LoginUser;
use serde_json::{Value, json};

/// 读取被 `stringify_ids` 序列化为字符串的 ID。
/// ID 字段出站恒为字符串(`core/src/vo/response.rs`),仍兼容数字形态以防回退。
fn as_id(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// 父+子同批传入 cascade 删除应成功,且父子均从列表消失。
#[tokio::test]
async fn menu_cascade_delete_parent_and_child_together() {
    let app = TestApp::new().await;
    let admin = super_admin();

    // 1. 建父菜单(目录)与挂在它下面的子菜单。
    //    body 需含 isFrame/isCache 字符串键。
    let parent_payload = json!({
        "menuName": "级联删除父目录", "parentId": 0, "orderNum": 99,
        "path": "cascade-demo", "menuType": "M", "isFrame": "1",
        "isCache": "1", "visible": "0", "status": "0"
    });
    let (sp, bp) = app.post("/system/menu", parent_payload, Some(&admin)).await;
    assert_ok(sp, &bp);
    let parent_id = as_id(bp.get("data").unwrap_or(&Value::Null))
        .expect("POST 父菜单应返回 menuId");

    let child_payload = json!({
        "menuName": "级联删除子菜单", "parentId": parent_id, "orderNum": 1,
        "path": "cascade-child", "menuType": "C", "isFrame": "1",
        "isCache": "1", "visible": "0", "status": "0"
    });
    let (sc, bc) = app.post("/system/menu", child_payload, Some(&admin)).await;
    assert_ok(sc, &bc);
    let child_id = as_id(bc.get("data").unwrap_or(&Value::Null))
        .expect("POST 子菜单应返回 menuId");

    // 2. 父+子同批 cascade 删除 → 必须成功(修复前 400 存在子菜单)。
    let (sd, bd) = app
        .delete(
            &format!("/system/menu/cascade/{parent_id},{child_id}"),
            Some(&admin),
        )
        .await;
    assert_ok(sd, &bd);

    // 3. 列表中断言父子均已删除。
    let (sl, bl) = app.get("/system/menu/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    let remaining: Vec<i64> = bl
        .pointer("/data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("menuId"))
                .filter_map(as_id)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !remaining.contains(&parent_id),
        "父菜单 {parent_id} 应已删除: {bl}"
    );
    assert!(
        !remaining.contains(&child_id),
        "子菜单 {child_id} 应已删除: {bl}"
    );
}

/// 排除语义的反向边界:只传父 id(子孙不在集合内)仍应被拒。
/// 对齐 Java hasChildByMenuId 语义,防止修复被误改成「不检查子节点」。
#[tokio::test]
async fn menu_cascade_delete_parent_alone_still_rejected() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let parent_payload = json!({
        "menuName": "孤立删除父目录", "parentId": 0, "orderNum": 98,
        "path": "orphan-demo", "menuType": "M", "isFrame": "1",
        "isCache": "1", "visible": "0", "status": "0"
    });
    let (sp, bp) = app.post("/system/menu", parent_payload, Some(&admin)).await;
    assert_ok(sp, &bp);
    let parent_id = as_id(bp.get("data").unwrap_or(&Value::Null))
        .expect("POST 父菜单应返回 menuId");

    let child_payload = json!({
        "menuName": "孤立删除子菜单", "parentId": parent_id, "orderNum": 1,
        "path": "orphan-child", "menuType": "C", "isFrame": "1",
        "isCache": "1", "visible": "0", "status": "0"
    });
    let (sc, bc) = app.post("/system/menu", child_payload, Some(&admin)).await;
    assert_ok(sc, &bc);

    // 只传父 id:子节点不在删除集合内 → 应 400「存在子菜单,不允许删除」。
    let (sd, bd) = app
        .delete(&format!("/system/menu/cascade/{parent_id}"), Some(&admin))
        .await;
    assert_eq!(
        code(&bd),
        400,
        "子孙不在集合内时级联删除应被拒: status={sd} body={bd}"
    );
}

/// 创建一个父目录及其子菜单,返回 (parent_id, child_id)。
async fn seed_menu_parent_child(
    app: &TestApp,
    admin: &LoginUser,
) -> (i64, i64) {
    let parent_payload = json!({
        "menuName": "事务回滚父目录", "parentId": 0, "orderNum": 97,
        "path": "tx-rollback-parent", "menuType": "M", "isFrame": "1",
        "isCache": "1", "visible": "0", "status": "0"
    });
    let (sp, bp) = app.post("/system/menu", parent_payload, Some(admin)).await;
    assert_ok(sp, &bp);
    let parent_id = as_id(bp.get("data").unwrap_or(&Value::Null))
        .expect("POST 父菜单应返回 menuId");

    let child_payload = json!({
        "menuName": "事务回滚子菜单", "parentId": parent_id, "orderNum": 1,
        "path": "tx-rollback-child", "menuType": "C", "isFrame": "1",
        "isCache": "1", "visible": "0", "status": "0"
    });
    let (sc, bc) = app.post("/system/menu", child_payload, Some(admin)).await;
    assert_ok(sc, &bc);
    let child_id = as_id(bc.get("data").unwrap_or(&Value::Null))
        .expect("POST 子菜单应返回 menuId");

    (parent_id, child_id)
}

/// 级联删除中途失败后,此前操作必须整体回滚,不能留下半删的菜单树。
#[tokio::test]
async fn cascade_delete_rolls_back_on_partial_failure() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (parent_id, child_id) = seed_menu_parent_child(&app, &admin).await;

    // 混入一个不存在的 id,使删除序列中途失败。
    let ids = format!("{parent_id},{child_id},99999999");
    let (sd, _bd) = app
        .delete(&format!("/system/menu/cascade/{ids}"), Some(&admin))
        .await;
    assert!(
        sd.is_client_error() || sd.is_server_error(),
        "含不存在 id 的级联删除应失败: status={sd}"
    );

    // 关键:失败后 parent/child 必须都还在(未被半删)。
    let (sl, bl) = app.get("/system/menu/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    let body = bl.to_string();
    assert!(
        body.contains(&parent_id.to_string()),
        "事务未回滚: parent 已被删除"
    );
    assert!(
        body.contains(&child_id.to_string()),
        "事务未回滚: child 已被删除"
    );
}
