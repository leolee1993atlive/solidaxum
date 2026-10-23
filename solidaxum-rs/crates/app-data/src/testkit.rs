//! 业务测试套件共享的 DDL/种子脚本(设计 9.4:admin testkit DB 半部下沉)。
//!
//! app-data 不能依赖业务实体(范围声明:app-data 不认识 system/flow crate),
//! 这些 DDL/种子只能以**字符串形态**栖身于此 —— 它们描述的是业务表结构,
//! 由消费方(admin testkit,后续 flow tests)在自己持有的 DataSource 上执行。
//! 字符串自 `crates/admin/src/testkit.rs` **原样搬家,零语义改动**
//! (条数 22/18/14 经实数核实,见 `check-no-raw-sql` 白名单锚本文件)。
//!
//! `execute_raw`/`run_script` 是本 crate 内部合法持连的执行出口(收
//! `&DataSource`,经 crate 私有的
//! `DataSource::execute_raw_sql` 执行 —— 转发,不是 SQL 构造点),
//! `scripts/check-no-raw-sql.sh` 的白名单锚在本文件路径上。

use crate::DataSource;

/// 系统表 DDL(22 条,自 admin testkit 原样搬家)。
pub const SYSTEM_TABLE_DDL: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS sys_user (
        user_id INTEGER PRIMARY KEY, dept_id INTEGER, user_name TEXT NOT NULL,
        nick_name TEXT NOT NULL, user_type TEXT, email TEXT, phone_number TEXT,
        gender TEXT, avatar INTEGER, password TEXT NOT NULL, status TEXT NOT NULL,
        del_flag TEXT NOT NULL, login_ip TEXT, login_date TEXT, create_dept INTEGER,
        create_by INTEGER, create_time TEXT, update_by INTEGER, update_time TEXT,
        remark TEXT
    )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_role (
        role_id INTEGER PRIMARY KEY, role_name TEXT NOT NULL, role_key TEXT NOT NULL,
        parent_id INTEGER, role_sort INTEGER NOT NULL, data_scope TEXT NOT NULL,
        menu_check_strictly INTEGER NOT NULL, dept_check_strictly INTEGER NOT NULL,
        status TEXT NOT NULL, del_flag TEXT NOT NULL, create_dept INTEGER,
        create_by INTEGER, create_time TEXT, update_by INTEGER, update_time TEXT,
        remark TEXT
    )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_menu (
        menu_id INTEGER PRIMARY KEY, menu_name TEXT NOT NULL, parent_id INTEGER NOT NULL,
        order_num INTEGER NOT NULL, path TEXT NOT NULL, component TEXT, query_param TEXT,
        is_frame TEXT NOT NULL, is_cache TEXT NOT NULL, menu_type TEXT NOT NULL,
        visible TEXT NOT NULL, status TEXT NOT NULL, perms TEXT, icon TEXT,
        active_menu TEXT, ext TEXT, create_dept INTEGER, create_by INTEGER,
        create_time TEXT, update_by INTEGER, update_time TEXT, remark TEXT
    )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_dept (
        dept_id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL, ancestors TEXT,
        dept_name TEXT NOT NULL, dept_category TEXT, order_num INTEGER NOT NULL,
        leader INTEGER, phone TEXT, email TEXT, status TEXT NOT NULL, del_flag TEXT NOT NULL,
        create_dept INTEGER, create_by INTEGER, create_time TEXT, update_by INTEGER,
        update_time TEXT
    )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_post (
        post_id INTEGER PRIMARY KEY, dept_id INTEGER NOT NULL, post_code TEXT NOT NULL,
        post_category TEXT, post_name TEXT NOT NULL, post_sort INTEGER NOT NULL,
        status TEXT NOT NULL DEFAULT '0', del_flag TEXT NOT NULL DEFAULT '0',
        create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT, remark TEXT
    )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_user_role (user_id INTEGER NOT NULL, role_id INTEGER NOT NULL)"#,
    r#"CREATE TABLE IF NOT EXISTS sys_role_menu (role_id INTEGER NOT NULL, menu_id INTEGER NOT NULL)"#,
    r#"CREATE TABLE IF NOT EXISTS sys_role_dept (role_id INTEGER NOT NULL, dept_id INTEGER NOT NULL)"#,
    r#"CREATE TABLE IF NOT EXISTS sys_user_post (user_id INTEGER NOT NULL, post_id INTEGER NOT NULL)"#,
    r#"CREATE TABLE IF NOT EXISTS sys_notice (
        notice_id INTEGER PRIMARY KEY, notice_title TEXT NOT NULL, notice_type TEXT NOT NULL,
        notice_content TEXT, status TEXT NOT NULL DEFAULT '0', create_dept INTEGER,
        create_by INTEGER, create_time TEXT, update_by INTEGER, update_time TEXT, remark TEXT
    )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_client (
        id INTEGER PRIMARY KEY, client_id TEXT NOT NULL, client_key TEXT NOT NULL,
        client_secret TEXT NOT NULL, grant_type TEXT NOT NULL, device_type TEXT,
        access_path TEXT, ip_whitelist TEXT, active_timeout INTEGER NOT NULL,
        timeout INTEGER NOT NULL, status TEXT NOT NULL, del_flag TEXT NOT NULL,
        create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_social (
        id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, tenant_id TEXT,
        auth_id TEXT, source TEXT, open_id TEXT, user_name TEXT, nick_name TEXT,
        avatar TEXT, access_token TEXT, expire_in INTEGER, refresh_token TEXT,
        create_time TEXT, del_flag TEXT NOT NULL DEFAULT '0' )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_message (
        message_id INTEGER PRIMARY KEY, category TEXT NOT NULL,
        type TEXT NOT NULL, source TEXT NOT NULL, title TEXT, message TEXT,
        content TEXT, data_json TEXT, path TEXT,
        send_user_ids TEXT NOT NULL DEFAULT '0',
        create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_config (
        config_id INTEGER PRIMARY KEY, config_name TEXT NOT NULL,
        config_key TEXT NOT NULL, config_value TEXT NOT NULL,
        config_type TEXT NOT NULL DEFAULT 'N',
        create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT, remark TEXT )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_oss_config (
        oss_config_id INTEGER PRIMARY KEY, config_key TEXT NOT NULL,
        access_key TEXT NOT NULL, secret_key TEXT NOT NULL,
        bucket_name TEXT NOT NULL, prefix TEXT NOT NULL DEFAULT '',
        endpoint TEXT NOT NULL, domain_url TEXT NOT NULL DEFAULT '',
        is_https TEXT NOT NULL DEFAULT 'N', region TEXT NOT NULL DEFAULT '',
        status TEXT NOT NULL DEFAULT '0', access_policy TEXT NOT NULL DEFAULT '1',
        ext1 TEXT, create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT, remark TEXT )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_oss (
        oss_id INTEGER PRIMARY KEY, file_name TEXT NOT NULL,
        original_name TEXT NOT NULL, file_suffix TEXT NOT NULL,
        url TEXT NOT NULL, ext1 TEXT, create_dept INTEGER, create_by INTEGER,
        create_time TEXT, update_by INTEGER, update_time TEXT,
        service TEXT NOT NULL )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_oper_log (
        oper_id INTEGER PRIMARY KEY, title TEXT NOT NULL,
        business_type INTEGER NOT NULL, method TEXT, request_method TEXT NOT NULL,
        operator_type INTEGER NOT NULL, oper_name TEXT NOT NULL,
        dept_name TEXT, oper_url TEXT NOT NULL, oper_ip TEXT NOT NULL,
        oper_location TEXT NOT NULL, oper_param TEXT NOT NULL,
        json_result TEXT NOT NULL, status INTEGER NOT NULL,
        error_msg TEXT NOT NULL, oper_time TEXT, cost_time INTEGER NOT NULL,
        user_id INTEGER, dept_id INTEGER, client_key TEXT, device_type TEXT,
        browser TEXT, os TEXT )"#,
    r#"CREATE TABLE IF NOT EXISTS sys_login_info (
        info_id INTEGER PRIMARY KEY, user_name TEXT NOT NULL,
        client_key TEXT, device_type TEXT, ipaddr TEXT NOT NULL,
        login_location TEXT NOT NULL, browser TEXT NOT NULL, os TEXT NOT NULL,
        status TEXT NOT NULL, msg TEXT NOT NULL, login_time TEXT )"#,
    // 字典模块契约测试所需表(testkit 默认未建)
    r#"CREATE TABLE IF NOT EXISTS sys_dict_type (
        dict_id INTEGER PRIMARY KEY, dict_name TEXT, dict_type TEXT,
        create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT, remark TEXT)"#,
    r#"CREATE TABLE IF NOT EXISTS sys_dict_data (
        dict_code INTEGER PRIMARY KEY, dict_sort INTEGER, dict_label TEXT,
        dict_value TEXT, dict_type TEXT, css_class TEXT, list_class TEXT,
        is_default TEXT, create_dept INTEGER, create_by INTEGER, create_time TEXT,
        update_by INTEGER, update_time TEXT, remark TEXT)"#,
    // 雪花 ID 字符串化契约测试所需表(testkit 默认未建)
    r#"CREATE TABLE IF NOT EXISTS sys_job (
        job_id INTEGER PRIMARY KEY, job_name TEXT NOT NULL, job_group TEXT NOT NULL,
        invoke_target TEXT NOT NULL, cron_expression TEXT NOT NULL, job_params TEXT,
        misfire_policy INTEGER NOT NULL, concurrent INTEGER NOT NULL,
        status TEXT NOT NULL, remark TEXT, create_by TEXT, update_by TEXT,
        create_time TEXT, update_time TEXT, del_flag TEXT NOT NULL)"#,
    r#"CREATE TABLE IF NOT EXISTS sys_job_log (
        job_log_id INTEGER PRIMARY KEY, job_name TEXT NOT NULL, job_group TEXT NOT NULL,
        invoke_target TEXT NOT NULL, job_params TEXT, job_message TEXT,
        status TEXT NOT NULL, exception_info TEXT, start_time TEXT,
        stop_time TEXT, create_time TEXT)"#,
];

