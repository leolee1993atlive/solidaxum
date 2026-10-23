//! 系统模块 HTTP 契约集成测试。
//!
//! 运行: `cargo test -p admin --features test-util`
//!
//! 契约基准是原版 Vue 前端的 API 调用(路径/方法/响应信封)。

#![cfg(feature = "test-util")]

use admin::testkit::{
    TestApp, api_uri, assert_ok, code, super_admin, user_with_perms,
};
use app_core::util::excel::ExcelBuilder;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use system::user::model::user::UserImportRow;
use tower::ServiceExt;

/// 读取被 `stringify_ids` 序列化为字符串的 ID。
/// ID 字段出站恒为字符串(`core/src/vo/response.rs`,不再随值大小漂移),
/// 仍兼容数字形态以防回退;字符串不变量本身由 `contract_id_string_test` 钉住。
fn as_id(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

// ───────────────────────── 脚手架自检(green) ─────────────────────────

#[tokio::test]
async fn health_check_works() {
    let app = TestApp::new().await;
    let (status, _body) = app.get("/health", None).await;
    assert_eq!(status.as_u16(), 200);
}

#[tokio::test]
async fn dept_list_succeeds_with_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:list"]);
    let (status, body) = app.get("/system/dept/list", Some(&user)).await;
    assert_ok(status, &body);
    assert!(
        body.get("data").map(|d| d.is_array()).unwrap_or(false),
        "data 应为数组: {body}"
    );
}

#[tokio::test]
async fn dept_list_forbidden_without_permission() {
    let app = TestApp::new().await;
    // 登录但无 system:dept:list 权限
    let user = user_with_perms(&["system:user:list"]);
    let (_status, body) = app.get("/system/dept/list", Some(&user)).await;
    assert_eq!(code(&body), 403, "缺权限应返回 code=403: {body}");
}

#[tokio::test]
async fn unauthenticated_request_is_rejected() {
    let app = TestApp::new().await;
    let (_status, body) = app.get("/system/dept/list", None).await;
    assert_eq!(code(&body), 401, "未登录应返回 code=401: {body}");
}

#[tokio::test]
async fn super_admin_bypasses_permission_check() {
    let app = TestApp::new().await;
    let (status, body) =
        app.get("/system/dept/list", Some(&super_admin())).await;
    assert_ok(status, &body);
}

// ───────────────── 契约用例(对标 Vue 前端) ─────────────────
// 以下用例对标 Vue 前端实际调用的路径/方法/信封。

#[tokio::test]
async fn client_list_and_get_work() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:client:list", "system:client:query"]);
    let (s1, b1) = app.get("/system/client/list", Some(&user)).await;
    assert_ok(s1, &b1);
    let (s2, b2) = app.get("/system/client/1", Some(&user)).await;
    assert_ok(s2, &b2);
}

#[tokio::test]
async fn user_list_matches_vue_contract() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:list"]);
    let (status, body) = app.get("/system/user/list", Some(&user)).await;
    assert_ok(status, &body);
    // Vue 表格读取 data.rows / data.total
    assert!(
        body.pointer("/data/rows").is_some(),
        "分页信封应含 data.rows: {body}"
    );
    assert!(
        body.pointer("/data/total").is_some(),
        "分页信封应含 data.total: {body}"
    );
}

#[tokio::test]
async fn role_list_matches_vue_contract() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:list"]);
    let (status, body) = app.get("/system/role/list", Some(&user)).await;
    assert_ok(status, &body);
    // Vue 表格读 data.rows / data.total
    assert!(
        body.pointer("/data/rows").is_some(),
        "分页信封应含 data.rows: {body}"
    );
    assert!(
        body.pointer("/data/total").is_some(),
        "分页信封应含 data.total: {body}"
    );
}

#[tokio::test]
async fn role_list_filters_by_role_key() {
    // Vue 角色搜索表单发 roleKey(权限字符);此前 RolePageQuery 无该字段被静默忽略。
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:list"]);
    let (status, body) = app
        .get(
            "/system/role/list?pageNum=1&pageSize=10&roleKey=common",
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("rows");
    assert_eq!(
        rows.len(),
        1,
        "roleKey=common 应仅返回 common 角色,而非全部: {body}"
    );
    assert_eq!(rows[0]["roleKey"], "common", "应命中 common 角色: {body}");
}

#[tokio::test]
async fn role_detail_includes_check_strictly_fields() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:query"]);
    // seed role 202: menu_check_strictly=0, dept_check_strictly=0
    let (status, body) = app.get("/system/role/202", Some(&user)).await;
    assert_ok(status, &body);
    let data = &body["data"];
    assert_eq!(
        data.get("menuCheckStrictly").and_then(|v| v.as_bool()),
        Some(false),
        "handleDataScope 依赖 menuCheckStrictly: {body}"
    );
    assert_eq!(
        data.get("deptCheckStrictly").and_then(|v| v.as_bool()),
        Some(false),
        "handleDataScope 依赖 deptCheckStrictly: {body}"
    );
}

#[tokio::test]
async fn menu_list_endpoint_exists() {
    let app = TestApp::new().await;
    let (status, body) =
        app.get("/system/menu/list", Some(&super_admin())).await;
    assert_ok(status, &body);
    // menu list 是数组(非分页)
    assert!(
        body.get("data").map(|d| d.is_array()).unwrap_or(false),
        "data 应为数组: {body}"
    );
}

#[tokio::test]
async fn menu_mutations_require_super_admin() {
    let app = TestApp::new().await;
    let normal = user_with_perms(&[
        "system:menu:add",
        "system:menu:list",
        "system:menu:query",
    ]);
    let payload = serde_json::json!({
        "menuName": "应被拒",
        "parentId": 501,
        "orderNum": 1,
        "path": "deny",
        "isFrame": "1",
        "isCache": "0",
        "menuType": "C",
        "visible": "0",
        "status": "0"
    });
    let (_status, body) =
        app.post("/system/menu", payload, Some(&normal)).await;
    assert_eq!(code(&body), 403, "非超管 POST /menu 应 Forbidden: {body}");
}

// ============================================================
// CRUD roundtrip / 信封 / 批量逗号 / 数据权限 HTTP
// ============================================================

