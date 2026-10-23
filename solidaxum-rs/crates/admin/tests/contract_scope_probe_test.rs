//! 范围探针的 HTTP 级契约测试。
//!
//! 三个探针面 × 两个方向(受限用户 + 越权目标 → 404 NotFound 不泄露
//! 存在性;范围内目标 → 2xx),Java 基线行为:
//! - **user add/edit**:`SysUserController:177/:202 → checkDeptDataScope`
//!   (deptId 探针,超管放行、范围外 count=0 → 拒);
//! - **dept edit**:`SysDeptController:106`(bo.deptId 探针居首);
//! - **dept remove**:`SysDeptController:143`(探针居业务守卫之后)。
//!
//! 接线:`TestApp::new_with_resolver` + `AllScopeResolver::with_user` 给
//! 操作者(user_id=301)钉 `DeptIn([101])`(测试 kit 无真实角色语义,
//! resolver 覆盖即"该用户的数据范围");种子部门 101 技术部/102 销售部/
//! 103 前端组/104 后端组,101/103/104 ∈ 技术部子树,102 ∉。
//!
//! 错误形态:探针拒绝呈现为 NotFound(HTTP 404 + code 404)—— 与读路径
//! "这行对你不存在"同形态,不泄露目标存在性。
//!
//! 每个受限判例配一条 **All-resolver 对照**(`TestApp::new()`,恒
//! `ScopeFilter::All`)—— 同一越权目标请求在恒 All 下 2xx,证明上面
//! 判例的 4xx 源于范围过滤,而非鉴权/参数/业务守卫等其他因素。

#![cfg(feature = "test-util")]

use admin::testkit::{
    AllScopeResolver, TestApp, assert_ok, code, user_with_perms,
};
use axum::http::StatusCode;
use serde_json::json;

/// 受限操作者:user_id=301,数据范围 DeptIn([101])—— 技术部可见,
/// 销售部(102)越权。
fn restricted_user(perms: &[&str]) -> app_core::model::login_user::LoginUser {
    user_with_perms(perms)
}

