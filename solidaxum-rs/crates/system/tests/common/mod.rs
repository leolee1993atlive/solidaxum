//! 集成测试脚手架:建表走实体衍生 DDL
//! (`DataSource::create_table`),种子走免盖章 `insert_unstamped` ——
//! 审计列保持 NULL、del_flag 显式给值;本文件不出现 sea-orm。

use app_data::{
    DataSource, sea_orm::ActiveValue::Set, testing::sqlite_memory_empty,
};
use system::{
    assignee_sync::repository::entity::flow_assignee_sync,
    client::repository::client_repo,
    codegen::repository::{gen_column_repo, gen_table_repo},
    config::repository::config_repo,
    dept::repository::dept_repo,
    dict::repository::{dict_data_repo, dict_type_repo},
    menu::repository::menu_repo,
    message::repository::message_repo,
    notice::repository::notice_repo,
    oss::repository::{oss_config_repo, oss_repo},
    post::repository::post_repo,
    relation::{role_dept, role_menu, user_post, user_role},
    role::repository::role_repo,
    social::repository::social_repo,
    user::repository::user_repo,
};

/// 创建 SQLite 内存库:实体衍生 DDL 建 20 张表 + 免盖章种子。
/// 返回 `DataSource`。
// common 按测试 target 各自编译;scope_recursion_test 只消费
// setup_schema_and_seeds(自持连接造多快照),setup_db 在该 target 下未用。
#[allow(dead_code)]
pub async fn setup_db() -> DataSource {
    let ds = sqlite_memory_empty().await;
    setup_schema_and_seeds(&ds).await;
    ds
}