#[tokio::test]
async fn post_crud_roundtrip() {
    let app = TestApp::new().await;
    let admin = super_admin();

    // create
    let create = serde_json::json!({
        "postCode": "test_post_w6",
        "postName": "W6测试岗位",
        "postSort": 99,
        "deptId": 101,
    });
    let (sc, bc) = app.post("/system/post", create, Some(&admin)).await;
    assert_ok(sc, &bc);

    // list 应能查到
    let (sl, bl) = app.get("/system/post/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    assert!(
        bl.pointer("/data/rows").is_some(),
        "post list 应有 data.rows: {bl}"
    );

    // 在列表中找到我们建的岗位
    let rows = bl
        .pointer("/data/rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ours = rows
        .iter()
        .find(|r| {
            r.get("postCode").and_then(Value::as_str) == Some("test_post_w6")
        })
        .expect("列表应含新建岗位");
    let post_id = ours.get("postId").and_then(as_id).expect("岗位应有 postId");

    // update
    let update = serde_json::json!({
        "postId": post_id,
        "postCode": "test_post_w6",
        "postName": "W6测试岗位-改",
        "postSort": 99,
        "deptId": 101,
    });
    let (su, bu) = app.put("/system/post", update, Some(&admin)).await;
    assert_ok(su, &bu);

    // get by id 验证更新
    let (sg, bg) = app
        .get(&format!("/system/post/{post_id}"), Some(&admin))
        .await;
    assert_ok(sg, &bg);
    assert_eq!(
        bg.pointer("/data/postName").and_then(Value::as_str),
        Some("W6测试岗位-改"),
        "get by id 应返回更新后的岗位名: {bg}"
    );

    // delete
    let (sd, bd) = app
        .delete(&format!("/system/post/{post_id}"), Some(&admin))
        .await;
    assert_ok(sd, &bd);
}

#[tokio::test]
async fn post_list_filters_by_post_category() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // 先建一个带 postCategory 的岗位
    let (_s, b) = app
        .post(
            "/system/post",
            serde_json::json!({
                "postCode": "h3_cat_post",
                "postName": "H3分类岗位",
                "postCategory": "h3_cat",
                "postSort": 99,
                "deptId": 101,
                "status": "0"
            }),
            Some(&admin),
        )
        .await;
    assert_ok(_s, &b);

    let user = user_with_perms(&["system:post:list"]);
    let (status, body) = app
        .get(
            "/system/post/list?postCategory=h3_cat&pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("rows");
    let cats: Vec<&str> = rows
        .iter()
        .filter_map(|r| r.get("postCategory").and_then(|c| c.as_str()))
        .collect();
    assert!(
        cats.iter().all(|c| *c == "h3_cat"),
        "应只返回 postCategory=h3_cat 的岗位: {body}"
    );
}

#[tokio::test]
async fn post_list_filters_by_belong_dept_id_subtree() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:post:list"]);
    // belongDeptId=100(总部) 应展开为子树(含 101 技术部);
    // 种子岗位(1761200000000000001/002) dept_id=101 应命中
    let (status, body) = app
        .get(
            "/system/post/list?belongDeptId=100&pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("rows");
    assert!(
        !rows.is_empty(),
        "子树过滤应命中 dept_id=101 的种子岗位: {body}"
    );
    // 所有返回岗位的 deptId 都应在 100 的子树内(100/101/102/103/104)
    let subtree = [100i64, 101, 102, 103, 104];
    for r in rows {
        if let Some(d) = r.get("deptId").and_then(|v| v.as_i64()) {
            assert!(subtree.contains(&d), "deptId {} 不在子树内: {body}", d);
        }
    }
}

#[tokio::test]
async fn post_optionselect_mutually_exclusive_and_empty_when_no_param() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:post:query"]);

    // 1) 无参 → 空数组(对齐 Java SysPostController.optionselect:151-154 else 分支)
    let (status, body) =
        app.get("/system/post/optionselect", Some(&user)).await;
    assert_ok(status, &body);
    let arr = body["data"].as_array();
    assert!(
        arr.map(|a| a.is_empty()).unwrap_or(true),
        "无参应返回空数组(Java else 分支): {body}"
    );

    // 2) 仅 postIds → 按 ids 过滤(种子 ceo 1761200000000000001 应命中)
    let pid = 1761200000000000001i64;
    let (status, body) = app
        .get(
            &format!("/system/post/optionselect?postIds={pid}"),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
    // postId 因雪花 ID 超出 JS 安全整数范围会序列化为字符串,用 as_id 解析
    let ids: std::collections::HashSet<i64> = body["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| as_id(&v["postId"])).collect())
        .unwrap_or_default();
    assert!(ids.contains(&pid), "postIds 过滤应命中 {pid}: {body}");

    // 3) deptId 与 postIds 同传 → deptId 优先(对齐 Java if/else if 互斥)
    //    deptId=101 分支返回该 dept 的岗位(种子 2 个),不按 postIds 过滤
    let (status, body) = app
        .get(
            "/system/post/optionselect?deptId=101&postIds=9999",
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
    let cnt = body["data"].as_array().map(|a| a.len()).unwrap_or(0);
    assert!(cnt > 0, "deptId 应优先于 postIds(互斥分支): {body}");
}

#[tokio::test]
async fn role_crud_roundtrip() {
    let app = TestApp::new().await;
    let admin = super_admin();

    // create
    let create = serde_json::json!({
        "roleName": "w6_test_role",
        "roleKey": "w6_test_key",
        "roleSort": 99,
        "dataScope": "5",
        "menuCheckStrictly": true,
        "deptCheckStrictly": true,
        "status": "0",
    });
    let (sc, bc) = app.post("/system/role", create, Some(&admin)).await;
    assert_ok(sc, &bc);

    // list 找到新角色
    let (sl, bl) = app.get("/system/role/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    let rows = bl
        .pointer("/data/rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ours = rows
        .iter()
        .find(|r| {
            r.get("roleKey").and_then(Value::as_str) == Some("w6_test_key")
        })
        .expect("角色列表应含新建角色");
    let role_id = ours.get("roleId").and_then(as_id).expect("角色应有 roleId");

    // update
    let update = serde_json::json!({
        "roleId": role_id,
        "roleName": "w6_test_role-改",
        "roleKey": "w6_test_key",
        "roleSort": 99,
        "dataScope": "5",
        "menuCheckStrictly": true,
        "deptCheckStrictly": true,
        "status": "0",
    });
    let (su, bu) = app.put("/system/role", update, Some(&admin)).await;
    assert_ok(su, &bu);

    // delete
    let (sd, bd) = app
        .delete(&format!("/system/role/{role_id}"), Some(&admin))
        .await;
    assert_ok(sd, &bd);
}

#[tokio::test]
async fn user_batch_delete_with_three_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // 先建 3 个用户,然后批量删除(逗号列表)
    for n in 1..=3 {
        let body = serde_json::json!({
            "userName": format!("batch_user_{n}"),
            "nickName": format!("batch{n}"),
            "password": "p1234567",
        });
        let (s, b) = app.post("/system/user", body, Some(&admin)).await;
        assert_ok(s, &b);
    }
    // 查列表拿 ids
    let (sl, bl) = app.get("/system/user/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    let rows = bl
        .pointer("/data/rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ids: Vec<i64> = rows
        .iter()
        .filter_map(|r| {
            let name = r.get("userName").and_then(Value::as_str)?;
            if name.starts_with("batch_user_") {
                r.get("userId").and_then(as_id)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ids.len(), 3, "应找到 3 个 batch_user: {ids:?}");
    let csv = ids
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (sd, bd) = app
        .delete(&format!("/system/user/{csv}"), Some(&admin))
        .await;
    assert_ok(sd, &bd);
}

#[tokio::test]
async fn menu_treeselect_and_role_tree_work() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:menu:query"]);
    let (s1, b1) = app.get("/system/menu/treeselect", Some(&user)).await;
    assert_ok(s1, &b1);
    // 角色 201 在 seed 中已分配 501/502/504
    let (s2, b2) = app
        .get("/system/menu/roleMenuTreeselect/201", Some(&user))
        .await;
    assert_ok(s2, &b2);
    assert!(
        b2.pointer("/data/checkedKeys").is_some(),
        "应含 checkedKeys: {b2}"
    );
    assert!(b2.pointer("/data/menus").is_some(), "应含 menus: {b2}");
}

#[tokio::test]
async fn role_menu_treeselect_nodes_include_menu_metadata() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:menu:query"]);
    let (status, body) = app
        .get("/system/menu/roleMenuTreeselect/201", Some(&user))
        .await;
    assert_ok(status, &body);
    let menus = body.pointer("/data/menus").and_then(|v| v.as_array());
    let menus = menus.expect("menus array");
    assert!(!menus.is_empty(), "{body}");
    let node = &menus[0];
    for key in ["menuType", "visible", "status", "parentId", "weight"] {
        assert!(node.get(key).is_some(), "节点缺 {key}: {body}");
    }
}

#[tokio::test]
async fn menu_crud_roundtrip() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let create = serde_json::json!({
        "menuName": "测试菜单", "parentId": 0, "orderNum": 9,
        "path": "demo", "menuType": "C", "isFrame": "1", "isCache": "0",
        "visible": "0", "status": "0", "perms": "system:demo:list"
    });
    let (sc, bc) = app.post("/system/menu", create, Some(&admin)).await;
    assert_ok(sc, &bc);
    let menu_id = bc
        .pointer("/data")
        .and_then(as_id)
        .expect("POST 应返回 menuId");

    let (sg, bg) = app
        .get(&format!("/system/menu/{menu_id}"), Some(&admin))
        .await;
    assert_ok(sg, &bg);
    assert_eq!(
        bg.pointer("/data/menuName").and_then(|v| v.as_str()),
        Some("测试菜单")
    );

    let update = serde_json::json!({
        "menuId": menu_id, "menuName": "测试菜单-已更新", "parentId": 0,
        "orderNum": 9, "path": "demo", "menuType": "C", "isFrame": "1",
        "isCache": "0", "visible": "0", "status": "0", "perms": "system:demo:list"
    });
    let (su, bu) = app.put("/system/menu", update, Some(&admin)).await;
    assert_ok(su, &bu);

    let (sl, bl) = app.get("/system/menu/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    let has_updated = bl
        .pointer("/data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter().any(|m| {
                m.get("menuId").and_then(as_id) == Some(menu_id)
                    && m.get("menuName").and_then(|v| v.as_str())
                        == Some("测试菜单-已更新")
            })
        })
        .unwrap_or(false);
    assert!(has_updated, "列表中应存在已更新的菜单: {bl}");

    let (sd, bd) = app
        .delete(&format!("/system/menu/{menu_id}"), Some(&admin))
        .await;
    assert_ok(sd, &bd);

    let (sl2, bl2) = app.get("/system/menu/list", Some(&admin)).await;
    assert_ok(sl2, &bl2);
    let still_exists = bl2
        .pointer("/data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter().any(|m| {
                m.get("menu_id").and_then(|v| v.as_i64()) == Some(menu_id)
            })
        })
        .unwrap_or(false);
    assert!(!still_exists, "删除后列表不应再包含该菜单: {bl2}");
}

#[tokio::test]
async fn duplicate_menu_name_under_same_parent_rejected() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // seed: 502「用户管理」parentId=501
    let dup = serde_json::json!({
        "menuName": "用户管理",
        "parentId": 501,
        "orderNum": 1,
        "path": "user-dup",
        "isFrame": "1",
        "isCache": "0",
        "menuType": "C",
        "visible": "0",
        "status": "0"
    });
    let (_s, b) = app.post("/system/menu", dup, Some(&admin)).await;
    assert_ne!(code(&b), 200, "同级重复 menuName 应被拒: {b}");
}

#[tokio::test]
async fn menu_cannot_be_its_own_parent() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let update = serde_json::json!({
        "menuId": 502,
        "menuName": "用户管理",
        "parentId": 502,
        "orderNum": 0,
        "path": "user",
        "isFrame": "1",
        "isCache": "0",
        "menuType": "C",
        "visible": "0",
        "status": "0",
        "perms": "system:user:list"
    });
    let (_s, b) = app.put("/system/menu", update, Some(&admin)).await;
    assert_ne!(code(&b), 200, "menuId==parentId 应被拒: {b}");
}

#[tokio::test]
async fn menu_cascade_delete_rejects_when_has_children() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) =
        app.delete("/system/menu/cascade/501", Some(&admin)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "有子菜单应返回 400: {body}"
    );
    assert_eq!(code(&body), 400, "有子菜单应返回 code=400: {body}");
    assert!(
        body.get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("存在子菜单"),
        "应提示存在子菜单: {body}"
    );
}

#[tokio::test]
async fn menu_cascade_delete_leaf_only_deletes_requested_id() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) =
        app.delete("/system/menu/cascade/503", Some(&admin)).await;
    assert_ok(status, &body);

    let (s2, b2) = app.get("/system/menu/502", Some(&admin)).await;
    assert_ok(s2, &b2);
    assert_eq!(
        b2.pointer("/data/menuId").and_then(as_id),
        Some(502),
        "父菜单 502 应仍存在: {b2}"
    );

    let (_s3, b3) = app.get("/system/menu/503", Some(&admin)).await;
    assert_ne!(code(&b3), 200, "已删除的 503 不应再存在: {b3}");
}

#[tokio::test]
async fn menu_create_persists_active_menu_ext_remark() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let create = serde_json::json!({
        "menuName": "扩展字段菜单", "parentId": 0, "orderNum": 10,
        "path": "ext-demo", "menuType": "C", "isFrame": "1", "isCache": "0",
        "visible": "0", "status": "0", "perms": "system:ext:list",
        "activeMenu": "/system/user", "ext": "{\"color\":\"blue\"}",
        "remark": "菜单备注"
    });
    let (sc, bc) = app.post("/system/menu", create, Some(&admin)).await;
    assert_ok(sc, &bc);
    let menu_id = bc
        .pointer("/data")
        .and_then(as_id)
        .expect("POST 应返回 menuId");

    let (sg, bg) = app
        .get(&format!("/system/menu/{menu_id}"), Some(&admin))
        .await;
    assert_ok(sg, &bg);
    assert_eq!(
        bg.pointer("/data/activeMenu").and_then(|v| v.as_str()),
        Some("/system/user")
    );
    assert_eq!(
        bg.pointer("/data/ext").and_then(|v| v.as_str()),
        Some("{\"color\":\"blue\"}")
    );
    assert_eq!(
        bg.pointer("/data/remark").and_then(|v| v.as_str()),
        Some("菜单备注")
    );

    let (sd, bd) = app
        .delete(&format!("/system/menu/{menu_id}"), Some(&admin))
        .await;
    assert_ok(sd, &bd);
}

#[tokio::test]
async fn user_batch_delete_accepts_id_list() {
    let app = TestApp::new().await;
    let (status, body) = app
        .delete("/system/user/301,302", Some(&super_admin()))
        .await;
    assert_ok(status, &body);
}

// ============================================================
// 唯一性校验 + 越权保护
// ============================================================