#[tokio::test]
async fn user_add_probe_rejects_foreign_dept_and_allows_in_scope() {
    // Java SysUserController:177 → checkDeptDataScope(user.getDeptId())
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:add"]);

    // 越权目标:新建用户挂到 102(不在操作者数据范围)→ 探针拒,
    // 404 NotFound(不泄露部门存在性)
    let (status, body) = app
        .post(
            "/system/user",
            json!({
                "userName": "probe_add_foreign",
                "nickName": "越权新增",
                "password": "p1234567",
                "deptId": 102,
            }),
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权部门应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");

    // 范围内目标:挂到 101 → 2xx(Java 基线放行)
    let (status, body) = app
        .post(
            "/system/user",
            json!({
                "userName": "probe_add_in_scope",
                "nickName": "范围内新增",
                "password": "p1234567",
                "deptId": 101,
            }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn user_edit_probe_rejects_foreign_dept_and_allows_in_scope() {
    // Java SysUserController:202 → checkDeptDataScope(user.getDeptId()):
    // 漏斗 update 的行级范围锚**旧行**,把用户改挂越权部门拦不住,
    // 目标部门必须显式探针(user_handler.rs 注释)。
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:edit"]);

    // 越权目标:编辑本人(301,旧行 dept 101 在范围内)但改挂 102 →
    // 探针先于 update 拒 → 404
    let (status, body) = app
        .put(
            "/system/user",
            json!({
                "userId": 301,
                "userName": "test_user",
                "nickName": "tester",
                "deptId": 102,
            }),
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权部门应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");

    // 范围内目标:deptId=101(探针 + 旧行都在范围内)→ 2xx
    let (status, body) = app
        .put(
            "/system/user",
            json!({
                "userId": 301,
                "userName": "test_user",
                "nickName": "tester_edited",
                "deptId": 101,
            }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn dept_edit_probe_rejects_foreign_dept_and_allows_in_scope() {
    // Java SysDeptController:106 → checkDeptDataScope(bo.deptId)探针
    // 居唯一性/守卫之前
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:dept:edit"]);

    // 越权目标:编辑 102(销售部,不在范围)→ 404
    let (status, body) = app
        .put(
            "/system/dept",
            json!({
                "deptId": 102,
                "parentId": 100,
                "deptName": "销售部",
                "orderNum": 2,
            }),
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权部门应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");

    // 范围内目标:编辑 101(技术部,不换父)→ 2xx
    let (status, body) = app
        .put(
            "/system/dept",
            json!({
                "deptId": 101,
                "parentId": 100,
                "deptName": "技术部",
                "orderNum": 1,
            }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn dept_remove_probe_rejects_foreign_dept_and_allows_in_scope() {
    // Java SysDeptController:143 → 探针居业务守卫(子部门/用户/岗位)
    // 之后;种子里 103/104 均无子部门、用户、岗位,守卫放行 → 探针可见。
    // 范围 DeptIn([101,103]):103 在内、104 在外。
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101, 103])),
    )
    .await;
    let user = restricted_user(&["system:dept:remove"]);

    // 越权目标:删 104(不在范围)→ 404
    let (status, body) = app.delete("/system/dept/104", Some(&user)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权部门应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");

    // 范围内目标:删 103(前端组)→ 2xx(软删)
    let (status, body) = app.delete("/system/dept/103", Some(&user)).await;
    assert_ok(status, &body);
}

// ============================================================
// All-resolver 对照 —— 同一"越权目标"请求在
// 恒 `ScopeFilter::All` 的 [`TestApp::new`] 下必须 2xx。与上面四个受限
// 判例互为对照:4xx 只能来自范围过滤,拆掉范围(恒 All)后同一请求
// 放行,即证明鉴权/参数/业务守卫均未参与拦截。
// ============================================================

#[tokio::test]
async fn user_add_probe_passes_under_all_resolver() {
    // 与 user_add_probe_rejects_foreign_dept_and_allows_in_scope 的越权
    // 目标同一请求(挂 102):恒 All 下探针放行 → 2xx
    let app = TestApp::new().await;
    let user = restricted_user(&["system:user:add"]);
    let (status, body) = app
        .post(
            "/system/user",
            json!({
                "userName": "probe_add_all_resolver",
                "nickName": "对照新增",
                "password": "p1234567",
                "deptId": 102,
            }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn user_edit_probe_passes_under_all_resolver() {
    // 与 user_edit_probe_rejects_foreign_dept_and_allows_in_scope 的越权
    // 目标同一请求(301 改挂 102):恒 All 下探针放行 → 2xx
    let app = TestApp::new().await;
    let user = restricted_user(&["system:user:edit"]);
    let (status, body) = app
        .put(
            "/system/user",
            json!({
                "userId": 301,
                "userName": "test_user",
                "nickName": "tester_all_resolver",
                "deptId": 102,
            }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn dept_edit_probe_passes_under_all_resolver() {
    // 与 dept_edit_probe_rejects_foreign_dept_and_allows_in_scope 的越权
    // 目标同一请求(编辑 102):恒 All 下探针放行 → 2xx
    let app = TestApp::new().await;
    let user = restricted_user(&["system:dept:edit"]);
    let (status, body) = app
        .put(
            "/system/dept",
            json!({
                "deptId": 102,
                "parentId": 100,
                "deptName": "销售部",
                "orderNum": 2,
            }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn dept_remove_probe_passes_under_all_resolver() {
    // 与 dept_remove_probe_rejects_foreign_dept_and_allows_in_scope 的越权
    // 目标同一请求(删 104):恒 All 下探针放行 → 2xx
    let app = TestApp::new().await;
    let user = restricted_user(&["system:dept:remove"]);
    let (status, body) = app.delete("/system/dept/104", Some(&user)).await;
    assert_ok(status, &body);
}

// ============================================================
// checkUserAllowed 先于数据范围
// 探针 —— Java SysUserController edit:200-201 / resetPwd:255-256 /
// changeStatus:272-273 均此序。受限操作者打超管靶标必须先回 400
// 「不允许操作超级管理员」,而非探针的 404。
// 用 DeptIn([101]) resolver:若无超管守卫前置,这些请求会先撞探针
// 404 —— 断言 400 即钉住顺序。
// ============================================================

#[tokio::test]
async fn user_edit_super_admin_target_rejected_before_scope_probe() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:edit"]);
    let (status, body) = app
        .put(
            "/system/user",
            json!({
                "userId": app_core::SUPER_ADMIN_USER_ID,
                "userName": "super_admin",
                "nickName": "超管靶标",
                "deptId": 101,
            }),
            Some(&user),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "超管靶标应先回 400 而非探针 404: {body}"
    );
    assert!(
        body.get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("不允许操作超级管理员"),
        "应提示不允许操作超级管理员: {body}"
    );
}

#[tokio::test]
async fn user_reset_pwd_super_admin_target_rejected_before_scope_probe() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:resetPwd"]);
    let (status, body) = app
        .put(
            "/system/user/resetPwd",
            json!({
                "userId": app_core::SUPER_ADMIN_USER_ID,
                "password": "newpass123",
            }),
            Some(&user),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "超管靶标应先回 400 而非探针 404: {body}"
    );
    assert!(
        body.get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("不允许操作超级管理员"),
        "应提示不允许操作超级管理员: {body}"
    );
}

#[tokio::test]
async fn user_change_status_super_admin_target_rejected_before_scope_probe() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:edit"]);
    let (status, body) = app
        .put(
            "/system/user/changeStatus",
            json!({
                "userId": app_core::SUPER_ADMIN_USER_ID,
                "status": "1",
            }),
            Some(&user),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "超管靶标应先回 400 而非探针 404: {body}"
    );
    assert!(
        body.get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("不允许操作超级管理员"),
        "应提示不允许操作超级管理员: {body}"
    );
}

// ============================================================
// 三处探针的 HTTP 级覆盖。
//
// 「越权被拦」的拦截点在 handler 探针:role 的四处探针接线与 user 的
// resetPwd/changeStatus/软删探针若没有 HTTP 判例 —— 接线漏了不会红。
// 本节按 Java 逐端点钉:受限操作者(301,DeptIn([101]))打越权靶标
// 必须 404;末尾一条 All-resolver 对照证明 4xx 源于范围过滤。
//
// 越权靶标:用户 302(部门 102,操作者范围外);角色 201(种子未设
// create_dept ⇒ NULL,`create_dept IN (101)` 恒不成立 ⇒ 范围外)。
// 范围内靶标:用户 901(部门 101)。
// ============================================================

/// Java SysUserServiceImpl.deleteUserByIds:531-534 —— 软删探针在
/// **service 层**(controller remove:221-228 只有「不能删自己」)。
/// 这是软删唯一的越权拦截。
#[tokio::test]
async fn user_remove_probe_rejects_foreign_dept_target() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:remove"]);

    let (status, body) = app.delete("/system/user/302", Some(&user)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权软删应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");

    // 范围内目标放行(证明 404 来自范围而非端点本身不可用)
    let (status, body) = app.delete("/system/user/901", Some(&user)).await;
    assert_ok(status, &body);
}

/// Java SysUserController.resetPwd:256 checkUserDataScope —— 越权**部门**
/// 靶标(非超管,故走得到探针;超管顺序判例见上一节)。
#[tokio::test]
async fn user_reset_pwd_probe_rejects_foreign_dept_target() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:resetPwd"]);

    let (status, body) = app
        .put(
            "/system/user/resetPwd",
            json!({ "userId": 302, "password": "newpass123" }),
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权改密应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");
}

/// Java SysUserController.changeStatus:273 checkUserDataScope —— 同上。
#[tokio::test]
async fn user_change_status_probe_rejects_foreign_dept_target() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:user:edit"]);

    let (status, body) = app
        .put(
            "/system/user/changeStatus",
            json!({ "userId": 302, "status": "1" }),
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "越权改状态应 404: {body}");
    assert_eq!(code(&body), 404, "信封 code 应同步 404: {body}");
}

/// role 的五处探针接线(Java SysRoleController getInfo:83 / edit:120 /
/// editPermission:146 / changeStatus:166 / selectAuthUserAll:266)——
/// 逐端点打越权角色 201,全部必须 404。任一处接线漏了或 role_id 传错,
/// 这条就红。
#[tokio::test]
async fn role_probes_reject_out_of_scope_role_on_every_endpoint() {
    let app = TestApp::new_with_resolver(
        AllScopeResolver::default()
            .with_user(301, app_data::ScopeFilter::DeptIn(vec![101])),
    )
    .await;
    let user = restricted_user(&["system:role:query", "system:role:edit"]);

    // ① getInfo
    let (status, body) = app.get("/system/role/201", Some(&user)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "getInfo 越权应 404: {body}");

    // ② edit
    let (status, body) = app
        .put(
            "/system/role",
            json!({
                "roleId": 201,
                "roleName": "普通角色",
                "roleKey": "common",
                "status": "0",
            }),
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "edit 越权应 404: {body}");

    // ③ editPermission
    let (status, body) = app
        .put(
            "/system/role/permission",
            json!({ "roleId": 201, "menuIds": [], "deptIds": [] }),
            Some(&user),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "editPermission 越权应 404: {body}"
    );

    // ④ changeStatus
    let (status, body) = app
        .put(
            "/system/role/changeStatus",
            json!({ "roleId": 201, "status": "1" }),
            Some(&user),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "changeStatus 越权应 404: {body}"
    );

    // ⑤ authUser/selectAll
    let (status, body) = app
        .put(
            "/system/role/authUser/selectAll?roleId=201&userIds=302",
            json!({}),
            Some(&user),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "authUser/selectAll 越权应 404: {body}"
    );
}

/// All-resolver 对照:同一批请求在恒 All 下不得是 404 —— 证明上面五条
/// 的 404 源于范围过滤,而非鉴权/参数/业务守卫。
#[tokio::test]
async fn role_probes_pass_under_all_resolver() {
    let app = TestApp::new().await;
    let user = restricted_user(&["system:role:query", "system:role:edit"]);

    let (status, body) = app.get("/system/role/201", Some(&user)).await;
    assert_ok(status, &body);

    let (status, body) = app
        .put(
            "/system/role/changeStatus",
            json!({ "roleId": 201, "status": "1" }),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);

    let (status, body) = app
        .put(
            "/system/role/authUser/selectAll?roleId=201&userIds=302",
            json!({}),
            Some(&user),
        )
        .await;
    assert_ok(status, &body);
}