/// 系统种子(18 条,自 admin testkit 原样搬家)。
pub const SYSTEM_SEED: &[&str] = &[
    r#"INSERT INTO sys_dept (dept_id, parent_id, ancestors, dept_name, dept_category, leader, order_num, status, del_flag) VALUES
        (100, 0, '0', '总部', NULL, 1761100000000000001, 0, '0', '0'),
        (101, 100, '0,100', '技术部', NULL, NULL, 1, '0', '0'),
        (102, 100, '0,100', '销售部', NULL, NULL, 2, '0', '0'),
        (103, 101, '0,100,101', '前端组', 'fe_group', NULL, 0, '0', '0'),
        (104, 101, '0,100,101', '后端组', NULL, NULL, 1, '0', '0')"#,
    r#"INSERT INTO sys_role (role_id, role_name, role_key, role_sort, data_scope, menu_check_strictly, dept_check_strictly, status, del_flag) VALUES
        (200, '管理员', 'admin', 0, '1', 1, 1, '0', '0'),
        (201, '普通角色', 'common', 1, '2', 1, 1, '0', '0'),
        (202, '严格联动关', 'strict_off', 2, '2', 0, 0, '0', '0')"#,
    r#"INSERT INTO sys_user (user_id, dept_id, user_name, nick_name, password, status, del_flag, avatar) VALUES
        (1761100000000000001, 100, 'super_admin', 'admin', '$2a$10$placeholder', '0', '0', NULL),
        (301, 101, 'test_user', 'tester', '$2a$10$placeholder', '0', '0', 9001),
        (302, 102, 'sales_user', 'sales', '$2a$10$placeholder', '0', '0', NULL),
        (901, 101, 'flow_query_user', 'flow_query', '$2a$10$placeholder', '0', '0', NULL)"#,
    r#"INSERT INTO sys_user_role (user_id, role_id) VALUES (301, 201)"#,
    r#"INSERT INTO sys_menu (menu_id, menu_name, parent_id, order_num, path, is_frame, is_cache, menu_type, visible, status, perms) VALUES
        (501, '系统管理', 0, 0, 'system', '1', '0', 'M', '0', '0', NULL),
        (502, '用户管理', 501, 0, 'user', '1', '0', 'C', '0', '0', 'system:user:list'),
        (503, '用户新增', 502, 0, '', '1', '0', 'F', '0', '0', 'system:user:add'),
        (504, '角色管理', 501, 1, 'role', '1', '0', 'C', '0', '0', 'system:role:list')"#,
    r#"INSERT INTO sys_role_menu (role_id, menu_id) VALUES (201, 501), (201, 502), (201, 504)"#,
    r#"INSERT INTO sys_post (post_id, dept_id, post_code, post_name, post_sort, status) VALUES
        (1761200000000000001, 101, 'ceo', '董事长', 1, '0'),
        (1761200000000000002, 101, 'se', '工程师', 2, '0')"#,
    r#"INSERT INTO sys_oss (oss_id, file_name, original_name, file_suffix, url, service, create_by) VALUES
        (9001, 'avatar.png', 'avatar.png', '.png', 'https://example.com/avatar/9001.png', 'local', 1761100000000000001)"#,
    r#"INSERT INTO sys_notice (notice_id, notice_title, notice_type, notice_content, status, create_by, create_time) VALUES
        (1761800000000000001, '版本上线', '2', 'v2.0', '0', 1761100000000000001, datetime('now')),
        (1761800000000000002, '维护通知', '1', '周末维护', '0', 301, datetime('now', '-1 day'))"#,
    r#"INSERT INTO sys_client (id, client_id, client_key, client_secret, grant_type, active_timeout, timeout, status, del_flag) VALUES
        (1, 'e5cd7e4891bf95d1d19206ce24a7b32e', 'pc', 'pc123', 'password', 1800, 604800, '0', '0')"#,
    r#"INSERT INTO sys_social (id, user_id, tenant_id, auth_id, source, open_id, user_name, nick_name, avatar, access_token, expire_in, refresh_token, create_time, del_flag) VALUES
        (1, 301, NULL, 'auth_301_github', 'github', 'gh_open_301', 'test_user', 'tester', 'https://example.com/avatar.png', 'token_301', 3600, 'refresh_301', datetime('now'), '0'),
        (2, 301, NULL, 'auth_301_gitee', 'gitee', 'gt_open_301', 'test_user', 'tester', NULL, 'token_302', 3600, 'refresh_302', datetime('now'), '0')"#,
    r#"INSERT INTO sys_message (message_id, category, type, source, title, message, content, data_json, path, send_user_ids, create_time) VALUES
        (1, 'system', 'MESSAGE', 'BACKEND', '系统消息', '欢迎使用系统', NULL, NULL, NULL, '0', datetime('now')),
        (2, 'notice', 'NOTICE', 'NOTICE', '通知公告消息', '系统维护通知', NULL, NULL, NULL, '0', datetime('now'))"#,
    // 注册开关默认关闭
    r#"INSERT INTO sys_config (config_id, config_name, config_key, config_value, config_type) VALUES
        (1, '用户初始密码', 'sys.user.initPassword', '123456', 'Y'),
        (2, '是否开启注册', 'sys.account.registerUser', 'false', 'Y'),
        (3, '系统信息-系统标题', 'sys.title', 'SolidAxum', 'Y'),
        (4, '系统信息-系统图标', 'sys.icon', '', 'Y')"#,
    // 字典模块种子(dictType 用合法格式 ^[a-z][a-z0-9_]*$)
    r#"INSERT INTO sys_dict_type (dict_id, dict_name, dict_type, remark) VALUES
        (8001, '通用状态', 'sys_normal_disable', '通用状态字典'),
        (8002, '用户性别', 'sys_user_gender', '性别字典'),
        (8003, '业务状态', 'wf_business_status', '业务状态列表'),
        (8004, '任务状态', 'wf_task_status', '任务状态')"#,
    r#"INSERT INTO sys_dict_data (dict_code, dict_sort, dict_label, dict_value, dict_type, is_default) VALUES
        (8101, 1, '男', '0', 'sys_user_gender', 'Y'),
        (8102, 2, '女', '1', 'sys_user_gender', 'N'),
        (8103, 1, '正常', '0', 'sys_normal_disable', 'Y'),
        (8104, 2, '停用', '1', 'sys_normal_disable', 'N')"#,
    // wf_business_status / wf_task_status 字典种子(对齐 RuoYi 原版)
    r#"INSERT INTO sys_dict_data (dict_code, dict_sort, dict_label, dict_value, dict_type, css_class, list_class, is_default) VALUES
        (8105, 1, '已撤销', 'cancel', 'wf_business_status', '', 'danger', 'N'),
        (8106, 2, '草稿', 'draft', 'wf_business_status', '', 'info', 'N'),
        (8107, 3, '待审核', 'waiting', 'wf_business_status', '', 'primary', 'N'),
        (8108, 4, '已完成', 'finish', 'wf_business_status', '', 'success', 'N'),
        (8109, 5, '已作废', 'invalid', 'wf_business_status', '', 'danger', 'N'),
        (8110, 6, '已退回', 'back', 'wf_business_status', '', 'danger', 'N'),
        (8111, 7, '已终止', 'termination', 'wf_business_status', '', 'danger', 'N'),
        (8112, 1, '撤销', 'cancel', 'wf_task_status', '', 'danger', 'N'),
        (8113, 2, '通过', 'pass', 'wf_task_status', '', 'success', 'N'),
        (8114, 3, '待审核', 'waiting', 'wf_task_status', '', 'primary', 'N'),
        (8115, 4, '作废', 'invalid', 'wf_task_status', '', 'danger', 'N'),
        (8116, 5, '退回', 'back', 'wf_task_status', '', 'danger', 'N'),
        (8117, 6, '终止', 'termination', 'wf_task_status', '', 'danger', 'N'),
        (8118, 7, '转办', 'transfer', 'wf_task_status', '', 'primary', 'N'),
        (8119, 8, '委托', 'depute', 'wf_task_status', '', 'primary', 'N'),
        (8120, 9, '抄送', 'copy', 'wf_task_status', '', 'primary', 'N'),
        (8121, 10, '加签', 'sign', 'wf_task_status', '', 'primary', 'N'),
        (8122, 11, '减签', 'sign_off', 'wf_task_status', '', 'danger', 'N'),
        (8123, 11, '超时', 'timeout', 'wf_task_status', '', 'danger', 'N')"#,
    r#"INSERT INTO sys_job (job_id, job_name, job_group, invoke_target, cron_expression,
        misfire_policy, concurrent, status, del_flag) VALUES
        (1761300000000000001, '雪花任务', 'DEFAULT', 'demo', '0/5 * * * * ?', 1, 0, '1', '0'),
        (42, '小id任务', 'DEFAULT', 'demo', '0/5 * * * * ?', 1, 0, '1', '0')"#,
    r#"INSERT INTO sys_job_log (job_log_id, job_name, job_group, invoke_target, status) VALUES
        (7, '小id任务', 'DEFAULT', 'demo', '1')"#,
];