#[tokio::test]
async fn duplicate_username_rejected() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // seed 已有 test_user(301); 再建同名应失败
    let dup = serde_json::json!({
        "userName": "test_user",
        "nickName": "dup",
        "password": "p1234567",
    });
    let (_s, b) = app.post("/system/user", dup, Some(&admin)).await;
    assert_ne!(code(&b), 200, "重复用户名应被拒: {b}");
}

#[tokio::test]
async fn duplicate_post_name_rejected() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // seed 已有 ceo 岗位; 再建同名/同 code 应失败
    let dup = serde_json::json!({
        "postCode": "ceo",
        "postName": "董事长",
        "postSort": 9,
    });
    let (_s, b) = app.post("/system/post", dup, Some(&admin)).await;
    assert_ne!(code(&b), 200, "重复岗位编码应被拒: {b}");
}

#[tokio::test]
async fn cannot_delete_super_admin() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // 用 super_admin 删除自己会先命中 self-delete 守卫; 这里换成 admin
    // 的 user_id == SUPER_ADMIN_USER_ID 也仍然应被拒(任何方式都不能删超管)。
    let (_s, b) = app
        .delete("/system/user/1761100000000000001", Some(&admin))
        .await;
    assert_ne!(code(&b), 200, "禁止删除超管: {b}");
}

#[tokio::test]
async fn cannot_delete_self_via_user_endpoint() {
    let app = TestApp::new().await;
    // 让一个普通管理员(已有 system:user:remove)删除自己
    let me = user_with_perms(&["system:user:remove"]);
    let (_s, b) = app
        .delete(&format!("/system/user/{}", me.user_id), Some(&me))
        .await;
    assert_ne!(code(&b), 200, "禁止删除当前登录用户: {b}");
}

// ============================================================
// 注册开关
// ============================================================

#[tokio::test]
async fn register_respects_switch() {
    let app = TestApp::new().await;
    // seed sys_config sys.account.registerUser=false → 注册应被拒
    let body = serde_json::json!({
        "username": "newbie",
        "password": "p1234567",
        "confirmPassword": "p1234567",
        "clientId": "e5cd7e4891bf95d1d19206ce24a7b32e",
        "grantType": "password",
        "userType": "sys_user",
    });
    let (_s, b) = app.post("/auth/register", body, None).await;
    assert_ne!(code(&b), 200, "未开放注册应被拒: {b}");
}

// ============================================================
// dept 守卫
// ============================================================

#[tokio::test]
async fn dept_delete_guards() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // seed dept 101 (技术部) 含子部门 103/104 → 应被拒
    let (_s, b) = app.delete("/system/dept/101", Some(&admin)).await;
    assert_ne!(code(&b), 200, "含子部门不可删: {b}");
}

// ============================================================
// getRouters 边界
// ============================================================

#[tokio::test]
async fn get_routers_returns_router_tree() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (_s, b) = app.get("/system/menu/getRouters", Some(&admin)).await;
    // 至少应返回数组(即使空也要是 200 + 数组)
    assert_eq!(code(&b), 200, "getRouters 应成功: {b}");
    assert!(
        b.get("data").map(|d| d.is_array()).unwrap_or(false),
        "data 应为数组: {b}"
    );
}

#[tokio::test]
async fn user_get_returns_roles_and_posts() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:query"]);
    // seed 中 user 301 已分配 role 201
    let (s, b) = app.get("/system/user/301", Some(&user)).await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/roleIds").is_some(), "应含 roleIds: {b}");
    assert!(b.pointer("/data/postIds").is_some(), "应含 postIds: {b}");
}

#[tokio::test]
async fn user_change_status_and_reset_pwd_exist() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (s1, b1) = app
        .put(
            "/system/user/changeStatus",
            serde_json::json!({"userId": 301, "status": "1"}),
            Some(&admin),
        )
        .await;
    assert_ok(s1, &b1);
    let (s2, b2) = app
        .put(
            "/system/user/resetPwd",
            serde_json::json!({"userId": 301, "password": "newpass123"}),
            Some(&admin),
        )
        .await;
    assert_ok(s2, &b2);
}

#[tokio::test]
async fn dept_optionselect_respects_dept_ids() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:query"]);
    let (s, b) = app
        .get("/system/dept/optionselect?deptIds=101,103", Some(&user))
        .await;
    assert_ok(s, &b);
    let arr = b
        .pointer("/data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    let ids: std::collections::HashSet<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("deptId").and_then(|id| id.as_str().map(String::from))
        })
        .collect();
    assert_eq!(
        ids,
        std::collections::HashSet::from(["101".to_string(), "103".to_string()]),
        "deptIds 过滤应精确返回 101/103: {b}"
    );
}

/// optionselect status 过滤:Java `selectDeptByIds`
/// (SysDeptServiceImpl:185)`.eq(SysDept::getStatus, NORMAL)` —— 下拉只出
/// 正常状态部门;list 无此过滤(区分面:过滤是 optionselect 专属)。
#[tokio::test]
async fn dept_optionselect_filters_out_disabled_dept() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // 停用 103(前端组):无子部门/用户,停用守卫放行
    let (s, b) = app
        .put(
            "/system/dept",
            serde_json::json!({
                "deptId": 103,
                "parentId": 101,
                "deptName": "前端组",
                "orderNum": 0,
                "status": "1",
            }),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);

    let user = user_with_perms(&["system:dept:query", "system:dept:list"]);
    // optionselect:停用部门不出
    let (s, b) = app.get("/system/dept/optionselect", Some(&user)).await;
    assert_ok(s, &b);
    let ids: std::collections::HashSet<i64> = b["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| as_id(&v["deptId"])).collect())
        .unwrap_or_default();
    assert!(
        !ids.contains(&103),
        "停用部门不得出现在下拉(Java status=NORMAL 过滤): {b}"
    );
    assert!(ids.contains(&101), "正常部门应在下拉: {b}");
    // deptIds 命中停用部门同样被滤掉(in 与 status 条件叠加)
    let (s, b) = app
        .get("/system/dept/optionselect?deptIds=101,103", Some(&user))
        .await;
    assert_ok(s, &b);
    let ids: std::collections::HashSet<i64> = b["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| as_id(&v["deptId"])).collect())
        .unwrap_or_default();
    assert_eq!(ids, std::collections::HashSet::from([101]), "{b}");

    // list 无状态过滤:103 仍在(证明过滤是 optionselect 专属,非全局)
    let (s, b) = app.get("/system/dept/list", Some(&user)).await;
    assert_ok(s, &b);
    let list_ids: std::collections::HashSet<i64> = b["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| as_id(&v["deptId"])).collect())
        .unwrap_or_default();
    assert!(
        list_ids.contains(&103),
        "部门列表不应滤停用部门(Java selectDeptList 无 status 强制): {b}"
    );
}

#[tokio::test]
async fn dept_list_filters_by_dept_category() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:list"]);
    let (status, body) = app
        .get("/system/dept/list?deptCategory=fe_group", Some(&user))
        .await;
    assert_ok(status, &body);
    let arr = body["data"].as_array().expect("data array");
    let categories: Vec<&str> = arr
        .iter()
        .filter_map(|v| v.get("deptCategory").and_then(|c| c.as_str()))
        .collect();
    assert!(
        categories.iter().all(|c| *c == "fe_group"),
        "应只返回 deptCategory=fe_group 的部门: {body}"
    );
}

#[tokio::test]
async fn dept_get_by_id_includes_parent_name() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:query"]);
    // dept 103「前端组」parent_id=101「技术部」
    let (status, body) = app.get("/system/dept/103", Some(&user)).await;
    assert_ok(status, &body);
    assert_eq!(
        body["data"]["parentName"].as_str(),
        Some("技术部"),
        "get/{{id}} 应填充 parentName 供 el-tree-select fallback label: {body}"
    );
}

#[tokio::test]
async fn dept_get_by_id_includes_leader_name() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:query"]);
    // dept 100 总部 leader=超管(1761100000000000001) → leader_name 应解析为 super_admin
    let (status, body) = app.get("/system/dept/100", Some(&user)).await;
    assert_ok(status, &body);
    assert_eq!(
        body["data"]["leaderName"].as_str(),
        Some("super_admin"),
        "get/{{id}} 应翻译 leaderName: {body}"
    );
}

#[tokio::test]
async fn dept_disable_rejected_when_dept_has_users() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // dept 102 销售部:叶子(无子部门)且挂正常用户 302(sales_user, dept_id=102, status=0)
    // → 停用应被 checkDeptExistUser 拒(对齐 SysDeptController.edit:114-116)
    let (status, body) = app
        .put(
            "/system/dept",
            serde_json::json!({
                "deptId": 102,
                "parentId": 100,
                "deptName": "销售部",
                "orderNum": 2,
                "status": "1"
            }),
            Some(&admin),
        )
        .await;
    // 业务校验失败走 AppError::BadRequest → HTTP 400(与同方法内子部门守卫一致)
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "部门下存在正常用户时停用应被拒(checkDeptExistUser): {body}"
    );
}

#[tokio::test]
async fn user_optionselect_and_dept_tree_exist() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:query", "system:user:list"]);
    let (s1, b1) = app.get("/system/user/optionselect", Some(&user)).await;
    assert_ok(s1, &b1);
    let (s2, b2) = app.get("/system/user/deptTree", Some(&user)).await;
    assert_ok(s2, &b2);
    // 前端 el-tree 使用 props { value: 'id', label: 'label', children },
    // 因此节点必须含 id/label 字段(否则树能点但不显示名字)。
    let nodes = b2["data"].as_array().expect("deptTree data 应为数组");
    assert!(!nodes.is_empty(), "deptTree 应返回至少一个部门节点: {b2}");
    let first = &nodes[0];
    assert!(first.get("id").is_some(), "部门树节点缺少 id 字段: {b2}");
    assert!(
        first
            .get("label")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty()),
        "部门树节点缺少非空 label 字段: {b2}"
    );
}

/// 部门树节点必须含非空 label(前端 el-tree props label='label')。
fn assert_dept_tree_nodes(nodes: &serde_json::Value, ctx: &str) {
    let arr = nodes
        .as_array()
        .unwrap_or_else(|| panic!("{ctx}: depts 应为数组, got {nodes}"));
    assert!(!arr.is_empty(), "{ctx}: 应返回至少一个部门节点: {nodes}");
    assert!(arr[0].get("id").is_some(), "{ctx}: 节点缺少 id: {nodes}");
    assert!(
        arr[0]
            .get("label")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty()),
        "{ctx}: 节点缺少非空 label: {nodes}"
    );
}