/// 建表 + 种子的主体。装配层测试需要自行持有母体(同一内存库上造多个
/// DataSource 快照,如 scope_recursion_test 的陷阱 resolver 双通道)时,
/// 经 `sqlite_memory_empty()` 建库后调本函数,快照经
/// `derive_with_resolver` 只读派生。
pub async fn setup_schema_and_seeds(ds: &DataSource) {
    ds.create_table(client_repo::Entity)
        .await
        .expect("建 sys_client 表");
    ds.create_table(user_repo::Entity)
        .await
        .expect("建 sys_user 表");
    ds.create_table(role_repo::Entity)
        .await
        .expect("建 sys_role 表");
    ds.create_table(menu_repo::Entity)
        .await
        .expect("建 sys_menu 表");
    ds.create_table(dept_repo::Entity)
        .await
        .expect("建 sys_dept 表");
    ds.create_table(user_role::Entity)
        .await
        .expect("建 sys_user_role 表");
    ds.create_table(role_menu::Entity)
        .await
        .expect("建 sys_role_menu 表");
    ds.create_table(role_dept::Entity)
        .await
        .expect("建 sys_role_dept 表");
    ds.create_table(post_repo::Entity)
        .await
        .expect("建 sys_post 表");
    ds.create_table(notice_repo::Entity)
        .await
        .expect("建 sys_notice 表");
    ds.create_table(dict_type_repo::Entity)
        .await
        .expect("建 sys_dict_type 表");
    ds.create_table(dict_data_repo::Entity)
        .await
        .expect("建 sys_dict_data 表");
    ds.create_table(config_repo::Entity)
        .await
        .expect("建 sys_config 表");
    ds.create_table(user_post::Entity)
        .await
        .expect("建 sys_user_post 表");
    ds.create_table(gen_table_repo::Entity)
        .await
        .expect("建 gen_table 表");
    ds.create_table(gen_column_repo::Entity)
        .await
        .expect("建 gen_table_column 表");
    ds.create_table(message_repo::Entity)
        .await
        .expect("建 sys_message 表");
    ds.create_table(social_repo::Entity)
        .await
        .expect("建 sys_social 表");
    ds.create_table(oss_repo::Entity)
        .await
        .expect("建 sys_oss 表");
    //参考实现 outbox 表:user/role 写路径挂钩点事务内写入,
    // 凡驱动这些写路径的测试都需要它存在
    ds.create_table(flow_assignee_sync::Entity)
        .await
        .expect("建 flow_assignee_sync 表");
    ds.create_table(oss_config_repo::Entity)
        .await
        .expect("建 sys_oss_config 表");

    let ctx = ds.system_context();

    // 部门
    for (id, parent, ancestors, name, order) in [
        (100, 0, "0", "总部", 0),
        (101, 100, "0,100", "技术部", 1),
        (102, 100, "0,100", "销售部", 2),
        (103, 101, "0,100,101", "前端组", 0),
        (104, 101, "0,100,101", "后端组", 1),
    ] {
        ctx.insert_unstamped(dept_repo::ActiveModel {
            dept_id: Set(id),
            parent_id: Set(parent),
            ancestors: Set(Some(ancestors.into())),
            dept_name: Set(name.into()),
            order_num: Set(order),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("seed dept");
    }

    // 角色(数据范围 1-6 各一)
    for (id, name, key, sort, scope) in [
        (200, "全部数据角色", "role_all", 0, "1"),
        (201, "自定义数据角色", "role_custom", 1, "2"),
        (202, "本部门角色", "role_dept", 2, "3"),
        (203, "本部门及以下角色", "role_dept_children", 3, "4"),
        (204, "仅本人角色", "role_self", 4, "5"),
        (205, "本部门及以下或本人角色", "role_dept_self", 5, "6"),
    ] {
        ctx.insert_unstamped(role_repo::ActiveModel {
            role_id: Set(id),
            role_name: Set(name.into()),
            role_key: Set(key.into()),
            role_sort: Set(sort),
            data_scope: Set(scope.into()),
            menu_check_strictly: Set(true),
            dept_check_strictly: Set(true),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("seed role");
    }

    // 用户(超管 + 各范围角色持有人 + 无角色/多角色 + 各部门靶标)
    for (id, dept_id, user_name, nick_name) in [
        (1761100000000000001_i64, 100, "super_admin", "超级管理员"),
        (301, 101, "u_all", "全部数据"),
        (302, 101, "u_custom", "自定义数据"),
        (303, 101, "u_dept", "本部门"),
        (304, 101, "u_dept_children", "本部门及以下"),
        (305, 101, "u_self", "仅本人"),
        (306, 101, "u_dept_self", "本部门及以下或本人"),
        (307, 101, "u_no_role", "无角色"),
        (308, 101, "u_multi", "多角色并集"),
        (401, 100, "target_hq", "总部用户"),
        (402, 101, "target_tech", "技术部用户"),
        (403, 103, "target_frontend", "前端组用户"),
        (404, 104, "target_backend", "后端组用户"),
    ] {
        ctx.insert_unstamped(user_repo::ActiveModel {
            user_id: Set(id),
            dept_id: Set(Some(dept_id)),
            user_name: Set(user_name.into()),
            nick_name: Set(nick_name.into()),
            password: Set("$2a$10$placeholder".into()),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("seed user");
    }

    // 用户↔角色
    for (user_id, role_id) in [
        (301, 200),
        (302, 201),
        (303, 202),
        (304, 203),
        (305, 204),
        (306, 205),
        (308, 201),
        (308, 203),
    ] {
        ctx.insert_unstamped(user_role::ActiveModel {
            user_id: Set(user_id),
            role_id: Set(role_id),
        })
        .await
        .expect("seed user_role");
    }

    // 菜单(status 仅 505 为停用;perms 仅业务菜单有)
    for (id, name, parent, order, path, menu_type, status, perms) in [
        (501, "系统管理", 0, 0, "system", "M", "0", None),
        (
            502,
            "用户管理",
            501,
            0,
            "user",
            "C",
            "0",
            Some("system:user:list"),
        ),
        (
            503,
            "用户新增",
            502,
            0,
            "add",
            "F",
            "0",
            Some("system:user:add"),
        ),
        (
            504,
            "角色管理",
            501,
            1,
            "role",
            "C",
            "0",
            Some("system:role:list"),
        ),
        (
            505,
            "禁用菜单",
            501,
            2,
            "disabled",
            "C",
            "1",
            Some("system:disabled"),
        ),
    ] {
        ctx.insert_unstamped(menu_repo::ActiveModel {
            menu_id: Set(id),
            menu_name: Set(name.into()),
            parent_id: Set(parent),
            order_num: Set(order),
            path: Set(path.into()),
            is_frame: Set("1".into()),
            is_cache: Set("0".into()),
            menu_type: Set(menu_type.into()),
            visible: Set("0".into()),
            status: Set(status.into()),
            perms: Set(perms.map(Into::into)),
            ..Default::default()
        })
        .await
        .expect("seed menu");
    }

    // 角色↔菜单 / 角色↔部门(自定义范围)
    for (role_id, menu_id) in [(201, 501), (201, 502), (201, 504)] {
        ctx.insert_unstamped(role_menu::ActiveModel {
            role_id: Set(role_id),
            menu_id: Set(menu_id),
        })
        .await
        .expect("seed role_menu");
    }
    for (role_id, dept_id) in [(201, 101), (201, 103)] {
        ctx.insert_unstamped(role_dept::ActiveModel {
            role_id: Set(role_id),
            dept_id: Set(dept_id),
        })
        .await
        .expect("seed role_dept");
    }

    // 岗位(del_flag 原靠 DDL DEFAULT '0',实体衍生 DDL 无默认值,显式给)
    for (id, dept_id, code, name, sort) in [
        (1761200000000000001_i64, 101, "ceo", "董事长", 1),
        (1761200000000000002_i64, 101, "pm", "项目经理", 2),
    ] {
        ctx.insert_unstamped(post_repo::ActiveModel {
            post_id: Set(id),
            dept_id: Set(dept_id),
            post_code: Set(code.into()),
            post_name: Set(name.into()),
            post_sort: Set(sort),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("seed post");
    }
    for (user_id, post_id) in [
        (302, 1761200000000000001_i64),
        (302, 1761200000000000002_i64),
    ] {
        ctx.insert_unstamped(user_post::ActiveModel {
            user_id: Set(user_id),
            post_id: Set(post_id),
        })
        .await
        .expect("seed user_post");
    }

    // 通知公告
    for (id, title, notice_type, content) in [
        (1761800000000000001_i64, "新版本上线通知", "2", "v2.0 发布"),
        (1761800000000000002_i64, "维护通知", "1", "周末维护"),
    ] {
        ctx.insert_unstamped(notice_repo::ActiveModel {
            notice_id: Set(id),
            notice_title: Set(title.into()),
            notice_type: Set(notice_type.into()),
            notice_content: Set(Some(content.into())),
            status: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("seed notice");
    }

    // 字典类型 / 字典数据
    for (id, name, dict_type) in [
        (1, "系统开关", "sys_normal_disable"),
        (2, "用户性别", "sys_user_gender"),
        (3, "系统是否", "sys_yes_no"),
    ] {
        ctx.insert_unstamped(dict_type_repo::ActiveModel {
            dict_id: Set(id),
            dict_name: Set(name.into()),
            dict_type: Set(dict_type.into()),
            ..Default::default()
        })
        .await
        .expect("seed dict_type");
    }
    for (code, sort, label, value, dict_type, is_default) in [
        (1, 1, "正常", "0", "sys_normal_disable", "Y"),
        (2, 2, "停用", "1", "sys_normal_disable", "N"),
        (3, 1, "男", "0", "sys_user_gender", "Y"),
        (4, 2, "女", "1", "sys_user_gender", "N"),
        (5, 1, "是", "Y", "sys_yes_no", "Y"),
        (6, 2, "否", "N", "sys_yes_no", "N"),
    ] {
        ctx.insert_unstamped(dict_data_repo::ActiveModel {
            dict_code: Set(code),
            dict_sort: Set(sort),
            dict_label: Set(label.into()),
            dict_value: Set(value.into()),
            dict_type: Set(dict_type.into()),
            is_default: Set(is_default.into()),
            ..Default::default()
        })
        .await
        .expect("seed dict_data");
    }

    // 参数配置
    for (id, name, key, value) in [
        (1, "用户初始密码", "sys.user.initPassword", "123456"),
        (2, "是否开启注册", "sys.account.registerUser", "false"),
    ] {
        ctx.insert_unstamped(config_repo::ActiveModel {
            config_id: Set(id),
            config_name: Set(name.into()),
            config_key: Set(key.into()),
            config_value: Set(value.into()),
            config_type: Set("Y".into()),
            ..Default::default()
        })
        .await
        .expect("seed config");
    }

    // OSS 记录(translation_service::oss_id_to_url 的查询靶标)
    for (id, file_name, suffix, service) in [
        (5001, "a.png", ".png", "minio"),
        (5002, "b.jpg", ".jpg", "minio"),
    ] {
        ctx.insert_unstamped(oss_repo::ActiveModel {
            oss_id: Set(id),
            file_name: Set(file_name.into()),
            original_name: Set(file_name.into()),
            file_suffix: Set(suffix.into()),
            url: Set(format!("/oss/{file_name}")),
            service: Set(service.into()),
            ..Default::default()
        })
        .await
        .expect("seed oss");
    }
}
