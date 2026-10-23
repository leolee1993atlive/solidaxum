-- 系统信息(标题/图标)配置 —— 2026-10-13
-- 功能:顶栏标题 + 侧栏图标运行时可配(锚:原 Java 版系统信息页)。
-- 载体:sys_config 两键(sys.title/sys.icon,icon 为 OSS URL);
-- 端点 GET /system/config/sysInfo(仅需登录),保存走既有 updateByKey。
-- 注:MySQL 种子里另有 Java 版移植的 sys_info_config 表,当前无代码消费,
--     功能由 sys_config 承载(轻量、可回流脚手架);该表暂保留不启用。

-- 1. 参数两行(已存在则跳过;幂等靠主键冲突忽略)
INSERT IGNORE INTO `sys_config`
    (`config_id`, `config_name`, `config_key`, `config_value`, `config_type`,
     `create_dept`, `create_by`, `create_time`, `remark`)
VALUES
    (1761700000000000004, '系统信息-系统标题', 'sys.title', 'SolidAxum', 'Y',
     1761000000000000103, 1761100000000000001, '2026-10-13 10:00:00',
     '顶栏标题(医院全称+系统名),系统信息页可改'),
    (1761700000000000005, '系统信息-系统图标', 'sys.icon', '', 'Y',
     1761000000000000103, 1761100000000000001, '2026-10-13 10:00:00',
     '侧栏顶部图标 URL(OSS 上传),系统信息页可改');

-- 2. 菜单:系统管理 → 系统信息(编辑权限 system:config:edit)
INSERT IGNORE INTO `sys_menu`
    (`menu_id`, `menu_name`, `parent_id`, `order_num`, `path`, `component`,
     `query_param`, `is_frame`, `is_cache`, `menu_type`, `visible`, `status`,
     `perms`, `icon`, `active_menu`, `ext`, `create_dept`, `create_by`,
     `create_time`, `remark`)
VALUES
    (1761400000000000150, '系统信息', 1761400000000000001, 20, 'sysinfo',
     'system/sysinfo/index', '', '1', '0', 'C', '0', '0',
     'system:config:edit', 'form', '', '', 1761000000000000103,
     1761100000000000001, '2026-10-13 10:00:00', '系统信息菜单(标题/图标)');