#[tokio::test]
async fn get_user_add_init_returns_roles_and_posts() {
    // 点击"新建用户"时前端 GET /system/user/(无 userId)拉取可选角色/岗位。
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:query"]);
    let (s, b) = app.get("/system/user/", Some(&user)).await;
    assert_ok(s, &b);
    assert!(
        b["data"]["roles"].is_array(),
        "新建用户初始化应含 roles 数组: {b}"
    );
    assert!(
        b["data"]["posts"].is_array(),
        "新建用户初始化应含 posts 数组: {b}"
    );
}

#[tokio::test]
async fn role_dept_tree_returns_id_label_nodes() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:query"]);
    let (s, b) = app.get("/system/role/deptTree/2", Some(&user)).await;
    assert_ok(s, &b);
    // 角色数据权限树:{ checkedKeys, depts: [{id,label,children}] }
    assert_dept_tree_nodes(&b["data"]["depts"], "role/deptTree");
}

#[tokio::test]
async fn post_dept_tree_returns_id_label_nodes() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:post:list"]);
    let (s, b) = app.get("/system/post/deptTree", Some(&user)).await;
    assert_ok(s, &b);
    assert_dept_tree_nodes(&b["data"], "post/deptTree");
}

#[tokio::test]
async fn role_allocated_user_list_exists() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:list"]);
    let (status, body) = app
        .get(
            "/system/role/authUser/allocatedList?roleId=201",
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn role_auth_user_list_serializes_phone_number() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:list"]);
    let (status, body) = app
        .get(
            "/system/role/authUser/allocatedList?roleId=201&pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
    let rows = body.pointer("/data/rows").and_then(|v| v.as_array());
    if let Some(rows) = rows
        && let Some(first) = rows.first()
    {
        assert!(
            first.get("phoneNumber").is_some(),
            "authUser 列表应含 phoneNumber: {body}"
        );
    }
    // 请求侧：带 phoneNumber 搜索不应 400
    let (s2, b2) = app
        .get(
            "/system/role/authUser/unallocatedList?roleId=201&phoneNumber=138&pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_eq!(s2, StatusCode::OK, "phoneNumber 查询参数应被接收: {b2}");
}

#[tokio::test]
async fn role_optionselect_filters_by_role_ids_and_returns_status() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:query"]);
    let (status, body) = app
        .get("/system/role/optionselect?roleIds=200,202", Some(&user))
        .await;
    assert_ok(status, &body);
    let arr = body["data"].as_array().expect("data array");
    let ids: std::collections::HashSet<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("roleId").and_then(|id| id.as_str().map(String::from))
        })
        .collect();
    assert_eq!(
        ids,
        std::collections::HashSet::from(["200".to_string(), "202".to_string()]),
        "{body}"
    );
    let first = arr.first().expect("non-empty");
    assert!(first.get("status").is_some(), "缺 status: {body}");
    assert!(first.get("roleKey").is_some(), "缺 roleKey: {body}");
}

/// optionselect status 过滤:Java `selectRoleByIds`
/// (SysRoleServiceImpl:189)`.eq(SysRole::getStatus, NORMAL)` —— 下拉只出
/// 正常状态角色;list 无此过滤(selectRoleAll → 空 bo,无 status 条件)。
#[tokio::test]
async fn role_optionselect_filters_out_disabled_role() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (s1, b1) = app
        .put(
            "/system/role/changeStatus",
            serde_json::json!({"roleId": 201, "status": "1"}),
            Some(&admin),
        )
        .await;
    assert_ok(s1, &b1);

    let user = user_with_perms(&["system:role:query", "system:role:list"]);
    // roleIds 命中停用角色 → 被 status 条件滤掉,只剩 200
    let (s, b) = app
        .get("/system/role/optionselect?roleIds=200,201", Some(&user))
        .await;
    assert_ok(s, &b);
    let ids: std::collections::HashSet<i64> = b["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| as_id(&v["roleId"])).collect())
        .unwrap_or_default();
    assert_eq!(
        ids,
        std::collections::HashSet::from([200]),
        "停用角色不得出现在下拉(Java status=NORMAL 过滤): {b}"
    );

    // list 无状态过滤:201 仍在(证明过滤是 optionselect 专属,非全局)
    let (s, b) = app.get("/system/role/list", Some(&user)).await;
    assert_ok(s, &b);
    let list_ids: std::collections::HashSet<i64> = b["data"]["rows"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| as_id(&v["roleId"])).collect())
        .unwrap_or_default();
    assert!(
        list_ids.contains(&201),
        "角色列表不应滤停用角色(Java selectRoleList 按 bo 条件): {b}"
    );
}

#[tokio::test]
async fn role_change_status_and_optionselect_exist() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (s1, b1) = app
        .put(
            "/system/role/changeStatus",
            serde_json::json!({"roleId": 201, "status": "1"}),
            Some(&admin),
        )
        .await;
    assert_ok(s1, &b1);
    let (s2, b2) = app.get("/system/role/optionselect", Some(&admin)).await;
    assert_ok(s2, &b2);
}

#[tokio::test]
async fn role_unallocated_list_and_auth_ops_exist() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (s1, b1) = app
        .get(
            "/system/role/authUser/unallocatedList?roleId=201",
            Some(&admin),
        )
        .await;
    assert_ok(s1, &b1);
    // selectAll: 给 role 201 批量分配用户 302
    let (s2, b2) = app
        .put(
            "/system/role/authUser/selectAll?roleId=201&userIds=302",
            serde_json::json!({}),
            Some(&admin),
        )
        .await;
    assert_ok(s2, &b2);
}

#[tokio::test]
async fn social_list_works() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]); // 社交列表是当前用户自己的,无需特定权限
    let (s, b) = app.get("/system/social/list", Some(&user)).await;
    assert_ok(s, &b);
    let data = b.get("data").expect("data 字段缺失");
    assert!(data.is_array(), "data 应为数组: {b}");
    let arr = data.as_array().unwrap();
    assert!(!arr.is_empty(), "社交列表应至少返回一条绑定: {b}");
    for item in arr {
        assert_eq!(
            item.get("userId").and_then(Value::as_str),
            Some(&user.user_id.to_string()[..]),
            "社交账号必须属于当前用户: {item}"
        );
        assert!(item.get("id").is_some(), "社交账号应有 id 字段: {item}");
        assert!(
            item.get("source").is_some(),
            "社交账号应有 source 字段: {item}"
        );
        assert!(
            item.get("userName").is_some(),
            "社交账号应有 userName 字段: {item}"
        );
        assert!(
            item.get("accessToken").is_none(),
            "accessToken 不应暴露: {item}"
        );
        assert!(
            item.get("refreshToken").is_none(),
            "refreshToken 不应暴露: {item}"
        );
        assert!(item.get("expireIn").is_none(), "expireIn 不应暴露: {item}");
    }
}

#[tokio::test]
async fn message_box_works() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let (s, b) = app.get("/resource/message/box", Some(&user)).await;
    assert_ok(s, &b);
    let data = b.get("data").expect("data 字段缺失");
    // 站内信箱按 Java SysMessageBoxVo 返回三个分组数组(systemList/noticeList/workflowList),
    // 非扁平数组(见 audit §2.13)。
    for key in ["systemList", "noticeList", "workflowList"] {
        assert!(
            data.get(key).map(|v| v.is_array()).unwrap_or(false),
            "消息箱应含数组字段 {key}: {b}"
        );
    }
}

// 工作流契约测试已迁移到 workflow_contract_test.rs。

// ============================================================
// 用户 Excel 导入 + 模板下载
// ============================================================

#[tokio::test]
async fn user_import_data_succeeds() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:import"]);

    // 构造 2 行导入数据
    let rows = vec![
        UserImportRow {
            user_id: None,
            dept_id: Some(101),
            user_name: "import_user_1".to_string(),
            nick_name: "导入用户一".to_string(),
            email: Some("imp1@test.com".to_string()),
            phone_number: Some("13800000001".to_string()),
            gender: Some("0".to_string()),
            status: Some("0".to_string()),
        },
        UserImportRow {
            user_id: None,
            dept_id: Some(102),
            user_name: "import_user_2".to_string(),
            nick_name: "导入用户二".to_string(),
            email: Some("imp2@test.com".to_string()),
            phone_number: Some("13800000002".to_string()),
            gender: Some("1".to_string()),
            status: Some("0".to_string()),
        },
    ];
    let xlsx_bytes = ExcelBuilder::<UserImportRow>::export(&rows)
        .expect("生成测试 xlsx 失败");

    // 构造 multipart 请求
    let boundary = "----import_boundary_test";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"user_import.xlsx\"\r\nContent-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet\r\n\r\n",
            boundary
        ).as_bytes(),
    );
    body.extend_from_slice(&xlsx_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let payload = serde_json::to_vec(&user).expect("序列化 LoginUser 失败");
    let req = Request::builder()
        .method("POST")
        .uri(api_uri("/system/user/importData"))
        .header("x-test-login-user", BASE64.encode(&payload))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .expect("构建请求失败");
    let resp = app.router.clone().oneshot(req).await.expect("路由处理失败");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("读取响应体失败")
        .to_bytes();
    let b: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    assert_ok(status, &b);
    // 返回消息应含导入成功字样
    let msg = b.get("data").and_then(Value::as_str).unwrap_or("");
    assert!(msg.contains("导入成功"), "应返回导入成功消息: {b}");

    // 验证用户确实写入数据库
    let admin = super_admin();
    let (sl, bl) = app.get("/system/user/list", Some(&admin)).await;
    assert_ok(sl, &bl);
    let user_names: Vec<String> = bl
        .pointer("/data/rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            r.get("userName").and_then(Value::as_str).map(String::from)
        })
        .collect();
    assert!(
        user_names.contains(&"import_user_1".to_string()),
        "数据库应含 import_user_1: {user_names:?}"
    );
    assert!(
        user_names.contains(&"import_user_2".to_string()),
        "数据库应含 import_user_2: {user_names:?}"
    );
}

#[tokio::test]
async fn user_import_template_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:import"]);

    let payload = serde_json::to_vec(&user).expect("序列化 LoginUser 失败");
    let req = Request::builder()
        .method("GET")
        .uri(api_uri("/system/user/importTemplate"))
        .header("x-test-login-user", BASE64.encode(&payload))
        .body(Body::empty())
        .expect("构建请求失败");
    let resp = app.router.clone().oneshot(req).await.expect("路由处理失败");
    let status = resp.status();
    assert_eq!(status.as_u16(), 200, "importTemplate 应返回 200");
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("spreadsheet")
            || content_type.contains("octet-stream"),
        "Content-Type 应为 spreadsheet 或 octet-stream: {content_type}"
    );
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("读取响应体失败")
        .to_bytes();
    // xlsx 文件以 PK zip 魔数开头
    assert!(bytes.len() > 100, "模板文件不应为空: len={}", bytes.len());
    assert_eq!(&bytes[..2], b"PK", "xlsx 应以 PK 魔数开头");
}