/// flow 种子(14 条,自 admin testkit 原样搬家)。**依赖实体衍生表**
/// (flow_* / test_leave 由 `Schema::create_table_from_entity` 建),
/// 本 crate 的自测只覆盖 DDL/系统种子,flow 种子的可执行性由
/// admin 契约测试锚。
pub const FLOW_SEED: &[&str] = &[
    r#"INSERT INTO flow_category (category_id, parent_id, ancestors, category_name, order_num, del_flag, create_dept, create_by, create_time) VALUES
        (1, 0, '0', '默认', 0, '0', NULL, 301, datetime('now')),
        (2, 0, '0', '采购', 1, '0', NULL, 301, datetime('now'))"#,
    // leave1 审批流程: start -> approve -> end
    // category 存 categoryId（Java 语义），指向 flow_category.category_id=1
    r#"INSERT INTO flow_definition (id, flow_code, flow_name, model_value, category, version, is_publish, form_custom, form_path, activity_status, listener_type, listener_path, ext, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000001, 'leave1', '请假流程', '{\"flowCode\":\"leave1\",\"flowName\":\"请假流程\",\"version\":\"1\",\"nodeList\":[]}', '1', '1', 1, NULL, NULL, 1, NULL, NULL, NULL, datetime('now'), '301', datetime('now'), '301', '0', NULL)"#,
    r#"INSERT INTO flow_node (id, node_type, definition_id, node_code, node_name, permission_flag, node_ratio, coordinate, any_node_skip, listener_type, listener_path, form_custom, form_path, version, create_time, create_by, update_time, update_by, ext, del_flag, tenant_id) VALUES
        (9000000000000000011, 0, 9000000000000000001, 'start', '开始', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', datetime('now'), '301', datetime('now'), '301', NULL, '0', NULL),
        (9000000000000000012, 1, 9000000000000000001, 'approve', '审批', '301', NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', datetime('now'), '301', datetime('now'), '301', NULL, '0', NULL),
        (9000000000000000013, 2, 9000000000000000001, 'end', '结束', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', datetime('now'), '301', datetime('now'), '301', NULL, '0', NULL)"#,
    // 注意:flow_skip 实体无 node_id 列(#[sea_orm(ignore)]),create_table_from_entity 建出的表
    // 也无该列。种子 INSERT 若列出 node_id 会整条失败(被 .ok() 吞掉)→ leave1 无跳转边
    // → startWorkFlow 报「开始节点后无有效跳转」。故此处不得包含 node_id。
    r#"INSERT INTO flow_skip (id, definition_id, now_node_code, now_node_type, next_node_code, next_node_type, skip_name, skip_type, skip_condition, coordinate, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000021, 9000000000000000001, 'start', 0, 'approve', 1, NULL, 'PASS', NULL, NULL, datetime('now'), '301', datetime('now'), '301', '0', NULL),
        (9000000000000000022, 9000000000000000001, 'approve', 1, 'end', 2, NULL, 'PASS', NULL, NULL, datetime('now'), '301', datetime('now'), '301', '0', NULL)"#,
    // flow_spel 种子：对齐新 schema（component_name/method_name/method_params/view_spel/remark/status）
    r#"INSERT INTO flow_spel (id, component_name, method_name, method_params, view_spel, remark, status, del_flag, create_dept, create_by, create_time, update_by, update_time) VALUES
        (9000000000000000002, 'spelRuleComponent', 'selectDeptLeaderById', 'initiatorDeptId', '#{@spelRuleComponent.selectDeptLeaderById(#initiatorDeptId)}', '根据部门id获取负责人', '0', '0', NULL, 301, datetime('now'), 301, datetime('now'))"#,
    // --- TaskPageQuery 参数面 contract 测试用种子 ---
    // 第二个流程定义,用于 flowCode/flowName 过滤;category=2 用于实例分类过滤
    r#"INSERT INTO flow_definition (id, flow_code, flow_name, model_value, category, version, is_publish, form_custom, form_path, activity_status, listener_type, listener_path, ext, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000002, 'purchase1', '采购流程', '{"flowCode":"purchase1","flowName":"采购流程","version":"1","nodeList":[]}', '2', '1', 1, NULL, NULL, 1, NULL, NULL, NULL, datetime('now'), '901', datetime('now'), '901', '0', NULL)"#,
    r#"INSERT INTO flow_node (id, node_type, definition_id, node_code, node_name, permission_flag, node_ratio, coordinate, any_node_skip, listener_type, listener_path, form_custom, form_path, version, create_time, create_by, update_time, update_by, ext, del_flag, tenant_id) VALUES
        (9000000000000000101, 0, 9000000000000000002, 'start', '开始', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL),
        (9000000000000000102, 1, 9000000000000000002, 'approve', '审批', '901', NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL),
        (9000000000000000103, 2, 9000000000000000002, 'end', '结束', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL)"#,
    r#"INSERT INTO flow_skip (id, definition_id, now_node_code, now_node_type, next_node_code, next_node_type, skip_name, skip_type, skip_condition, coordinate, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000104, 9000000000000000002, 'start', 0, 'approve', 1, NULL, 'PASS', NULL, NULL, datetime('now'), '901', datetime('now'), '901', '0', NULL),
        (9000000000000000105, 9000000000000000002, 'approve', 1, 'end', 2, NULL, 'PASS', NULL, NULL, datetime('now'), '901', datetime('now'), '901', '0', NULL)"#,
    // 三个运行中实例,create_by 分别为 901 / 302 / 901,用于 createByIds 过滤
    // 203 节点名改为'复核',与 201 区分,便于 pageByCurrent nodeName 过滤断言
    r#"INSERT INTO flow_instance (id, definition_id, business_id, node_type, node_code, node_name, variable, flow_status, activity_status, def_json, create_time, create_by, update_time, update_by, ext, del_flag, tenant_id) VALUES
        (9000000000000000201, 9000000000000000001, 'biz_wait_1', 1, 'approve', '审批', '{}', '1', 1, NULL, datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL),
        (9000000000000000202, 9000000000000000001, 'biz_wait_2', 1, 'audit', '审计', '{}', '1', 1, NULL, datetime('now'), '302', datetime('now'), '302', NULL, '0', NULL),
        (9000000000000000203, 9000000000000000002, 'biz_wait_3', 1, 'approve', '复核', '{}', '1', 1, NULL, datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL)"#,
    // 三个已完成实例,分别属于 category=1(leave1) / category=2(purchase1) / category=1(leave1,create_by=302)
    r#"INSERT INTO flow_instance (id, definition_id, business_id, node_type, node_code, node_name, variable, flow_status, activity_status, def_json, create_time, create_by, update_time, update_by, ext, del_flag, tenant_id) VALUES
        (9000000000000000204, 9000000000000000001, 'biz_finish_1', 2, 'end', '结束', '{}', '8', 1, NULL, datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL),
        (9000000000000000205, 9000000000000000002, 'biz_finish_2', 2, 'end', '归档', '{}', '8', 1, NULL, datetime('now'), '901', datetime('now'), '901', NULL, '0', NULL),
        (9000000000000000206, 9000000000000000001, 'biz_finish_3', 2, 'end', '结束', '{}', '8', 1, NULL, datetime('now'), '302', datetime('now'), '302', NULL, '0', NULL)"#,
    // 三个待办任务,node_name 不同、definition 不同。
    // **flow_task.create_by 刻意与所属实例的 create_by 错开**:它记录的是「创建该任务行
    // 的操作者」(首节点之后即上一个审批人),不是申请人。若种子让两者一致,
    // createByIds 过滤走错列也能蒙混过关(历史上正是如此)。
    //   301 → 实例 201(申请人 901),任务行由 302 创建
    //   302 → 实例 202(申请人 302),任务行由 901 创建
    //   303 → 实例 203(申请人 901),任务行由 302 创建
    r#"INSERT INTO flow_task (id, definition_id, instance_id, node_code, node_name, node_type, flow_status, form_custom, form_path, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000301, 9000000000000000001, 9000000000000000201, 'approve', '审批', 1, '1', NULL, NULL, datetime('now'), '302', datetime('now'), '302', '0', NULL),
        (9000000000000000302, 9000000000000000001, 9000000000000000202, 'audit', '审计', 1, '1', NULL, NULL, datetime('now'), '901', datetime('now'), '901', '0', NULL),
        (9000000000000000303, 9000000000000000002, 9000000000000000203, 'approve', '复核', 1, '1', NULL, NULL, datetime('now'), '302', datetime('now'), '302', '0', NULL)"#,
    // 我的待办办理人(type=1): 901 是 task 301 / 302 / 303 的办理人。
    // 其中 302 所属实例由 302 发起 —— 这样「我的待办 + createByIds」筛选才有区分度。
    r#"INSERT INTO flow_user (id, type, processed_by, associated, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000401, '1', '901', 9000000000000000301, datetime('now'), '901', datetime('now'), '901', '0', NULL),
        (9000000000000000402, '1', '901', 9000000000000000303, datetime('now'), '901', datetime('now'), '901', '0', NULL),
        (9000000000000000403, '1', '901', 9000000000000000302, datetime('now'), '901', datetime('now'), '901', '0', NULL)"#,
    // 抄送人(type=4): 901 能收到 task 301 / 302 / 303 的抄送
    r#"INSERT INTO flow_user (id, type, processed_by, associated, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
        (9000000000000000501, '4', '901', 9000000000000000301, datetime('now'), '901', datetime('now'), '901', '0', NULL),
        (9000000000000000502, '4', '901', 9000000000000000302, datetime('now'), '302', datetime('now'), '302', '0', NULL),
        (9000000000000000503, '4', '901', 9000000000000000303, datetime('now'), '901', datetime('now'), '901', '0', NULL)"#,
    // 三条已办历史: 901 办理,节点名不同,分别对应 leave1/purchase1 两个流程
    r#"INSERT INTO flow_his_task (id, definition_id, instance_id, task_id, node_code, node_name, node_type, target_node_code, target_node_name, approver, cooperate_type, collaborator, skip_type, flow_status, form_custom, form_path, message, variable, ext, create_time, update_time, del_flag, tenant_id) VALUES
        (9000000000000000601, 9000000000000000001, 9000000000000000201, 9000000000000000301, 'start', '开始', 0, 'approve', '审批', '901', 1, NULL, 'PASS', 'PASS', NULL, NULL, NULL, '{}', NULL, datetime('now'), datetime('now'), '0', NULL),
        (9000000000000000602, 9000000000000000001, 9000000000000000202, 9000000000000000302, 'approve', '审批', 1, 'end', '结束', '901', 1, NULL, 'PASS', 'PASS', NULL, NULL, NULL, '{}', NULL, datetime('now'), datetime('now'), '0', NULL),
        (9000000000000000603, 9000000000000000002, 9000000000000000203, 9000000000000000303, 'approve', '审批', 1, 'end', '结束', '901', 1, NULL, 'PASS', 'PASS', NULL, NULL, NULL, '{}', NULL, datetime('now'), datetime('now'), '0', NULL)"#,
];

/// 在持有方给的 DataSource 上执行一条原生 SQL(DDL/种子形态)。
///
/// 错误原样透传 `DbErr`,不吞不包 —— 种子的「严格断言」语义靠调用方
/// `.expect(...)` 保持(同 admin testkit 的既有处置)。
pub async fn execute_raw(
    ds: &DataSource,
    sql: &str,
) -> Result<(), sea_orm::DbErr> {
    ds.execute_raw_sql(sql).await?;
    Ok(())
}

/// 顺序执行一组原生 SQL,任一失败即停并透传错误。
pub async fn run_script(
    ds: &DataSource,
    stmts: &[&str],
) -> Result<(), sea_orm::DbErr> {
    for stmt in stmts {
        execute_raw(ds, stmt).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 条数钉桩:与搬家源 `crates/admin/src/testkit.rs` 的 DDL(22)/
    /// 系统种子(18)/flow 种子(14)一致。计数经 awk 实数核实。
    #[test]
    fn script_counts_are_pinned() {
        assert_eq!(SYSTEM_TABLE_DDL.len(), 22, "系统表 DDL 条数");
        assert_eq!(SYSTEM_SEED.len(), 18, "系统种子条数");
        assert_eq!(FLOW_SEED.len(), 14, "flow 种子条数");
    }

    /// 字符串脚本不是死数据:DDL + 系统种子在 SQLite 内存库上真实可执行,
    /// 且种子真的落库 —— 重复插入 sys_user 主键 301 必须撞唯一约束
    /// (fork 的 ConnectionTrait 查询口只收 StatementBuilder,断言不引
    /// 新面孔,复用 execute_raw 的报错透传即可证行存在)。
    /// (FLOW_SEED 依赖实体衍生表,不在此断言 —— 它的可执行性由
    /// admin 契约测试锚。)
    #[tokio::test]
    async fn run_script_executes_on_sqlite_memory() {
        let ds = crate::testing::sqlite_memory_empty().await;
        run_script(&ds, SYSTEM_TABLE_DDL)
            .await
            .expect("DDL 脚本必须可执行");
        run_script(&ds, SYSTEM_SEED)
            .await
            .expect("系统种子必须可执行");
        execute_raw(
            &ds,
            "INSERT INTO sys_user (user_id, user_name, nick_name, password, \
             status, del_flag) VALUES (301, 'x', 'x', 'x', '0', '0')",
        )
        .await
        .expect_err("user_id=301 已在种子里,重复插入必须撞主键冲突");
    }

    /// execute_raw 的错误必须原样透传为 DbErr(在坏表上执行应 Err)。
    #[tokio::test]
    async fn execute_raw_propagates_db_err() {
        let ds = crate::testing::sqlite_memory_empty().await;
        let err = execute_raw(&ds, "INSERT INTO no_such_table VALUES (1)")
            .await
            .expect_err("坏 SQL 必须报错,不得静默吞掉");
        let _: sea_orm::DbErr = err;
    }
}