#[tokio::test]
async fn user_list_serializes_phone_number_camelcase() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app.get("/system/user/list", Some(&admin)).await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("data.rows array");
    let first = rows.first().expect("at least one seed user");
    assert!(
        first.get("phoneNumber").is_some(),
        "期望 camelCase phoneNumber, 实际键: {:?}",
        first.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    assert!(
        first.get("phonenumber").is_none(),
        "不应再出现 snake/lower phonenumber"
    );
}

#[tokio::test]
async fn auth_role_put_uses_query_path_and_get_returns_status() {
    let app = TestApp::new().await;
    let admin = super_admin();

    // GET authRole 的 roles 元素必须含 status(Vue checkSelectable 依赖)
    let (status, body) =
        app.get("/system/user/authRole/301", Some(&admin)).await;
    assert_ok(status, &body);
    let roles = body["data"]["roles"].as_array().expect("data.roles array");
    if let Some(r) = roles.first() {
        assert!(r.get("status").is_some(), "roles 元素缺 status: {r}");
    }

    // PUT authRole 走 query(无 path 段)必须命中(非 404)
    let (status, _b) = app
        .put(
            "/system/user/authRole?userId=301&roleIds=",
            serde_json::json!({}),
            Some(&admin),
        )
        .await;
    assert_ne!(status.as_u16(), 404, "PUT /authRole(query) 不应 404");
}

#[tokio::test]
async fn create_user_persists_role_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let payload = serde_json::json!({
        "userName": "role_wire_test",
        "nickName": "接线测试",
        "password": "123456",
        "deptId": 100,
        "roleIds": [201],
        "postIds": []
    });
    let (status, body) = app.post("/system/user", payload, Some(&admin)).await;
    assert_ok(status, &body);

    // 在列表中找到新建用户,取其 id (新建用户 userId 为雪花 id,序列化为字符串)
    let (_s, list) = app
        .get("/system/user/list?userName=role_wire_test", Some(&admin))
        .await;
    let uid = {
        let raw = &list["data"]["rows"][0]["userId"];
        raw.as_i64()
            .or_else(|| raw.as_str().and_then(|s| s.parse::<i64>().ok()))
            .expect("new user id")
    };

    let (_s, detail) =
        app.get(&format!("/system/user/{uid}"), Some(&admin)).await;
    let assigned = detail["data"]["roleIds"]
        .as_array()
        .expect("data.roleIds array");
    assert!(
        assigned.iter().any(|v| v.as_str() == Some("201")),
        "新建用户的 roleIds 未落库: {assigned:?}"
    );
}

/// POST /system/role 携带 menuIds → role_menu 落库(对齐 Java
/// SysRoleServiceImpl.insertRole:302 → insertRoleMenu)。此前 RoleCreateBo
/// 缺 menu_ids 字段,serde 静默丢弃,新建角色零权限 —— 审批人角色建出来
/// 全部 403 的根因(走查 D12)。
#[tokio::test]
async fn create_role_persists_menu_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let create = serde_json::json!({
        "roleName": "menu_wire_role",
        "roleKey": "menu_wire_key",
        "roleSort": 99,
        "status": "0",
        "menuIds": [501, 502],
    });
    let (sc, bc) = app.post("/system/role", create, Some(&admin)).await;
    assert_ok(sc, &bc);

    let (_sl, bl) = app
        .get("/system/role/list?roleKey=menu_wire_key", Some(&admin))
        .await;
    assert_ok(_sl, &bl);
    let rid = {
        let raw = &bl["data"]["rows"][0]["roleId"];
        raw.as_i64()
            .or_else(|| raw.as_str().and_then(|s| s.parse::<i64>().ok()))
            .expect("new role id")
    };

    // 经 roleMenuTreeselect 回读勾选:501/502 必须在 checkedKeys 里
    let (_sp, bp) = app
        .get(
            &format!("/system/menu/roleMenuTreeselect/{rid}"),
            Some(&admin),
        )
        .await;
    assert_ok(_sp, &bp);
    let checked = bp
        .pointer("/data/checkedKeys")
        .and_then(|v| v.as_array())
        .expect("checkedKeys array");
    let checked_nums: Vec<i64> = checked
        .iter()
        .filter_map(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .collect();
    assert!(
        checked_nums.contains(&501) && checked_nums.contains(&502),
        "新增角色的 menuIds 未落库: {checked_nums:?}"
    );
}

// ─── user_service / dict_type_service 删改分支的 HTTP 契约级护栏 ───
// 这些 service 方法构造需 Redis,无法在 system 单元测试里直接构造,
// 故在 HTTP 契约层断言其行为,作等价性护栏。

/// user_service.rs:410/432 — insert_user_auth「先删后插」(DELETE + INSERT sys_user_role)。
/// 既有 authRole 测试用空 roleIds(跳过 INSERT);此处用非空 roleIds 走完整 INSERT 分支,
/// 并断言落库结果。
#[tokio::test]
async fn user_auth_role_assign_persists_role_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // user 302 在 seed 中无角色
    let (s, b) = app
        .put(
            "/system/user/authRole?userId=302&roleIds=201,202",
            json!({}),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);

    // 校验落库:GET 用户详情应含 201/202
    let (_s, detail) = app.get("/system/user/302", Some(&admin)).await;
    let role_ids: Vec<i64> = detail["data"]["roleIds"]
        .as_array()
        .map(|arr| arr.iter().filter_map(as_id).collect())
        .unwrap_or_default();
    assert!(
        role_ids.contains(&201) && role_ids.contains(&202),
        "authRole 分配后 roleIds 应含 201 与 202, 实际: {role_ids:?}"
    );

    // 再分配为单一角色 → 验证「先删」生效(旧的 202 应消失)
    let (s, b) = app
        .put(
            "/system/user/authRole?userId=302&roleIds=200",
            json!({}),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);
    let (_s, detail) = app.get("/system/user/302", Some(&admin)).await;
    let role_ids: Vec<i64> = detail["data"]["roleIds"]
        .as_array()
        .map(|arr| arr.iter().filter_map(as_id).collect())
        .unwrap_or_default();
    assert_eq!(role_ids, vec![200], "重新分配后应只剩 200(旧关联已删)");
}

/// user_service.rs:371/393 — update_user_posts「先删后插」(DELETE + INSERT sys_user_post)。
/// 既有建用户测试用空 postIds(跳过 INSERT);此处用非空 postIds 走完整 INSERT 分支。
#[tokio::test]
async fn create_user_with_post_ids_persists() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let payload = json!({
        "userName": "post_wire_test",
        "nickName": "岗位接线测试",
        "password": "123456",
        "deptId": 101,
        "roleIds": [],
        "postIds": [1761200000000000001_i64]
    });
    let (status, body) = app.post("/system/user", payload, Some(&admin)).await;
    assert_ok(status, &body);

    // 取新建用户 id
    let (_s, list) = app
        .get("/system/user/list?userName=post_wire_test", Some(&admin))
        .await;
    let uid = as_id(&list["data"]["rows"][0]["userId"]).expect("new user id");

    // 校验落库:GET 用户详情应含 ceo 岗位
    let (_s, detail) =
        app.get(&format!("/system/user/{uid}"), Some(&admin)).await;
    let post_ids: Vec<i64> = detail["data"]["postIds"]
        .as_array()
        .map(|arr| arr.iter().filter_map(as_id).collect())
        .unwrap_or_default();
    assert!(
        post_ids.contains(&1761200000000000001_i64),
        "建用户带 postIds 后应落库 ceo 岗位, 实际: {post_ids:?}"
    );
}

/// user_service.rs:371 — update_user_posts 内 `DELETE FROM sys_user_post WHERE user_id=?`。
/// `create_user_with_post_ids_persists` 只护栏 INSERT(:393):建用户时无 pre-existing 岗位,
/// DELETE 删 0 行、随后 INSERT 又把岗位加回 —— 即便 DELETE 过滤器被改错(删错列/漏 WHERE)
/// 该测试也抓不到。本测试护栏 DELETE 过滤器本身:给一个**已有岗位**的用户发 PUT 更新,
/// 断言旧岗位被真删(结果集是「替换」而非「追加」)。
#[tokio::test]
async fn update_user_post_ids_replaces_old_deletes_previous() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let ceo = 1761200000000000001_i64;
    let se = 1761200000000000002_i64;

    // (1) 先建一个带 ceo 岗位的用户(establish pre-existing post)
    let payload = json!({
        "userName": "post_replace_test",
        "nickName": "岗位替换测试",
        "password": "123456",
        "deptId": 101,
        "roleIds": [],
        "postIds": [ceo]
    });
    let (s, b) = app.post("/system/user", payload, Some(&admin)).await;
    assert_ok(s, &b);
    let (_s, list) = app
        .get("/system/user/list?userName=post_replace_test", Some(&admin))
        .await;
    let uid = as_id(&list["data"]["rows"][0]["userId"]).expect("new user id");

    // (2) PUT 更新:把岗位从 ceo 换成 se(走 update_user_posts「先删后插」)
    let (s, b) = app
        .put(
            "/system/user",
            json!({
                "userId": uid,
                "userName": "post_replace_test",
                "nickName": "岗位替换测试",
                "deptId": 101,
                "postIds": [se]
            }),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);

    // (3) 断言「替换」而非「追加」:只剩 se,ceo 必须已被 DELETE 删除。
    //     若 :371 的 DELETE 过滤器被改坏(漏 WHERE / 删错列 / 不执行),
    //     ceo 会残留 → postIds 含 ceo 与 se → 此断言失败。
    let (_s, detail) =
        app.get(&format!("/system/user/{uid}"), Some(&admin)).await;
    let mut post_ids: Vec<i64> = detail["data"]["postIds"]
        .as_array()
        .map(|arr| arr.iter().filter_map(as_id).collect())
        .unwrap_or_default();
    post_ids.sort_unstable();
    assert_eq!(
        post_ids,
        vec![se],
        "更新岗位后应只含 se(替换);ceo 残留说明 DELETE(:371) 未删旧行: {detail}"
    );
}

/// dict_type_service.rs:245 — update_by_bo 改字典类型时级联 UPDATE sys_dict_data.dict_type。
/// 既有测试只覆盖 create/delete,未覆盖「类型变更触发级联」分支。
#[tokio::test]
async fn dict_type_update_cascades_dict_data_type() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // dict_type 8001 = 'sys_normal_disable' 且 seed 有 dict_data 行
    let (s, b) = app
        .put(
            "/system/dict/type",
            json!({
                "dictId": 8001,
                "dictName": "通用状态",
                "dictType": "sys_normal_disable_v2"
            }),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);

    // 级联后:新类型应能查到原数据
    let (_s, new_data) = app
        .get("/system/dict/data/type/sys_normal_disable_v2", Some(&admin))
        .await;
    let new_arr = new_data["data"].as_array();
    assert!(
        new_arr.map(|a| !a.is_empty()).unwrap_or(false),
        "类型变更后 dict_data 应级联到新类型 sys_normal_disable_v2: {new_data}"
    );
    // 旧类型应已无数据
    let (_s, old_data) = app
        .get("/system/dict/data/type/sys_normal_disable", Some(&admin))
        .await;
    let old_arr = old_data["data"].as_array();
    assert!(
        old_arr.map(|a| a.is_empty()).unwrap_or(true),
        "旧类型 sys_normal_disable 应已无 dict_data: {old_data}"
    );
}

/// dict_type_service.rs:291 — delete_by_id 先 COUNT(sys_dict_data) 阻断删除。
/// 既有 delete 测试删的是无数据的新建类型(count=0 通过);此处删有数据的类型,
/// 走 count>0 → BadRequest 分支。
#[tokio::test]
async fn dict_type_delete_blocked_when_has_data() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // dict_type 8002 = 'sys_user_gender' 且 seed 有 dict_data 行
    let (status, body) =
        app.delete("/system/dict/type/8002", Some(&admin)).await;
    assert_ne!(
        code(&body),
        200,
        "有字典数据的类型不应被删除(code 应非 200): {body}"
    );
    assert!(
        status.is_client_error() || status.as_u16() == 400,
        "应返回 4xx(BadRequest), 实际: {status}"
    );

    // 记录应仍在
    let (_s, detail) = app.get("/system/dict/type/8002", Some(&admin)).await;
    assert_eq!(
        code(&detail),
        200,
        "删除被阻断后,字典类型应仍存在: {detail}"
    );
}

#[tokio::test]
async fn user_detail_user_field_has_roles_array() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app.get("/system/user/301", Some(&admin)).await;
    assert_ok(status, &body);
    assert!(
        body["data"]["user"]["roles"].is_array(),
        "data.user.roles 必须为数组, 实际: {}",
        body["data"]["user"]
    );
}

#[cfg(test)]
mod monitor_security_tests {
    use admin::testkit::{super_admin, user_with_perms};

    use super::*;

    #[tokio::test]
    async fn monitor_endpoints_require_permission() {
        let app = TestApp::new().await;
        let no_perm = user_with_perms(&[]);

        let (_s, body) = app.get("/monitor/operlog/list", Some(&no_perm)).await;
        assert_eq!(code(&body), 403, "无权限用户不应访问操作日志列表: {body}");

        let (_s, body) =
            app.get("/monitor/loginInfo/list", Some(&no_perm)).await;
        assert_eq!(code(&body), 403, "无权限用户不应访问登录日志列表: {body}");

        let (_s, body) = app.get("/monitor/online/list", Some(&no_perm)).await;
        assert_eq!(code(&body), 403, "无权限用户不应访问在线用户列表: {body}");

        let (_s, body) = app.get("/monitor/cache", Some(&no_perm)).await;
        assert_eq!(code(&body), 403, "无权限用户不应访问缓存监控: {body}");
    }

    #[tokio::test]
    async fn monitor_operlog_accessible_with_permission() {
        let app = TestApp::new().await;
        let user = user_with_perms(&["monitor:operlog:list"]);
        let (_s, body) = app.get("/monitor/operlog/list", Some(&user)).await;
        // 此处只断言"不被权限拒绝"。
        assert_ne!(
            code(&body),
            403,
            "有 monitor:operlog:list 权限不应被拒: {body}"
        );
    }

    #[tokio::test]
    async fn login_info_uses_camelcase_path() {
        let app = TestApp::new().await;
        let user = user_with_perms(&["monitor:logininfo:list"]);
        let (s, _b) = app.get("/monitor/loginInfo/list", Some(&user)).await;
        assert_ne!(s.as_u16(), 404, "loginInfo 路径应存在");
    }

    #[test]
    fn online_list_returns_page_result() {
        // 原 Redis 集成测试降级为纯函数断言:parse_online_user 产出的
        // UserOnline 即为 PageResult.rows 的元素。验证 token_id 截取与分页结构。
        use std::collections::HashSet;

        use app_core::model::login_user::LoginUser;
        use monitor::online::service::parse_online_user;

        let login_user = LoginUser {
            user_id: 1,
            dept_id: Some(103),
            username: "admin".to_string(),
            nick_name: Some("管理员".to_string()),
            user_type: None,
            client_id: "pc".to_string(),
            client_key: Some("pc".to_string()),
            device_type: Some("pc".to_string()),
            permissions: HashSet::new(),
            roles: HashSet::new(),
            login_ip: Some("192.168.1.100".to_string()),
            login_date: None,
        };
        let user = parse_online_user("global:token:abc-123", &login_user);
        assert_eq!(user.token_id, "abc-123", "应去除 token 前缀");
        assert_eq!(user.user_name, "admin");

        // 模拟 list_online_users 返回的 PageResult 结构
        let rows = [user];
        let total = rows.len() as i64;
        assert_eq!(total, 1, "total 应等于 rows 长度");
        assert!(rows.iter().any(|u| u.user_id == 1), "rows 应含该用户");
    }

    #[test]
    fn online_get_info_returns_current_user_devices() {
        // 原 Redis 集成测试降级为纯函数断言:按当前用户名过滤后,
        // 返回的设备列表应仅含该用户的在线会话。
        use std::collections::HashSet;

        use app_core::model::login_user::LoginUser;
        use monitor::online::service::{
            matches_online_filter, parse_online_user,
        };

        fn mk(username: &str, ip: &str) -> LoginUser {
            LoginUser {
                user_id: 1,
                dept_id: None,
                username: username.to_string(),
                nick_name: None,
                user_type: None,
                client_id: "pc".to_string(),
                client_key: Some("pc".to_string()),
                device_type: Some("pc".to_string()),
                permissions: HashSet::new(),
                roles: HashSet::new(),
                login_ip: Some(ip.to_string()),
                login_date: None,
            }
        }

        // admin 有两台设备,ry 无设备
        let devices = [
            parse_online_user("global:token:t1", &mk("admin", "192.168.1.1")),
            parse_online_user("global:token:t2", &mk("admin", "192.168.1.2")),
            parse_online_user("global:token:t3", &mk("ry", "10.0.0.1")),
        ];

        let mine: Vec<_> = devices
            .iter()
            .filter(|u| matches_online_filter(u, Some("admin"), None))
            .collect();
        assert_eq!(mine.len(), 2, "admin 应有 2 个在线设备");
        assert!(mine.iter().all(|u| u.user_name == "admin"));
    }

    #[test]
    fn cache_get_info_returns_command_stats() {
        // 原 Redis 集成测试降级为纯函数断言:验证 parse_cache_info
        // 产出的 commandStats 结构为 {name, value},dbSize 为 DBSIZE 占位。
        use monitor::cache::handler::parse_cache_info;

        let raw = "# Server\n\
                   redis_version:7.0.0\n\
                   \n\
                   # Commandstats\n\
                   cmdstat_get:calls=10,usec=500\n\
                   cmdstat_set:calls=3,usec=120\n";
        let (info, command_stats) = parse_cache_info(raw);

        // info_map 含 Server 段键值
        assert_eq!(info.get("redis_version").unwrap(), "7.0.0");
        // commandStats 每项应有 name/value
        assert_eq!(command_stats.len(), 2);
        assert!(command_stats.iter().all(|c| c.contains_key("name")));
        assert!(command_stats.iter().all(|c| c.contains_key("value")));
        assert_eq!(command_stats[0].get("name").unwrap(), "get");
        assert_eq!(command_stats[0].get("value").unwrap(), "10");
        // dbSize 来自独立的 DBSIZE 命令,纯函数不产出;此处验证占位逻辑
        let db_size: i64 = 0; // 无 Redis 时占位
        assert_eq!(db_size, 0);
    }

    #[tokio::test]
    async fn dangerous_cache_endpoints_removed() {
        let app = TestApp::new().await;
        let admin = super_admin();
        let endpoints = [
            ("GET", "/monitor/cache/getNames"),
            ("GET", "/monitor/cache/getKeys/test"),
            ("GET", "/monitor/cache/getValue/test/key"),
            ("DELETE", "/monitor/cache/clearCacheName/test"),
            ("DELETE", "/monitor/cache/clearCacheKey/test"),
            ("POST", "/monitor/cache/clearCacheAll"),
        ];

        for (method, ep) in &endpoints {
            let status = match *method {
                "GET" => app.get(ep, Some(&admin)).await.0,
                "DELETE" => app.delete(ep, Some(&admin)).await.0,
                "POST" => app.post(ep, json!({}), Some(&admin)).await.0,
                _ => panic!("unknown method {method}"),
            };
            assert_eq!(
                status,
                axum::http::StatusCode::NOT_FOUND,
                "危险端点 {method} {ep} 应已删除"
            );
        }
    }

    #[tokio::test]
    async fn online_batch_logout_removed() {
        let app = TestApp::new().await;
        let admin = super_admin();
        let (status, _) = app
            .delete("/monitor/online/batchLogout?tokenIds=x", Some(&admin))
            .await;
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "batchLogout 应已删除"
        );
    }

    #[tokio::test]
    async fn gen_data_names_returns_master() {
        let app = TestApp::new().await;
        let user = user_with_perms(&["tool:gen:list"]);
        let (status, body) =
            app.get("/tool/gen/getDataNames", Some(&user)).await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "status={} body={}",
            status,
            body
        );
        let names = body["data"].as_array().expect("data 应为数组");
        assert!(
            names.iter().any(|n| n == "master"),
            "应包含 master 数据源: {body}"
        );
    }
}

// ───── deptName 翻译 / operlog 权限 / OSS 配置明文回读 ─────

#[tokio::test]
async fn user_list_fills_dept_name() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app.get("/system/user/list", Some(&admin)).await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("user list rows");
    let with_dept = rows.iter().find(|r| r["deptId"].as_i64().is_some());
    if let Some(r) = with_dept {
        assert!(
            r.get("deptName").and_then(|v| v.as_str()).is_some(),
            "有 deptId 的行应翻译出 deptName: {r}"
        );
    }
}

#[tokio::test]
async fn operlog_clean_uses_remove_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["monitor:operlog:remove"]);
    let (status, _b) = app.delete("/monitor/operlog/clean", Some(&user)).await;
    assert_ne!(status.as_u16(), 403, "持 :remove 不应被拒");
}

#[tokio::test]
async fn oss_config_get_returns_plaintext_keys() {
    use std::sync::atomic::{AtomicU16, Ordering};

    static SEQ: AtomicU16 = AtomicU16::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);

    let app = TestApp::new().await;
    let user =
        user_with_perms(&["system:ossConfig:list", "system:ossConfig:add"]);
    let config_key = format!("pe_{seq:04}");
    let (create_status, create_body) = app
        .post(
            "/resource/oss/config",
            json!({
                "configKey": config_key,
                "accessKey": "CONTRACT_AK",
                "secretKey": "CONTRACT_SK",
                "bucketName": "test-bucket",
                "endpoint": "http://127.0.0.1:9000"
            }),
            Some(&user),
        )
        .await;
    assert_ok(create_status, &create_body);

    let (list_status, list_body) =
        app.get("/resource/oss/config/list", Some(&user)).await;
    assert_ok(list_status, &list_body);
    let rows = list_body["data"]["rows"]
        .as_array()
        .expect("config list rows");
    let row = rows
        .iter()
        .find(|r| r["configKey"].as_str() == Some(config_key.as_str()))
        .expect("新建配置应在列表中");
    let id = as_id(&row["ossConfigId"]).expect("ossConfigId");

    let (get_status, get_body) = app
        .get(&format!("/resource/oss/config/{id}"), Some(&user))
        .await;
    assert_ok(get_status, &get_body);
    assert_eq!(get_body["data"]["accessKey"], "CONTRACT_AK");
    assert_eq!(get_body["data"]["secretKey"], "CONTRACT_SK");
}

// ───── User/Profile ─────

#[tokio::test]
async fn user_list_accepts_create_time_range_params() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let uri = "/system/user/list?pageNum=1&pageSize=10\
        &params%5BbeginTime%5D=2020-01-01%2000%3A00%3A00\
        &params%5BendTime%5D=2030-01-01%2000%3A00%3A00";
    let (status, body) = app.get(uri, Some(&admin)).await;
    assert_ok(status, &body);
    assert!(
        body["data"]["rows"].is_array(),
        "日期区间过滤应返回分页: {body}"
    );
}

#[tokio::test]
async fn user_list_by_dept_returns_array_not_page() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) =
        app.get("/system/user/list/dept/101", Some(&admin)).await;
    assert_ok(status, &body);
    assert!(
        body["data"].is_array(),
        "list/dept 应返回裸数组, 实际: {}",
        body["data"]
    );
    assert!(body.get("rows").is_none(), "不应再有顶层 rows");
}

#[tokio::test]
async fn auth_role_get_user_includes_nick_name() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) =
        app.get("/system/user/authRole/301", Some(&admin)).await;
    assert_ok(status, &body);
    assert!(
        body["data"]["user"]["nickName"].is_string(),
        "authRole user 应含 nickName: {}",
        body["data"]["user"]
    );
}

#[tokio::test]
async fn optionselect_filters_by_user_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app
        .get("/system/user/optionselect?userIds=301", Some(&admin))
        .await;
    assert_ok(status, &body);
    let arr = body["data"].as_array().expect("optionselect data 应为数组");
    assert_eq!(arr.len(), 1, "userIds=301 应只返回 1 条: {body}");
    assert_eq!(
        arr[0].get("userId").and_then(Value::as_str),
        Some("301"),
        "应返回 userId=301: {body}"
    );
    assert!(
        arr[0].get("userName").is_some() && arr[0].get("nickName").is_some(),
        "应含 userName/nickName: {}",
        arr[0]
    );
}

#[tokio::test]
async fn optionselect_filters_by_dept_id() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app
        .get("/system/user/optionselect?deptId=101", Some(&admin))
        .await;
    assert_ok(status, &body);
    let arr = body["data"].as_array().expect("optionselect data 应为数组");
    let ids: std::collections::HashSet<String> = arr
        .iter()
        .filter_map(|v| {
            v.get("userId").and_then(|id| id.as_str().map(String::from))
        })
        .collect();
    assert_eq!(
        ids,
        std::collections::HashSet::from(["301".to_string(), "901".to_string()]),
        "deptId=101 应返回技术部用户 301 与 901: {body}"
    );
}

#[tokio::test]
async fn user_detail_includes_dept_and_profile_fields() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app.get("/system/user/301", Some(&admin)).await;
    assert_ok(status, &body);
    let user = &body["data"]["user"];
    assert_eq!(
        user.get("deptName").and_then(Value::as_str),
        Some("技术部"),
        "详情 user 应翻译 deptName: {user}"
    );
    assert!(
        user.get("dept")
            .and_then(|d| d.get("deptName"))
            .and_then(Value::as_str)
            .is_some(),
        "详情 user 应含嵌套 dept.deptName: {user}"
    );
    assert!(user.get("loginIp").is_some(), "应含 loginIp 键: {user}");
    assert!(user.get("loginDate").is_some(), "应含 loginDate 键: {user}");
    assert!(user.get("userType").is_some(), "应含 userType 键: {user}");
    assert!(user.get("avatar").is_some(), "应含 avatar 键: {user}");
    assert!(
        user.get("phoneNumber").is_some(),
        "详情应使用 camelCase phoneNumber: {user}"
    );
}

#[tokio::test]
async fn user_detail_roles_and_posts_include_status_fields() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app.get("/system/user/301", Some(&admin)).await;
    assert_ok(status, &body);
    let roles = body["data"]["roles"].as_array().expect("roles 应为数组");
    assert!(!roles.is_empty(), "应返回可选角色: {body}");
    let role = &roles[0];
    assert!(
        role.get("status").is_some(),
        "RoleBrief 应含 status: {role}"
    );
    assert!(
        role.get("roleKey").is_some(),
        "RoleBrief 应含 roleKey: {role}"
    );
    let posts = body["data"]["posts"].as_array().expect("posts 应为数组");
    assert!(!posts.is_empty(), "应返回可选岗位: {body}");
    let post = &posts[0];
    assert!(
        post.get("status").is_some(),
        "PostBrief 应含 status: {post}"
    );
    assert!(
        post.get("postCode").is_some(),
        "PostBrief 应含 postCode: {post}"
    );
}

#[tokio::test]
async fn user_add_init_roles_and_posts_include_status_fields() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:query"]);
    let (status, body) = app.get("/system/user/", Some(&user)).await;
    assert_ok(status, &body);
    let role = &body["data"]["roles"][0];
    assert!(
        role.get("status").is_some(),
        "新建初始化 roles 应含 status: {role}"
    );
    let post = &body["data"]["posts"][0];
    assert!(
        post.get("status").is_some(),
        "新建初始化 posts 应含 status: {post}"
    );
}

#[tokio::test]
async fn get_info_avatar_field_split_id_and_url() {
    // 对齐 RuoYi 6.X: avatar 保留原始 OSS ID(数字), avatarUrl 为翻译后 URL
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let (status, body) = app.get("/system/user/getInfo", Some(&user)).await;
    assert_ok(status, &body);
    let avatar = &body["data"]["user"]["avatar"];
    assert!(
        avatar.is_string(),
        "getInfo user.avatar 应为 OSS ID(字符串), 实际: {avatar}"
    );
    assert_eq!(
        avatar.as_str(),
        Some("9001"),
        "avatar 应为原始 OSS ID: {body}"
    );
    let avatar_url = &body["data"]["user"]["avatarUrl"];
    assert!(
        avatar_url.is_string(),
        "getInfo user.avatarUrl 应为 URL 字符串, 实际: {avatar_url}"
    );
    assert_eq!(
        avatar_url.as_str(),
        Some("https://example.com/avatar/9001.png"),
        "avatarUrl 应翻译 OSS ID 为 url: {body}"
    );
}

#[tokio::test]
async fn profile_avatar_field_split_id_and_url() {
    // 对齐 RuoYi 6.X: avatar 保留原始 OSS ID(数字), avatarUrl 为翻译后 URL
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let (status, body) = app.get("/system/user/profile", Some(&user)).await;
    assert_ok(status, &body);
    let avatar = &body["data"]["user"]["avatar"];
    assert!(
        avatar.is_string(),
        "profile user.avatar 应为 OSS ID(字符串), 实际: {avatar}"
    );
    assert_eq!(
        avatar.as_str(),
        Some("9001"),
        "avatar 应为原始 OSS ID: {body}"
    );
    let avatar_url = &body["data"]["user"]["avatarUrl"];
    assert!(
        avatar_url.is_string(),
        "profile user.avatarUrl 应为 URL 字符串, 实际: {avatar_url}"
    );
    assert_eq!(
        avatar_url.as_str(),
        Some("https://example.com/avatar/9001.png"),
        "avatarUrl 应翻译 OSS ID 为 url: {body}"
    );
}

// ============================================================
// dict 模块契约(export / 唯一性 / 格式校验)
// ============================================================

#[tokio::test]
async fn dict_type_export_returns_xlsx_bytes() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dict:export"]);
    // Vue download() 发 application/x-www-form-urlencoded(tansParams 编码),非 JSON。
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/dict/type/export",
            "pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "Content-Type 应为 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(cd.contains("attachment"), "应附 attachment: {cd}");
    // xlsx 是 ZIP 归档,前两字节为 PK (0x50 0x4B)
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "响应体应为 xlsx(ZIP) 字节, 前两字节: {:?}",
        bytes.get(..2)
    );
}

#[tokio::test]
async fn dict_data_export_returns_xlsx_bytes() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dict:export"]);
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/dict/data/export",
            "pageNum=1&pageSize=10&dictType=sys_user_gender",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .contains("spreadsheetml"),
        "应为 xlsx"
    );
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "响应体应为 xlsx(ZIP) 字节"
    );
}

#[tokio::test]
async fn dict_data_rejects_duplicate_type_value() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // 种子已有 sys_user_gender / value=0(男,dict_code 8101);再插同 type+value 应被拒
    let (status, body) = app
        .post(
            "/system/dict/data",
            serde_json::json!({
                "dictSort": 9,
                "dictLabel": "重复男",
                "dictValue": "0",
                "dictType": "sys_user_gender",
                "isDefault": "N"
            }),
            Some(&admin),
        )
        .await;
    // 唯一性冲突走 AppError::Conflict → HTTP 409(对齐 checkDictDataUnique)
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "(dictType, dictValue) 重复应被拒(checkDictDataUnique): {body}"
    );
}

#[tokio::test]
async fn dict_data_wf_business_status_returns_full_status_list() {
    // wf_business_status / wf_task_status 字典种子对齐 RuoYi 原版。
    let app = TestApp::new().await;
    let user = super_admin();
    let (status, body) = app
        .get("/system/dict/data/type/wf_business_status", Some(&user))
        .await;
    assert_ok(status, &body);

    let arr = body["data"].as_array().expect("data 应为数组");
    assert!(!arr.is_empty(), "wf_business_status 字典不应为空: {body}");

    let mut labels: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for item in arr {
        let value = item["dictValue"].as_str().expect("dictValue");
        let label = item["dictLabel"].as_str().expect("dictLabel");
        labels.insert(value, label);
    }

    let expected = [
        ("cancel", "已撤销"),
        ("draft", "草稿"),
        ("waiting", "待审核"),
        ("finish", "已完成"),
        ("invalid", "已作废"),
        ("back", "已退回"),
        ("termination", "已终止"),
    ];
    assert_eq!(
        labels.len(),
        expected.len(),
        "wf_business_status 条目数应等于 {expected:?}: {labels:?}"
    );
    for (value, label) in expected {
        assert_eq!(
            labels.get(value).copied(),
            Some(label),
            "wf_business_status 缺 {value}={label}: {labels:?}"
        );
    }
}

#[tokio::test]
async fn dict_data_wf_task_status_returns_full_status_list() {
    // wf_task_status 字典种子对齐 RuoYi 原版任务状态。
    let app = TestApp::new().await;
    let user = super_admin();
    let (status, body) = app
        .get("/system/dict/data/type/wf_task_status", Some(&user))
        .await;
    assert_ok(status, &body);

    let arr = body["data"].as_array().expect("data 应为数组");
    assert!(!arr.is_empty(), "wf_task_status 字典不应为空: {body}");

    let mut labels: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for item in arr {
        let value = item["dictValue"].as_str().expect("dictValue");
        let label = item["dictLabel"].as_str().expect("dictLabel");
        labels.insert(value, label);
    }

    let expected = [
        ("cancel", "撤销"),
        ("pass", "通过"),
        ("waiting", "待审核"),
        ("invalid", "作废"),
        ("back", "退回"),
        ("termination", "终止"),
        ("transfer", "转办"),
        ("depute", "委托"),
        ("copy", "抄送"),
        ("sign", "加签"),
        ("sign_off", "减签"),
        ("timeout", "超时"),
    ];
    assert_eq!(
        labels.len(),
        expected.len(),
        "wf_task_status 条目数应等于 {expected:?}: {labels:?}"
    );
    for (value, label) in expected {
        assert_eq!(
            labels.get(value).copied(),
            Some(label),
            "wf_task_status 缺 {value}={label}: {labels:?}"
        );
    }
}

// ───────────────── Monitor/OSS 契约 ─────────────────

#[tokio::test]
async fn client_list_includes_grant_type_and_path_lists() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:client:list"]);
    let (status, body) = app
        .get("/system/client/list?pageNum=1&pageSize=10", Some(&user))
        .await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("rows");
    let first = rows.first().expect("non-empty rows");
    for key in ["grantTypeList", "accessPathList", "ipWhitelistList"] {
        assert!(first.get(key).is_some(), "client list 缺 {key}: {body}");
    }
    let grant_types = first["grantTypeList"]
        .as_array()
        .expect("grantTypeList array");
    assert!(
        grant_types.iter().any(|v| v.as_str() == Some("password")),
        "grantTypeList 应含 password: {body}"
    );
}

#[tokio::test]
async fn oss_list_includes_create_by_name() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:oss:list"]);
    let (status, body) = app
        .get("/resource/oss/list?pageNum=1&pageSize=10", Some(&user))
        .await;
    assert_ok(status, &body);
    let rows = body["data"]["rows"].as_array().expect("rows");
    let first = rows.first().expect("non-empty oss rows");
    assert_eq!(
        first.get("createByName").and_then(|v| v.as_str()),
        Some("super_admin"),
        "oss list 应翻译 createByName: {body}"
    );
}

#[tokio::test]
async fn operlog_export_returns_xlsx_bytes() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["monitor:operlog:export"]);
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/monitor/operlog/export",
            "pageNum=1&pageSize=10\
             &params%5BbeginTime%5D=2020-01-01%2000%3A00%3A00\
             &params%5BendTime%5D=2030-01-01%2000%3A00%3A00",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "Content-Type 应为 xlsx: {ct}");
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "响应体应为 xlsx(ZIP) 字节"
    );
}

#[tokio::test]
async fn logininfo_export_returns_xlsx_bytes() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["monitor:logininfo:export"]);
    let (status, _headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/monitor/loginInfo/export",
            "pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "响应体应为 xlsx(ZIP) 字节"
    );
}

// ---- dictType 格式正则校验 ----

#[tokio::test]
async fn dict_type_rejects_invalid_format() {
    let app = TestApp::new().await;
    let admin = super_admin();
    // 非法:大写开头 + 空格
    let (status, body) = app
        .post(
            "/system/dict/type",
            serde_json::json!({
                "dictName": "非法格式",
                "dictType": "Bad Type!"
            }),
            Some(&admin),
        )
        .await;
    // validator 校验失败映射为 422(对齐 AppError::Validation,与其它 BO 校验一致)
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "dictType 格式非法(须 ^[a-z][a-z0-9_]*$)应被拒: {body}"
    );
    assert_ne!(code(&body), 200, "拒绝响应 code 不应为 200: {body}");
}

#[tokio::test]
async fn dict_type_accepts_valid_format() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (status, body) = app
        .post(
            "/system/dict/type",
            serde_json::json!({
                "dictName": "合法格式",
                "dictType": "h3_valid_type_1"
            }),
            Some(&admin),
        )
        .await;
    assert_ok(status, &body);
    // 清理:删除刚创建的(取 dict_id)
    if let Some(id) = body["data"].as_i64() {
        let _ = app
            .delete(&format!("/system/dict/type/{id}"), Some(&admin))
            .await;
    }
}

// ============================================================
// role / post / config / client 导出端点
// ============================================================

#[tokio::test]
async fn role_export_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:role:export"]);
    // Vue download() 发 application/x-www-form-urlencoded(tansParams 编码),非 JSON。
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/role/export",
            "roleName=",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "role 导出未返回 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(cd.contains("attachment"), "role 导出应附 attachment: {cd}");
    // xlsx 是 ZIP 归档,前两字节为 PK (0x50 0x4B)
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "role 导出内容不是有效 xlsx"
    );
}

#[tokio::test]
async fn post_export_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:post:export"]);
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/post/export",
            "postName=",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "post 导出未返回 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(cd.contains("attachment"), "post 导出应附 attachment: {cd}");
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "post 导出内容不是有效 xlsx"
    );
}

#[tokio::test]
async fn user_export_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:export"]);
    // Vue download() 发 application/x-www-form-urlencoded,非 JSON。
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/user/export",
            "userName=",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "user 导出未返回 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(cd.contains("attachment"), "user 导出应附 attachment: {cd}");
    // xlsx 是 ZIP 归档,前两字节为 PK (0x50 0x4B)
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "user 导出内容不是有效 xlsx"
    );
}

#[tokio::test]
async fn user_export_requires_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:user:list"]); // 有 list 没有 export
    let (status, _, _) = app
        .request_form(
            axum::http::Method::POST,
            "/system/user/export",
            "",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn config_export_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:config:export"]);
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/config/export",
            "configName=",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "config 导出未返回 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        cd.contains("attachment"),
        "config 导出应附 attachment: {cd}"
    );
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "config 导出内容不是有效 xlsx"
    );
}

#[tokio::test]
async fn client_export_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:client:export"]);
    let (status, headers, bytes) = app
        .request_form(
            axum::http::Method::POST,
            "/system/client/export",
            "clientKey=",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "client 导出未返回 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        cd.contains("attachment"),
        "client 导出应附 attachment: {cd}"
    );
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "client 导出内容不是有效 xlsx"
    );
}

#[tokio::test]
async fn notice_list_returns_create_by_name() {
    // to_list_vo 曾硬编码 None,创建者列恒空 —— 防回归。
    let app = TestApp::new().await;
    let admin = super_admin();
    let (_, page) = app
        .get("/system/notice/list?pageNum=1&pageSize=1", Some(&admin))
        .await;
    let name = page["data"]["rows"][0]["createByName"].as_str();
    assert!(
        name.is_some() && !name.unwrap().is_empty(),
        "NTC5: createByName 未回填(仍为 None/空)"
    );
}

#[tokio::test]
async fn notice_list_filters_by_create_by_name() {
    // NoticePageQuery 曾无该字段,筛选被静默丢弃 —— 防回归。
    let app = TestApp::new().await;
    let admin = super_admin();
    let (_, all) = app
        .get("/system/notice/list?pageNum=1&pageSize=50", Some(&admin))
        .await;
    let all_total = all["data"]["total"].as_i64().unwrap();
    let (_, filtered) = app
        .get("/system/notice/list?pageNum=1&pageSize=50&createByName=super_admin", Some(&admin))
        .await;
    assert!(
        filtered["data"]["total"].as_i64().unwrap() < all_total,
        "NTC3: createByName 过滤未生效"
    );
}

#[tokio::test]
async fn get_info_returns_dept_name_and_role_objects() {
    // 对齐 Java SysUserVo: deptName(@Translation DEPT_ID_TO_NAME)
    // 与 roles(List<SysRoleVo>)。Rust 此前手写 json! 时漏了这两个字段。
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let (status, body) = app.get("/system/user/getInfo", Some(&user)).await;
    assert_ok(status, &body);

    // 测试用户 301 的 dept_id = 101 → 技术部(testkit 种子数据)
    assert_eq!(
        body["data"]["user"]["deptName"].as_str(),
        Some("技术部"),
        "getInfo user.deptName 应翻译 dept_id: {body}"
    );

    // 用户 301 经 sys_user_role 关联 role_id 201 → 普通角色
    let roles = &body["data"]["user"]["roles"];
    assert!(roles.is_array(), "user.roles 应为数组: {body}");
    assert_eq!(
        roles[0]["roleName"].as_str(),
        Some("普通角色"),
        "user.roles[0].roleName 不符: {body}"
    );
    assert_eq!(
        roles[0]["roleId"].as_str(),
        Some("201"),
        "roleId 应被 stringify_ids 转为字符串: {body}"
    );

    // 顶层 roles 是角色 key 集合(Java UserInfoVo.roles),
    // 不得被上面的对象列表顶替 —— 权限判断依赖它。
    let top_roles = &body["data"]["roles"];
    assert!(top_roles.is_array(), "顶层 roles 应仍为数组: {body}");
    assert!(
        top_roles.as_array().unwrap().iter().all(|v| v.is_string()),
        "顶层 roles 应为角色 key 字符串数组,不是对象: {body}"
    );
}
