-- =====================================================================
-- solidaxum_init.sql — solidaxum 初始化 SQL(全新部署)
-- =====================================================================
-- 来源: 测试库全量导出整理(原 DBX 导出于 2026-07-31), 已清理开发期产生的
--       运行时数据(sys_oper_log / sys_login_info / sys_message /
--       flow_* 运行时实例)与迁移残留表(sys_menu_backup)。
-- 用途: 全新部署唯一初始化脚本 —— 含建库、建表(38 张)与基础种子数据。
--       原 RuoYi ry_vue.sql 与开发期各补丁(sys_menu_seed / sys_job /
--       seed_dict_wf / sys_oss_config_local 等)的内容均已
--       吸收进本文件, 无需再单独执行任何旧补丁。
-- 执行: mysql -u root -p --default-character-set=utf8mb4 < sql/solidaxum_init.sql
--       (文件内已 SET NAMES 兜底; 不加该参数、经由 latin1 默认连接的客户端
--       —— 如 mysql:8.0 官方镜像的 docker-entrypoint initdb —— 也能正确导入)
-- 重置: 本脚本幂等可重入(各表 DROP TABLE IF EXISTS + CREATE), 重复执行
--       会清空并重建全部表; 谨慎用于生产。
-- OSS : 默认 provider 为 local(本地磁盘, endpoint='file://./data/oss',
--       status='Y'); minio/qiniu/aliyun/qcloud/image 配置保留但 status='N',
--       需要切换时在 OSS 配置页启用对应行。
-- =====================================================================

CREATE DATABASE IF NOT EXISTS `ruoyi` CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;
USE `ruoyi`;

# 兜底: 会话字符集钉死 utf8mb4。mysql CLI 在无 LANG 的容器里默认 latin1,
# UTF-8 中文字面量会被按 latin1 读入再转 utf8mb4 存储(双重编码乱码)。
SET NAMES utf8mb4;

SET FOREIGN_KEY_CHECKS = 0;

DROP TABLE IF EXISTS `test_tree`;

CREATE TABLE `test_tree` (
  `id` bigint NOT NULL COMMENT '主键',
  `parent_id` bigint DEFAULT '0' COMMENT '父id',
  `dept_id` bigint DEFAULT NULL COMMENT '部门id',
  `user_id` bigint DEFAULT NULL COMMENT '用户id',
  `tree_name` varchar(255) DEFAULT NULL COMMENT '值',
  `version` int DEFAULT '0' COMMENT '版本',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` bigint DEFAULT NULL COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新人',
  `del_flag` int DEFAULT '0' COMMENT '删除标志',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='测试树表';

INSERT INTO `test_tree` (`id`, `parent_id`, `dept_id`, `user_id`, `tree_name`, `version`, `create_dept`, `create_time`, `create_by`, `update_time`, `update_by`, `del_flag`) VALUES
(1762200000000000001, 0, 1761000000000000102, 1761100000000000004, '测试数据权限', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000002, 1762200000000000001, 1761000000000000102, 1761100000000000003, '子节点1', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000003, 1762200000000000002, 1761000000000000102, 1761100000000000003, '子节点2', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000004, 0, 1761000000000000108, 1761100000000000004, '测试树1', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000005, 1762200000000000004, 1761000000000000108, 1761100000000000003, '子节点11', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000006, 1762200000000000004, 1761000000000000108, 1761100000000000003, '子节点22', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000007, 1762200000000000004, 1761000000000000108, 1761100000000000003, '子节点33', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762200000000000008, 1762200000000000005, 1761000000000000108, 1761100000000000003, '子节点44', 0, 1761000000000000103, '2026-06-14 15:36:19', 1761100000000000001, NULL, NULL, 0),
(1762200000000000009, 1762200000000000006, 1761000000000000108, 1761100000000000003, '子节点55', 0, 1761000000000000103, '2026-06-14 15:36:19', 1761100000000000001, NULL, NULL, 0),
(1762200000000000010, 1762200000000000007, 1761000000000000108, 1761100000000000003, '子节点66', 0, 1761000000000000103, '2026-06-14 15:36:19', 1761100000000000001, NULL, NULL, 0),
(1762200000000000011, 1762200000000000007, 1761000000000000108, 1761100000000000003, '子节点77', 0, 1761000000000000103, '2026-06-14 15:36:19', 1761100000000000001, NULL, NULL, 0),
(1762200000000000012, 1762200000000000010, 1761000000000000108, 1761100000000000003, '子节点88', 0, 1761000000000000103, '2026-06-14 15:36:19', 1761100000000000001, NULL, NULL, 0),
(1762200000000000013, 1762200000000000010, 1761000000000000108, 1761100000000000003, '子节点99', 0, 1761000000000000103, '2026-06-14 15:36:19', 1761100000000000001, NULL, NULL, 0);

DROP TABLE IF EXISTS `sys_oss`;

CREATE TABLE `sys_oss` (
  `oss_id` bigint NOT NULL COMMENT '对象存储主键',
  `file_name` varchar(255) NOT NULL DEFAULT '' COMMENT '文件名',
  `original_name` varchar(255) NOT NULL DEFAULT '' COMMENT '原名',
  `file_suffix` varchar(10) NOT NULL DEFAULT '' COMMENT '文件后缀名',
  `url` varchar(500) NOT NULL COMMENT 'URL地址',
  `ext1` text COMMENT '扩展字段',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` bigint DEFAULT NULL COMMENT '上传人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新人',
  `service` varchar(20) NOT NULL DEFAULT 'minio' COMMENT '服务商',
  PRIMARY KEY (`oss_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='OSS对象存储表';

DROP TABLE IF EXISTS `sys_login_info`;

CREATE TABLE `sys_login_info` (
  `info_id` bigint NOT NULL COMMENT '访问ID',
  `user_name` varchar(50) DEFAULT '' COMMENT '用户账号',
  `client_key` varchar(32) DEFAULT '' COMMENT '客户端',
  `device_type` varchar(32) DEFAULT '' COMMENT '设备类型',
  `ipaddr` varchar(128) DEFAULT '' COMMENT '登录IP地址',
  `login_location` varchar(255) DEFAULT '' COMMENT '登录地点',
  `browser` varchar(50) DEFAULT '' COMMENT '浏览器类型',
  `os` varchar(50) DEFAULT '' COMMENT '操作系统',
  `status` char(1) DEFAULT '0' COMMENT '登录状态（0正常 1异常）',
  `msg` varchar(255) DEFAULT '' COMMENT '提示消息',
  `login_time` datetime DEFAULT NULL COMMENT '访问时间',
  PRIMARY KEY (`info_id`),
  KEY `idx_sys_login_info_s` (`status`),
  KEY `idx_sys_login_info_lt` (`login_time`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='系统访问记录';


DROP TABLE IF EXISTS `sys_user_role`;

CREATE TABLE `sys_user_role` (
  `user_id` bigint NOT NULL COMMENT '用户ID',
  `role_id` bigint NOT NULL COMMENT '角色ID',
  PRIMARY KEY (`user_id`,`role_id`),
  KEY `idx_sys_user_role_rid` (`role_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='用户和角色关联表';

INSERT INTO `sys_user_role` (`user_id`, `role_id`) VALUES
(1761100000000000001, 1761300000000000001),
(1761100000000000003, 1761300000000000003),
(1761100000000000004, 1761300000000000004);

DROP TABLE IF EXISTS `sys_dept`;

CREATE TABLE `sys_dept` (
  `dept_id` bigint NOT NULL COMMENT '部门id',
  `parent_id` bigint DEFAULT '0' COMMENT '父部门id',
  `ancestors` varchar(500) DEFAULT '' COMMENT '祖级列表',
  `dept_name` varchar(30) DEFAULT '' COMMENT '部门名称',
  `dept_category` varchar(100) DEFAULT NULL COMMENT '部门类别编码',
  `order_num` int DEFAULT '0' COMMENT '显示顺序',
  `leader` bigint DEFAULT NULL COMMENT '负责人',
  `phone` varchar(11) DEFAULT NULL COMMENT '联系电话',
  `email` varchar(50) DEFAULT NULL COMMENT '邮箱',
  `status` char(1) DEFAULT '0' COMMENT '部门状态（0正常 1停用）',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`dept_id`),
  KEY `idx_sys_dept_parent_id` (`parent_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='部门表';

INSERT INTO `sys_dept` (`dept_id`, `parent_id`, `ancestors`, `dept_name`, `dept_category`, `order_num`, `leader`, `phone`, `email`, `status`, `del_flag`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`) VALUES
(1761000000000000100, 0, '0', '示例公司', NULL, 0, NULL, '15888888888', 'xxx@qq.com', '0', '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:35:53', 1761100000000000001, '2026-06-16 05:08:14'),
(7472522067846500353, 1761000000000000100, '0,1761000000000000100', '示例研发部', NULL, 1, NULL, NULL, NULL, '0', '0', NULL, 1761100000000000001, '2026-06-16 05:34:54', 1761100000000000001, '2026-06-16 05:34:54'),
(7472533672114130945, 7472522067846500353, '0,1761000000000000100,7472522067846500353', '销售部', NULL, 1, NULL, NULL, NULL, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:21:00', 1761100000000000001, '2026-06-16 06:21:00'),
(7472533672114130947, 7472522067846500353, '0,1761000000000000100,7472522067846500353', '产品部', NULL, 2, NULL, NULL, NULL, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:21:16', 1761100000000000001, '2026-06-16 06:21:16'),
(7472533672114130949, 7472522067846500353, '0,1761000000000000100,7472522067846500353', '研发部', NULL, 3, NULL, NULL, NULL, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:21:26', 1761100000000000001, '2026-06-16 06:21:26');

DROP TABLE IF EXISTS `flow_node`;

CREATE TABLE `flow_node` (
  `id` bigint NOT NULL COMMENT '主键id',
  `node_type` tinyint(1) NOT NULL COMMENT '节点类型（0开始节点 1中间节点 2结束节点 3互斥网关 4并行网关）',
  `definition_id` bigint NOT NULL COMMENT '流程定义id',
  `node_code` varchar(100) NOT NULL COMMENT '流程节点编码',
  `node_name` varchar(100) DEFAULT NULL COMMENT '流程节点名称',
  `permission_flag` varchar(200) DEFAULT NULL COMMENT '权限标识（权限类型:权限标识，可以多个，用@@隔开)',
  `node_ratio` varchar(200) DEFAULT NULL COMMENT '流程签署比例值',
  `coordinate` varchar(100) DEFAULT NULL COMMENT '坐标',
  `any_node_skip` varchar(100) DEFAULT NULL COMMENT '任意结点跳转',
  `listener_type` varchar(100) DEFAULT NULL COMMENT '监听器类型',
  `listener_path` varchar(400) DEFAULT NULL COMMENT '监听器路径',
  `form_custom` char(1) DEFAULT 'N' COMMENT '审批表单是否自定义（Y是 N否）',
  `form_path` varchar(100) DEFAULT NULL COMMENT '审批表单路径',
  `version` varchar(20) NOT NULL COMMENT '版本',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` varchar(64) DEFAULT '' COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` varchar(64) DEFAULT '' COMMENT '更新人',
  `ext` text COMMENT '节点扩展属性',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程节点表';

INSERT INTO `flow_node` (`id`, `node_type`, `definition_id`, `node_code`, `node_name`, `permission_flag`, `node_ratio`, `coordinate`, `any_node_skip`, `listener_type`, `listener_path`, `form_custom`, `form_path`, `version`, `create_time`, `create_by`, `update_time`, `update_by`, `ext`, `del_flag`, `tenant_id`) VALUES
(7473380005238673410, 0, 7473380005238673409, '122b89a5-7c6f-40a3-aa09-7a263f902054', '开始', NULL, '0.000', '240,300|240,300', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7473380005238673411, 1, 7473380005238673409, 'c25a0e86-fdd1-4f03-8e22-14db70389dbd', '申请人', '${initiator}', '0.000', '400,300|400,300', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file"}]', '0', NULL),
(7473380005238673412, 1, 7473380005238673409, '2bfa3919-78cf-4bc1-b59b-df463a4546f9', '副经理', 'role:1761300000000000001@@role:1761300000000000003@@role:1761300000000000004', '0.000', '860,200|860,200', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination"}]', '0', NULL),
(7473380005238673413, 1, 7473380005238673409, 'ec17f60e-94e0-4d96-a3ce-3417e9d32d60', '组长', '1761100000000000001', '0.000', '860,400|860,400', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination"}]', '0', NULL),
(7473380005238673414, 1, 7473380005238673409, '07ecda1d-7a0a-47b5-8a91-6186c9473742', '副组长', '1761100000000000001', '0.000', '560,300|560,300', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,transfer,copy,pop"}]', '0', NULL),
(7473380005238673415, 3, 7473380005238673409, '48117e2c-6328-406b-b102-c4a9d115bb13', NULL, NULL, '0.000', '700,300', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7473380005238673416, 3, 7473380005238673409, '394e1cc8-b8b2-4189-9f81-44448e88ac32', NULL, NULL, '0.000', '1000,300', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7473380005238673417, 1, 7473380005238673409, '9c93a195-cff2-4e17-ab0a-a4f264191496', '经理会签', '1761100000000000001@@1761100000000000003', '100.000', '1180,300|1180,300', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,pop,addSign,subSign"}]', '0', NULL),
(7473380005238673418, 4, 7473380005238673409, 'a1a42056-afd1-4e90-88bc-36cbf5a66992', NULL, NULL, '0.000', '1340,300', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7473380005238673419, 1, 7473380005238673409, '350dfa0c-a77c-4efa-8527-10efa02d8be4', '总经理', '1761100000000000001@@1761100000000000003', '0.000', '1480,200|1480,200', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination"}]', '0', NULL),
(7473380005238673420, 1, 7473380005238673409, 'fcfdd9f6-f526-4c1a-b71d-88afa31aebc5', '副总经理', '1761100000000000001@@1761100000000000003', '0.000', '1480,400|1480,400', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination"}]', '0', NULL),
(7473380005238673421, 4, 7473380005238673409, 'c36a46ef-04f9-463f-bad7-4b395c818519', NULL, NULL, '0.000', '1640,300', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7473380005238673422, 1, 7473380005238673409, '3fcea762-b53a-4ae1-8365-7bec90444828', '董事', '1761100000000000001', '0.000', '1820,300|1820,300', NULL, '', '', 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination"}]', '0', NULL),
(7473380005238673423, 2, 7473380005238673409, '9cfbfd3e-6c04-41d6-9fc2-6787a7d2cd31', '结束', NULL, '0.000', '1980,300|1980,300', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994065, 0, 7474738325492994064, 'cef3895c-f7d8-4598-8bf3-8ec2ef6ce84a', '开始', NULL, '0.000', '300,240|300,240', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994066, 1, 7474738325492994064, 'fdcae93b-b69c-498a-b231-09255e74bcbd', '申请人', '${initiator}', '0.000', '440,240|440,240', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file"}]', '0', NULL),
(7474738325492994067, 3, 7474738325492994064, '7b8c7ead-7dc8-4951-a7f3-f0c41995909e', NULL, NULL, '0.000', '560,240', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994068, 1, 7474738325492994064, 'b3528155-dcb7-4445-bbdf-3d00e3499e86', '组长', '1761100000000000003@@1761100000000000004', '0.000', '720,320|720,320', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994069, 1, 7474738325492994064, 'c9fa6d7d-2a74-4e78-b947-0cad8a6af869', '总经理', 'role:1761300000000000001', '0.000', '860,240|860,240', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994070, 2, 7474738325492994064, '40aa65fd-0712-4d23-b6f7-d0432b920fd1', '结束', NULL, '0.000', '1000,240|1000,240', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994071, 1, 7474738325492994064, '5ed2362b-fc0c-4d52-831f-95208b830605', '部门领导', 'role:1761300000000000001', '0.000', '720,160|720,160', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994081, 0, 7474738325492994080, 'a80ecf9f-f465-4ae5-a429-e30ec5d0f957', '开始', NULL, '0.000', '380,220|380,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994082, 1, 7474738325492994080, 'b7bbb571-06de-455c-8083-f83c07bf0b99', '申请人', '${initiator}', '0.000', '520,220|520,220', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file"}]', '0', NULL),
(7474738325492994083, 4, 7474738325492994080, '84d7ed24-bb44-4ba1-bf1f-e6f5092d3f0a', NULL, NULL, '0.000', '680,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994084, 1, 7474738325492994080, '4b7743cd-940c-431b-926f-e7b614fbf1fe', '市场部', 'role:1761300000000000001', '0.000', '800,140|800,140', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994085, 4, 7474738325492994080, 'b66b6563-f9fe-41cc-a782-f7837bb6f3d2', NULL, NULL, '0.000', '920,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994086, 1, 7474738325492994080, '23e7429e-2b47-4431-b93e-40db7c431ce6', 'CEO', '1761100000000000001', '0.000', '1040,220|1040,220', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994087, 2, 7474738325492994080, 'f5ace37f-5a5e-4e64-a6f6-913ab9a71cd1', '结束', NULL, '0.000', '1160,220|1160,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994088, 1, 7474738325492994080, '762cb975-37d8-4276-b6db-79a4c3606394', '综合部', 'role:1761300000000000003@@role:1761300000000000004', '0.000', '800,300|800,300', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994099, 0, 7474738325492994098, '9ce8bf00-f25b-4fc6-91b8-827082fc4876', '开始', NULL, '0.000', '320,240|320,240', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994100, 1, 7474738325492994098, 'e90b98ef-35b4-410c-a663-bae8b7624b9f', '申请人', '${initiator}', '0.000', '460,240|460,240', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file"}]', '0', NULL),
(7474738325492994101, 1, 7474738325492994098, '768b5b1a-6726-4d67-8853-4cc70d5b1045', '百分之60通过', '${userList}', '60.000', '640,240|640,240', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,addSign,subSign"}]', '0', NULL),
(7474738325492994102, 1, 7474738325492994098, '2f9f2e21-9bcf-42a3-a07c-13037aad22d1', '全部审批通过', 'role:1761300000000000001@@role:1761300000000000003', '100.000', '820,240|820,240', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,addSign,subSign"}]', '0', NULL),
(7474738325492994103, 1, 7474738325492994098, '27461e01-3d9f-4530-8fe3-bd5ec7f9571f', 'CEO', '1761100000000000001', '0.000', '1000,240|1000,240', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994104, 2, 7474738325492994098, 'b62b88c3-8d8d-4969-911e-2aaea219e7fc', '结束', NULL, '0.000', '1120,240|1120,240', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994112, 0, 7474738325492994111, 'ebebaf26-9cb6-497e-8119-4c9fed4c597c', '开始', NULL, '0.000', '300,220|300,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994113, 1, 7474738325492994111, 'e1b04e96-dc81-4858-a309-2fe945d2f374', '申请人', '${initiator}', '0.000', '420,220|420,220', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file"}]', '0', NULL),
(7474738325492994114, 4, 7474738325492994111, '3e743f4f-51ca-41d4-8e94-21f5dd9b59c9', NULL, NULL, '0.000', '560,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994115, 1, 7474738325492994111, 'c80f273e-1f17-4bd8-9ad1-04a4a94ea862', '会签', 'role:1761300000000000001@@role:1761300000000000003', '100.000', '700,320|700,320', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,addSign,subSign"}]', '0', NULL),
(7474738325492994116, 4, 7474738325492994111, '1a20169e-3d82-4926-a151-e2daad28de1b', NULL, NULL, '0.000', '860,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994117, 1, 7474738325492994111, '7a8f0473-e409-442e-a843-5c2b813d00e9', 'CEO', '1761100000000000001', '0.000', '1000,220|1000,220', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,transfer,trust,copy"}]', '0', NULL),
(7474738325492994118, 2, 7474738325492994111, '03c4d2bc-58b5-4408-a2e4-65afb046f169', '结束', NULL, '0.000', '1140,220|1140,220', NULL, NULL, NULL, 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[]', '0', NULL),
(7474738325492994119, 1, 7474738325492994111, '1e3e8d3b-18ae-4d6c-a814-ce0d724adfa4', '百分之60票签', '${userList}', '60.000', '700,120|700,120', NULL, '', '', 'N', NULL, '1', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,addSign,subSign"}]', '0', NULL),
(7474749416772079617, 0, 7474738325492994053, 'd5ee3ddf-3968-4379-a86f-9ceabde5faac', '开始', NULL, '0.000', '200,200|200,200', NULL, NULL, NULL, 'N', NULL, '1', NULL, NULL, NULL, NULL, '[]', '0', NULL),
(7474749416772079618, 1, 7474738325492994053, 'dd515cdd-59f6-446f-94ca-25ca062afb42', '申请人', '${initiator}', '0.000', '360,200|360,200', NULL, '', '', 'N', NULL, '1', NULL, NULL, NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,file,copy"}]', '0', NULL),
(7474749416772079619, 1, 7474738325492994053, '78fa8e5b-e809-44ed-978a-41092409ebcf', '组长', 'user:1761100000000000001', '0.000', '540,200|540,200', NULL, '', '', 'N', NULL, '1', NULL, NULL, NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,copy,transfer,trust,file"}]', '0', NULL),
(7474749416772079620, 1, 7474738325492994053, 'a8abf15f-b83e-428a-86cc-033555ea9bbe', '部门主管', 'user:1761100000000000001', '0.000', '720,200|720,200', NULL, '', '', 'N', NULL, '1', NULL, NULL, NULL, NULL, '[{"code":"ButtonPermissionEnum","value":"back,termination,copy,transfer,trust,file"}]', '0', NULL),
(7474749416772079621, 2, 7474738325492994053, '8b82b7d7-8660-455e-b880-d6d22ea3eb6d', '结束', NULL, '0.000', '900,200|900,200', NULL, NULL, NULL, 'N', NULL, '1', NULL, NULL, NULL, NULL, '[]', '0', NULL);

DROP TABLE IF EXISTS `sys_post`;

CREATE TABLE `sys_post` (
  `post_id` bigint NOT NULL COMMENT '岗位ID',
  `dept_id` bigint NOT NULL COMMENT '部门id',
  `post_code` varchar(64) NOT NULL COMMENT '岗位编码',
  `post_category` varchar(100) DEFAULT NULL COMMENT '岗位类别编码',
  `post_name` varchar(50) NOT NULL COMMENT '岗位名称',
  `post_sort` int NOT NULL COMMENT '显示顺序',
  `status` char(1) NOT NULL COMMENT '状态（0正常 1停用）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  `del_flag` char(1) DEFAULT '0',
  PRIMARY KEY (`post_id`),
  KEY `idx_sys_post_dept_id` (`dept_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='岗位信息表';

DROP TABLE IF EXISTS `gen_table`;

CREATE TABLE `gen_table` (
  `table_id` bigint NOT NULL COMMENT '编号',
  `data_name` varchar(200) DEFAULT '' COMMENT '数据源名称',
  `table_name` varchar(200) DEFAULT '' COMMENT '表名称',
  `table_comment` varchar(500) DEFAULT '' COMMENT '表描述',
  `class_name` varchar(100) DEFAULT '' COMMENT '实体类名称',
  `tpl_category` varchar(200) DEFAULT 'crud' COMMENT '使用的模板（crud单表操作 tree树表操作）',
  `frontend_type` varchar(50) DEFAULT 'vue' COMMENT '前端模板类型，对应 vm 下的模板目录',
  `package_name` varchar(100) DEFAULT NULL COMMENT '生成包路径',
  `module_name` varchar(30) DEFAULT NULL COMMENT '生成模块名',
  `business_name` varchar(30) DEFAULT NULL COMMENT '生成业务名',
  `function_name` varchar(50) DEFAULT NULL COMMENT '生成功能名',
  `function_author` varchar(50) DEFAULT NULL COMMENT '生成功能作者',
  `gen_type` char(1) DEFAULT '0' COMMENT '生成代码方式（0zip压缩包 1自定义路径）',
  `gen_path` varchar(200) DEFAULT '/' COMMENT '生成路径（不填默认项目路径）',
  `options` varchar(1000) DEFAULT NULL COMMENT '其它生成选项',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`table_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='代码生成业务表';

DROP TABLE IF EXISTS `sys_oss_config`;

CREATE TABLE `sys_oss_config` (
  `oss_config_id` bigint NOT NULL COMMENT '主键',
  `config_key` varchar(20) NOT NULL DEFAULT '' COMMENT '配置key',
  `access_key` varchar(255) DEFAULT '' COMMENT 'accessKey',
  `secret_key` varchar(255) DEFAULT '' COMMENT '秘钥',
  `bucket_name` varchar(255) DEFAULT '' COMMENT '桶名称',
  `prefix` varchar(255) DEFAULT '' COMMENT '前缀',
  `endpoint` varchar(255) DEFAULT '' COMMENT '访问站点',
  `domain_url` varchar(255) DEFAULT '' COMMENT '自定义域名',
  `is_https` char(1) DEFAULT 'N' COMMENT '是否https（Y=是,N=否）',
  `region` varchar(255) DEFAULT '' COMMENT '域',
  `access_policy` char(1) NOT NULL DEFAULT '1' COMMENT '桶权限类型(0=private 1=public 2=custom)',
  `status` char(1) DEFAULT 'N' COMMENT '是否默认（Y=是,N=否）',
  `ext1` varchar(255) DEFAULT '' COMMENT '扩展字段',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`oss_config_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='对象存储配置表';

INSERT INTO `sys_oss_config` (`oss_config_id`, `config_key`, `access_key`, `secret_key`, `bucket_name`, `prefix`, `endpoint`, `domain_url`, `is_https`, `region`, `access_policy`, `status`, `ext1`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761900000000000001, 'minio', 'ruoyi', 'ruoyi123', 'ruoyi', '', '127.0.0.1:9000', '', 'N', '', '1', 'N', '', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:16', 1761100000000000001, '2026-06-14 15:36:16', NULL),
(1761900000000000002, 'qiniu', 'XXXXXXXXXXXXXXX', 'XXXXXXXXXXXXXXX', 'ruoyi', '', 's3-cn-north-1.qiniucs.com', '', 'N', '', '1', 'N', '', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:16', 1761100000000000001, '2026-06-14 15:36:16', NULL),
(1761900000000000003, 'aliyun', 'XXXXXXXXXXXXXXX', 'XXXXXXXXXXXXXXX', 'ruoyi', '', 'oss-cn-beijing.aliyuncs.com', '', 'N', '', '1', 'N', '', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:16', 1761100000000000001, '2026-06-14 15:36:16', NULL),
(1761900000000000004, 'qcloud', 'XXXXXXXXXXXXXXX', 'XXXXXXXXXXXXXXX', 'ruoyi-1240000000', '', 'cos.ap-beijing.myqcloud.com', '', 'N', 'ap-beijing', '1', 'N', '', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:17', 1761100000000000001, '2026-06-14 15:36:17', NULL),
(1761900000000000005, 'image', 'ruoyi', 'ruoyi123', 'ruoyi', 'image', '127.0.0.1:9000', '', 'N', '', '1', 'N', '', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:17', 1761100000000000001, '2026-06-14 15:36:17', NULL),
(1761900000000000006, 'local', '', '', 'default', '', 'file://./data/oss', '', 'N', '', '1', 'Y', NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 00:00:00', 1761100000000000001, '2026-07-31 00:00:00', '本地磁盘存储(新部署默认)');

DROP TABLE IF EXISTS `sys_message`;

CREATE TABLE `sys_message` (
  `message_id` bigint NOT NULL COMMENT '消息ID',
  `category` varchar(20) NOT NULL COMMENT '消息分组(system/notice/workflow)',
  `type` varchar(20) NOT NULL COMMENT '消息类型',
  `source` varchar(20) NOT NULL COMMENT '消息来源',
  `title` varchar(100) DEFAULT '' COMMENT '标题',
  `message` varchar(500) DEFAULT '' COMMENT '摘要消息',
  `content` longtext COMMENT '详细内容',
  `data_json` longtext COMMENT '扩展数据JSON',
  `path` varchar(500) DEFAULT NULL COMMENT '前端跳转路径',
  `send_user_ids` varchar(2000) NOT NULL DEFAULT '0' COMMENT '目标用户ID串，0表示全局',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`message_id`),
  KEY `idx_sys_message_category_time` (`category`,`create_time`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='消息记录表';


DROP TABLE IF EXISTS `flow_spel`;

CREATE TABLE `flow_spel` (
  `id` bigint NOT NULL COMMENT '主键id',
  `component_name` varchar(255) DEFAULT NULL COMMENT '组件名称',
  `method_name` varchar(255) DEFAULT NULL COMMENT '方法名',
  `method_params` varchar(255) DEFAULT NULL COMMENT '参数',
  `view_spel` varchar(255) DEFAULT NULL COMMENT '预览spel表达式',
  `remark` varchar(255) DEFAULT NULL COMMENT '备注',
  `status` char(1) DEFAULT '0' COMMENT '状态（0正常 1停用）',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程spel表达式定义表';

INSERT INTO `flow_spel` (`id`, `component_name`, `method_name`, `method_params`, `view_spel`, `remark`, `status`, `del_flag`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`) VALUES
(1762400000000000001, 'spelRuleComponent', 'selectDeptLeaderById', 'initiatorDeptId', '#{@spelRuleComponent.selectDeptLeaderById(#initiatorDeptId)}', '根据部门id获取部门负责人', '0', '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:21', 1761100000000000001, '2026-06-14 15:36:21'),
(1762400000000000002, NULL, NULL, 'initiator', '${initiator}', '流程发起人', '0', '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:39', 1761100000000000001, '2026-06-14 15:36:39');

DROP TABLE IF EXISTS `flow_variable`;

CREATE TABLE `flow_variable` (
  `id` bigint NOT NULL COMMENT '主键',
  `variable_name` varchar(64) NOT NULL COMMENT '变量名（SpEL 中的 #xxx）',
  `label` varchar(64) NOT NULL COMMENT '中文名',
  `data_type` varchar(32) NOT NULL DEFAULT 'String' COMMENT '数据类型（String/Long/Boolean）',
  `source` varchar(255) DEFAULT NULL COMMENT '来源说明（哪个 Service 注入）',
  `dict_type` varchar(100) DEFAULT NULL COMMENT '关联字典类型（值选择器数据源）',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  `status` char(1) NOT NULL DEFAULT '0' COMMENT '状态（0正常 1停用）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`id`),
  UNIQUE KEY `uk_flow_variable_name` (`variable_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程变量注册表';

DROP TABLE IF EXISTS `sys_user`;

CREATE TABLE `sys_user` (
  `user_id` bigint NOT NULL COMMENT '用户ID',
  `dept_id` bigint DEFAULT NULL COMMENT '部门ID',
  `user_name` varchar(30) NOT NULL COMMENT '用户账号',
  `nick_name` varchar(30) NOT NULL COMMENT '用户昵称',
  `user_type` varchar(10) DEFAULT 'sys_user' COMMENT '用户类型（sys_user系统用户）',
  `email` varchar(50) DEFAULT '' COMMENT '用户邮箱',
  `phone_number` varchar(11) DEFAULT '' COMMENT '手机号码',
  `gender` char(1) DEFAULT '0' COMMENT '用户性别（0男 1女 2未知）',
  `avatar` bigint DEFAULT NULL COMMENT '头像地址',
  `password` varchar(100) DEFAULT '' COMMENT '密码',
  `status` char(1) DEFAULT '0' COMMENT '账号状态（0正常 1停用）',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  `login_ip` varchar(128) DEFAULT '' COMMENT '最后登录IP',
  `login_date` datetime DEFAULT NULL COMMENT '最后登录时间',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`user_id`),
  KEY `idx_sys_user_dept_id` (`dept_id`),
  KEY `idx_sys_user_create_by` (`create_by`),
  KEY `idx_sys_user_user_name` (`user_name`),
  KEY `idx_sys_user_phone` (`phone_number`),
  KEY `idx_sys_user_email` (`email`),
  KEY `idx_sys_user_del_flag` (`del_flag`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='用户信息表';

INSERT INTO `sys_user` (`user_id`, `dept_id`, `user_name`, `nick_name`, `user_type`, `email`, `phone_number`, `gender`, `avatar`, `password`, `status`, `del_flag`, `login_ip`, `login_date`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761100000000000001, 1761000000000000103, 'admin', '管理员', 'sys_user', 'admin@example.com', '15888888888', '1', NULL, '$2a$10$7JB720yubVSZvUI0rEqK/.VqGOZTH.ulu33dHOiBE8ByOhJIrdAu2', '0', '0', '127.0.0.1', '2026-06-22 19:57:58', 1761000000000000103, 1761100000000000001, '2026-06-14 15:35:54', -1, '2026-06-22 19:57:58', '管理员');

DROP TABLE IF EXISTS `flow_skip`;

CREATE TABLE `flow_skip` (
  `id` bigint NOT NULL COMMENT '主键id',
  `definition_id` bigint NOT NULL COMMENT '流程定义id',
  `now_node_code` varchar(100) NOT NULL COMMENT '当前流程节点的编码',
  `now_node_type` tinyint(1) DEFAULT NULL COMMENT '当前节点类型（0开始节点 1中间节点 2结束节点 3互斥网关 4并行网关）',
  `next_node_code` varchar(100) NOT NULL COMMENT '下一个流程节点的编码',
  `next_node_type` tinyint(1) DEFAULT NULL COMMENT '下一个节点类型（0开始节点 1中间节点 2结束节点 3互斥网关 4并行网关）',
  `skip_name` varchar(100) DEFAULT NULL COMMENT '跳转名称',
  `skip_type` varchar(40) DEFAULT NULL COMMENT '跳转类型（PASS审批通过 REJECT退回）',
  `skip_condition` varchar(200) DEFAULT NULL COMMENT '跳转条件',
  `coordinate` varchar(100) DEFAULT NULL COMMENT '坐标',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` varchar(64) DEFAULT '' COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` varchar(64) DEFAULT '' COMMENT '更新人',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='节点跳转关联表';

INSERT INTO `flow_skip` (`id`, `definition_id`, `now_node_code`, `now_node_type`, `next_node_code`, `next_node_type`, `skip_name`, `skip_type`, `skip_condition`, `coordinate`, `create_time`, `create_by`, `update_time`, `update_by`, `del_flag`, `tenant_id`) VALUES
(7473380005238673424, 7473380005238673409, '122b89a5-7c6f-40a3-aa09-7a263f902054', 0, 'c25a0e86-fdd1-4f03-8e22-14db70389dbd', 1, NULL, 'PASS', NULL, '260,300;350,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673425, 7473380005238673409, 'c25a0e86-fdd1-4f03-8e22-14db70389dbd', 1, '07ecda1d-7a0a-47b5-8a91-6186c9473742', 1, NULL, 'PASS', NULL, '450,300;510,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673426, 7473380005238673409, '2bfa3919-78cf-4bc1-b59b-df463a4546f9', 1, '394e1cc8-b8b2-4189-9f81-44448e88ac32', 3, NULL, 'PASS', NULL, '910,200;1000,200;1000,275', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673427, 7473380005238673409, 'ec17f60e-94e0-4d96-a3ce-3417e9d32d60', 1, '394e1cc8-b8b2-4189-9f81-44448e88ac32', 3, NULL, 'PASS', NULL, '910,400;1000,400;1000,325', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673428, 7473380005238673409, '07ecda1d-7a0a-47b5-8a91-6186c9473742', 1, '48117e2c-6328-406b-b102-c4a9d115bb13', 3, NULL, 'PASS', NULL, '610,300;675,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673429, 7473380005238673409, '48117e2c-6328-406b-b102-c4a9d115bb13', 3, '2bfa3919-78cf-4bc1-b59b-df463a4546f9', 1, '大于两天', 'PASS', 'default@@${leaveDays > 2}', '700,275;700,200;810,200|700,237', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673430, 7473380005238673409, '48117e2c-6328-406b-b102-c4a9d115bb13', 3, 'ec17f60e-94e0-4d96-a3ce-3417e9d32d60', 1, NULL, 'PASS', 'spel@@#{@testLeaveServiceImpl.eval(#leaveDays)}', '700,325;700,400;810,400', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673431, 7473380005238673409, '394e1cc8-b8b2-4189-9f81-44448e88ac32', 3, '9c93a195-cff2-4e17-ab0a-a4f264191496', 1, NULL, 'PASS', NULL, '1025,300;1130,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673432, 7473380005238673409, '9c93a195-cff2-4e17-ab0a-a4f264191496', 1, 'a1a42056-afd1-4e90-88bc-36cbf5a66992', 4, NULL, 'PASS', NULL, '1230,300;1315,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673433, 7473380005238673409, 'a1a42056-afd1-4e90-88bc-36cbf5a66992', 4, 'fcfdd9f6-f526-4c1a-b71d-88afa31aebc5', 1, NULL, 'PASS', NULL, '1340,325;1340,400;1430,400', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673434, 7473380005238673409, 'a1a42056-afd1-4e90-88bc-36cbf5a66992', 4, '350dfa0c-a77c-4efa-8527-10efa02d8be4', 1, NULL, 'PASS', NULL, '1340,275;1340,200;1430,200', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673435, 7473380005238673409, '350dfa0c-a77c-4efa-8527-10efa02d8be4', 1, 'c36a46ef-04f9-463f-bad7-4b395c818519', 4, NULL, 'PASS', NULL, '1530,200;1640,200;1640,275', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673436, 7473380005238673409, 'fcfdd9f6-f526-4c1a-b71d-88afa31aebc5', 1, 'c36a46ef-04f9-463f-bad7-4b395c818519', 4, NULL, 'PASS', NULL, '1530,400;1640,400;1640,325', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673437, 7473380005238673409, 'c36a46ef-04f9-463f-bad7-4b395c818519', 4, '3fcea762-b53a-4ae1-8365-7bec90444828', 1, NULL, 'PASS', NULL, '1665,300;1770,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7473380005238673438, 7473380005238673409, '3fcea762-b53a-4ae1-8365-7bec90444828', 1, '9cfbfd3e-6c04-41d6-9fc2-6787a7d2cd31', 2, NULL, 'PASS', NULL, '1870,300;1960,300', '2026-06-18 22:24:02', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994072, 7474738325492994064, 'cef3895c-f7d8-4598-8bf3-8ec2ef6ce84a', 0, 'fdcae93b-b69c-498a-b231-09255e74bcbd', 1, NULL, 'PASS', NULL, '320,240;390,240', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994073, 7474738325492994064, 'fdcae93b-b69c-498a-b231-09255e74bcbd', 1, '7b8c7ead-7dc8-4951-a7f3-f0c41995909e', 3, NULL, 'PASS', NULL, '490,240;535,240', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994074, 7474738325492994064, '7b8c7ead-7dc8-4951-a7f3-f0c41995909e', 3, 'b3528155-dcb7-4445-bbdf-3d00e3499e86', 1, NULL, 'PASS', 'le@@leaveDays|2', '560,265;560,320;670,320', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994075, 7474738325492994064, '7b8c7ead-7dc8-4951-a7f3-f0c41995909e', 3, '5ed2362b-fc0c-4d52-831f-95208b830605', 1, '大于两天', 'PASS', 'gt@@leaveDays|2', '560,215;560,160;670,160|560,187', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994076, 7474738325492994064, 'b3528155-dcb7-4445-bbdf-3d00e3499e86', 1, 'c9fa6d7d-2a74-4e78-b947-0cad8a6af869', 1, NULL, 'PASS', NULL, '770,320;860,320;860,280', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994077, 7474738325492994064, 'c9fa6d7d-2a74-4e78-b947-0cad8a6af869', 1, '40aa65fd-0712-4d23-b6f7-d0432b920fd1', 2, NULL, 'PASS', NULL, '910,240;980,240', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994078, 7474738325492994064, '5ed2362b-fc0c-4d52-831f-95208b830605', 1, 'c9fa6d7d-2a74-4e78-b947-0cad8a6af869', 1, NULL, 'PASS', NULL, '770,160;860,160;860,200', '2026-06-22 16:22:48', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994089, 7474738325492994080, 'a80ecf9f-f465-4ae5-a429-e30ec5d0f957', 0, 'b7bbb571-06de-455c-8083-f83c07bf0b99', 1, NULL, 'PASS', NULL, '400,220;470,220', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994090, 7474738325492994080, 'b7bbb571-06de-455c-8083-f83c07bf0b99', 1, '84d7ed24-bb44-4ba1-bf1f-e6f5092d3f0a', 4, NULL, 'PASS', NULL, '570,220;655,220', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994091, 7474738325492994080, '84d7ed24-bb44-4ba1-bf1f-e6f5092d3f0a', 4, '4b7743cd-940c-431b-926f-e7b614fbf1fe', 1, NULL, 'PASS', NULL, '680,195;680,140;750,140', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994092, 7474738325492994080, '84d7ed24-bb44-4ba1-bf1f-e6f5092d3f0a', 4, '762cb975-37d8-4276-b6db-79a4c3606394', 1, NULL, 'PASS', NULL, '680,245;680,300;750,300', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994093, 7474738325492994080, '4b7743cd-940c-431b-926f-e7b614fbf1fe', 1, 'b66b6563-f9fe-41cc-a782-f7837bb6f3d2', 4, NULL, 'PASS', NULL, '850,140;920,140;920,195', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994094, 7474738325492994080, 'b66b6563-f9fe-41cc-a782-f7837bb6f3d2', 4, '23e7429e-2b47-4431-b93e-40db7c431ce6', 1, NULL, 'PASS', NULL, '945,220;975,220;975,220;960,220;960,220;990,220', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994095, 7474738325492994080, '23e7429e-2b47-4431-b93e-40db7c431ce6', 1, 'f5ace37f-5a5e-4e64-a6f6-913ab9a71cd1', 2, NULL, 'PASS', NULL, '1090,220;1140,220', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994096, 7474738325492994080, '762cb975-37d8-4276-b6db-79a4c3606394', 1, 'b66b6563-f9fe-41cc-a782-f7837bb6f3d2', 4, NULL, 'PASS', NULL, '850,300;920,300;920,245', '2026-06-22 16:22:57', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994105, 7474738325492994098, '9ce8bf00-f25b-4fc6-91b8-827082fc4876', 0, 'e90b98ef-35b4-410c-a663-bae8b7624b9f', 1, NULL, 'PASS', NULL, '340,240;410,240', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994106, 7474738325492994098, 'e90b98ef-35b4-410c-a663-bae8b7624b9f', 1, '768b5b1a-6726-4d67-8853-4cc70d5b1045', 1, NULL, 'PASS', NULL, '510,240;590,240', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994107, 7474738325492994098, '768b5b1a-6726-4d67-8853-4cc70d5b1045', 1, '2f9f2e21-9bcf-42a3-a07c-13037aad22d1', 1, NULL, 'PASS', NULL, '690,240;770,240', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994108, 7474738325492994098, '2f9f2e21-9bcf-42a3-a07c-13037aad22d1', 1, '27461e01-3d9f-4530-8fe3-bd5ec7f9571f', 1, NULL, 'PASS', NULL, '870,240;950,240', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994109, 7474738325492994098, '27461e01-3d9f-4530-8fe3-bd5ec7f9571f', 1, 'b62b88c3-8d8d-4969-911e-2aaea219e7fc', 2, NULL, 'PASS', NULL, '1050,240;1080,240;1080,240;1070,240;1070,240;1100,240', '2026-06-22 16:23:04', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994120, 7474738325492994111, 'ebebaf26-9cb6-497e-8119-4c9fed4c597c', 0, 'e1b04e96-dc81-4858-a309-2fe945d2f374', 1, NULL, 'PASS', NULL, '320,220;350,220;350,220;340,220;340,220;370,220', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994121, 7474738325492994111, 'e1b04e96-dc81-4858-a309-2fe945d2f374', 1, '3e743f4f-51ca-41d4-8e94-21f5dd9b59c9', 4, NULL, 'PASS', NULL, '470,220;535,220', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994122, 7474738325492994111, '3e743f4f-51ca-41d4-8e94-21f5dd9b59c9', 4, 'c80f273e-1f17-4bd8-9ad1-04a4a94ea862', 1, NULL, 'PASS', NULL, '560,245;560,320;650,320', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994123, 7474738325492994111, '3e743f4f-51ca-41d4-8e94-21f5dd9b59c9', 4, '1e3e8d3b-18ae-4d6c-a814-ce0d724adfa4', 1, NULL, 'PASS', NULL, '560,195;560,120;650,120', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994124, 7474738325492994111, 'c80f273e-1f17-4bd8-9ad1-04a4a94ea862', 1, '1a20169e-3d82-4926-a151-e2daad28de1b', 4, NULL, 'PASS', NULL, '750,320;860,320;860,245', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994125, 7474738325492994111, '1a20169e-3d82-4926-a151-e2daad28de1b', 4, '7a8f0473-e409-442e-a843-5c2b813d00e9', 1, NULL, 'PASS', NULL, '885,220;950,220', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994126, 7474738325492994111, '7a8f0473-e409-442e-a843-5c2b813d00e9', 1, '03c4d2bc-58b5-4408-a2e4-65afb046f169', 2, NULL, 'PASS', NULL, '1050,220;1120,220', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474738325492994127, 7474738325492994111, '1e3e8d3b-18ae-4d6c-a814-ce0d724adfa4', 1, '1a20169e-3d82-4926-a151-e2daad28de1b', 4, NULL, 'PASS', NULL, '750,120;860,120;860,195', '2026-06-22 16:23:09', '1761100000000000001', NULL, NULL, '0', NULL),
(7474749416772079622, 7474738325492994053, 'd5ee3ddf-3968-4379-a86f-9ceabde5faac', 0, 'dd515cdd-59f6-446f-94ca-25ca062afb42', 1, NULL, 'PASS', NULL, '220,200;310,200', NULL, NULL, NULL, NULL, '0', NULL),
(7474749416772079623, 7474738325492994053, 'dd515cdd-59f6-446f-94ca-25ca062afb42', 1, '78fa8e5b-e809-44ed-978a-41092409ebcf', 1, NULL, 'PASS', NULL, '410,200;490,200', NULL, NULL, NULL, NULL, '0', NULL),
(7474749416772079624, 7474738325492994053, '78fa8e5b-e809-44ed-978a-41092409ebcf', 1, 'a8abf15f-b83e-428a-86cc-033555ea9bbe', 1, NULL, 'PASS', NULL, '590,200;670,200', NULL, NULL, NULL, NULL, '0', NULL),
(7474749416772079625, 7474738325492994053, 'a8abf15f-b83e-428a-86cc-033555ea9bbe', 1, '8b82b7d7-8660-455e-b880-d6d22ea3eb6d', 2, NULL, 'PASS', NULL, '770,200;880,200', NULL, NULL, NULL, NULL, '0', NULL);

DROP TABLE IF EXISTS `sys_job`;

CREATE TABLE `sys_job` (
  `job_id` bigint NOT NULL COMMENT '任务ID（雪花ID）',
  `job_name` varchar(64) NOT NULL COMMENT '任务名称',
  `job_group` varchar(64) NOT NULL DEFAULT 'DEFAULT' COMMENT '任务组名',
  `invoke_target` varchar(500) NOT NULL COMMENT '调用目标字符串（注册的 handler 名称）',
  `cron_expression` varchar(255) NOT NULL COMMENT 'cron 表达式',
  `job_params` text COMMENT '任务参数 (JSON / 字符串)',
  `misfire_policy` tinyint NOT NULL DEFAULT '1' COMMENT '计划执行错误策略 1=默认 2=立即执行 3=放弃执行',
  `concurrent` tinyint NOT NULL DEFAULT '1' COMMENT '是否允许并发 0=禁止 1=允许',
  `status` char(1) NOT NULL DEFAULT '0' COMMENT '任务状态 0=暂停 1=运行',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  `create_by` varchar(64) DEFAULT '' COMMENT '创建者',
  `update_by` varchar(64) DEFAULT '' COMMENT '更新者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `del_flag` char(1) NOT NULL DEFAULT '0' COMMENT '删除标志 0=正常 2=删除',
  PRIMARY KEY (`job_id`),
  KEY `idx_sys_job_status_del` (`status`,`del_flag`),
  KEY `idx_sys_job_name` (`job_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='定时任务调度表';

DROP TABLE IF EXISTS `sys_job_log`;

CREATE TABLE `sys_job_log` (
  `job_log_id` bigint NOT NULL AUTO_INCREMENT COMMENT '日志ID',
  `job_name` varchar(64) NOT NULL COMMENT '任务名称',
  `job_group` varchar(64) NOT NULL COMMENT '任务组名',
  `invoke_target` varchar(500) NOT NULL COMMENT '调用目标 handler 名称',
  `job_params` text COMMENT '任务参数',
  `job_message` varchar(500) DEFAULT NULL COMMENT '日志信息',
  `status` char(1) NOT NULL DEFAULT '0' COMMENT '执行状态 0=失败 1=成功',
  `exception_info` text COMMENT '异常信息',
  `start_time` datetime DEFAULT NULL COMMENT '执行开始时间',
  `stop_time` datetime DEFAULT NULL COMMENT '执行结束时间',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  PRIMARY KEY (`job_log_id`),
  KEY `idx_sys_job_log_name_status` (`job_name`,`status`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='定时任务调度日志表';

DROP TABLE IF EXISTS `sys_notice`;

CREATE TABLE `sys_notice` (
  `notice_id` bigint NOT NULL COMMENT '公告ID',
  `notice_title` varchar(50) NOT NULL COMMENT '公告标题',
  `notice_type` char(1) NOT NULL COMMENT '公告类型（1通知 2公告）',
  `notice_content` longblob COMMENT '公告内容',
  `status` char(1) DEFAULT '0' COMMENT '公告状态（0正常 1关闭）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(255) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`notice_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='通知公告表';

INSERT INTO `sys_notice` (`notice_id`, `notice_title`, `notice_type`, `notice_content`, `status`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761800000000000001, '温馨提醒：2018-07-01 新版本发布啦', '2', 0xe696b0e78988e69cace58685e5aeb9, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:16', NULL, NULL, '管理员'),
(1761800000000000002, '维护通知：2018-07-01 系统凌晨维护', '1', 0xe7bbb4e68aa4e58685e5aeb9, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:16', NULL, NULL, '管理员');

DROP TABLE IF EXISTS `sys_client`;

CREATE TABLE `sys_client` (
  `id` bigint NOT NULL COMMENT 'id',
  `client_id` varchar(64) DEFAULT NULL COMMENT '客户端id',
  `client_key` varchar(32) DEFAULT NULL COMMENT '客户端key',
  `client_secret` varchar(255) DEFAULT NULL COMMENT '客户端秘钥',
  `grant_type` varchar(255) DEFAULT NULL COMMENT '授权类型',
  `device_type` varchar(32) DEFAULT NULL COMMENT '设备类型',
  `access_path` varchar(2000) DEFAULT NULL COMMENT '允许访问路径',
  `ip_whitelist` varchar(1000) DEFAULT NULL COMMENT 'IP白名单',
  `active_timeout` int DEFAULT '1800' COMMENT 'token活跃超时时间',
  `timeout` int DEFAULT '604800' COMMENT 'token固定超时',
  `status` char(1) DEFAULT '0' COMMENT '状态（0正常 1停用）',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='系统授权表';

INSERT INTO `sys_client` (`id`, `client_id`, `client_key`, `client_secret`, `grant_type`, `device_type`, `access_path`, `ip_whitelist`, `active_timeout`, `timeout`, `status`, `del_flag`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`) VALUES
(1762000000000000001, 'e5cd7e4891bf95d1d19206ce24a7b32e', 'pc', 'pc123', 'password,social', 'pc', NULL, NULL, 1800, 604800, '0', '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:17', 1761100000000000001, '2026-06-14 15:36:17'),
(1762000000000000002, '428a8310cd442757ae699df5d894f051', 'app', 'app123', 'password,sms,social', 'android', '/app/**', NULL, 1800, 604800, '0', '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:17', 1761100000000000001, '2026-06-14 15:36:17');

DROP TABLE IF EXISTS `flow_user`;

CREATE TABLE `flow_user` (
  `id` bigint NOT NULL COMMENT '主键id',
  `type` char(1) NOT NULL COMMENT '人员类型（1待办任务的审批人权限 2待办任务的转办人权限 3待办任务的委托人权限）',
  `processed_by` varchar(80) DEFAULT NULL COMMENT '权限人',
  `associated` bigint NOT NULL COMMENT '任务表id',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` varchar(80) DEFAULT NULL COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` varchar(64) DEFAULT '' COMMENT '创建人',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE,
  KEY `user_processed_type` (`processed_by`,`type`),
  KEY `user_associated` (`associated`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程用户表';


DROP TABLE IF EXISTS `sys_user_post`;

CREATE TABLE `sys_user_post` (
  `user_id` bigint NOT NULL COMMENT '用户ID',
  `post_id` bigint NOT NULL COMMENT '岗位ID',
  PRIMARY KEY (`user_id`,`post_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='用户与岗位关联表';

INSERT INTO `sys_user_post` (`user_id`, `post_id`) VALUES
(1761100000000000001, 1761200000000000001);

DROP TABLE IF EXISTS `flow_instance`;

CREATE TABLE `flow_instance` (
  `id` bigint NOT NULL COMMENT '主键id',
  `definition_id` bigint NOT NULL COMMENT '对应flow_definition表的id',
  `business_id` varchar(40) NOT NULL COMMENT '业务id',
  `node_type` tinyint(1) NOT NULL COMMENT '节点类型（0开始节点 1中间节点 2结束节点 3互斥网关 4并行网关）',
  `node_code` varchar(40) NOT NULL COMMENT '流程节点编码',
  `node_name` varchar(100) DEFAULT NULL COMMENT '流程节点名称',
  `variable` text COMMENT '任务变量',
  `flow_status` varchar(20) NOT NULL COMMENT '流程状态（0待提交 1审批中 2审批通过 4终止 5作废 6撤销 8已完成 9已退回 10失效 11拿回）',
  `activity_status` tinyint(1) NOT NULL DEFAULT '1' COMMENT '流程激活状态（0挂起 1激活）',
  `def_json` text COMMENT '流程定义json',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` varchar(64) DEFAULT '' COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` varchar(64) DEFAULT '' COMMENT '更新人',
  `ext` varchar(500) DEFAULT NULL COMMENT '扩展字段，预留给业务系统使用',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程实例表';

DROP TABLE IF EXISTS `flow_instance_biz_ext`;

CREATE TABLE `flow_instance_biz_ext` (
  `id` bigint NOT NULL COMMENT '主键id',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `business_code` varchar(255) DEFAULT NULL COMMENT '业务编码',
  `business_title` varchar(1000) DEFAULT NULL COMMENT '业务标题',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  `instance_id` bigint DEFAULT NULL COMMENT '流程实例Id',
  `business_id` varchar(255) DEFAULT NULL COMMENT '业务Id',
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程实例业务扩展表';


DROP TABLE IF EXISTS `flow_his_task`;

CREATE TABLE `flow_his_task` (
  `id` bigint NOT NULL COMMENT '主键id',
  `definition_id` bigint NOT NULL COMMENT '对应flow_definition表的id',
  `instance_id` bigint NOT NULL COMMENT '对应flow_instance表的id',
  `task_id` bigint NOT NULL COMMENT '对应flow_task表的id',
  `node_code` varchar(100) DEFAULT NULL COMMENT '开始节点编码',
  `node_name` varchar(100) DEFAULT NULL COMMENT '开始节点名称',
  `node_type` tinyint(1) DEFAULT NULL COMMENT '开始节点类型（0开始节点 1中间节点 2结束节点 3互斥网关 4并行网关）',
  `target_node_code` varchar(200) DEFAULT NULL COMMENT '目标节点编码',
  `target_node_name` varchar(200) DEFAULT NULL COMMENT '结束节点名称',
  `approver` varchar(40) DEFAULT NULL COMMENT '审批人',
  `cooperate_type` tinyint(1) NOT NULL DEFAULT '0' COMMENT '协作方式(1审批 2转办 3委派 4会签 5票签 6加签 7减签)',
  `collaborator` varchar(500) DEFAULT NULL COMMENT '协作人',
  `skip_type` varchar(10) NOT NULL COMMENT '流转类型（PASS通过 REJECT退回 NONE无动作）',
  `flow_status` varchar(20) NOT NULL COMMENT '流程状态（0待提交 1审批中 2审批通过 4终止 5作废 6撤销 8已完成 9已退回 10失效 11拿回）',
  `form_custom` char(1) DEFAULT 'N' COMMENT '审批表单是否自定义（Y是 N否）',
  `form_path` varchar(100) DEFAULT NULL COMMENT '审批表单路径',
  `message` varchar(500) DEFAULT NULL COMMENT '审批意见',
  `variable` text COMMENT '任务变量',
  `ext` text COMMENT '业务详情 存业务表对象json字符串',
  `create_time` datetime DEFAULT NULL COMMENT '任务开始时间',
  `update_time` datetime DEFAULT NULL COMMENT '审批完成时间',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='历史任务记录表';


DROP TABLE IF EXISTS `flow_category`;

CREATE TABLE `flow_category` (
  `category_id` bigint NOT NULL COMMENT '流程分类ID',
  `parent_id` bigint DEFAULT '0' COMMENT '父流程分类id',
  `ancestors` varchar(500) DEFAULT '' COMMENT '祖级列表',
  `category_name` varchar(30) NOT NULL COMMENT '流程分类名称',
  `order_num` int DEFAULT '0' COMMENT '显示顺序',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`category_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程分类';

INSERT INTO `flow_category` (`category_id`, `parent_id`, `ancestors`, `category_name`, `order_num`, `del_flag`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`) VALUES
(1762300000000000100, 0, '0', 'OA审批', 0, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000101, 1762300000000000100, '0,1762300000000000100', '假勤管理', 0, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000102, 1762300000000000100, '0,1762300000000000100', '人事管理', 1, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000103, 1762300000000000101, '0,1762300000000000100,1762300000000000101', '请假', 0, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000104, 1762300000000000101, '0,1762300000000000100,1762300000000000101', '出差', 1, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000105, 1762300000000000101, '0,1762300000000000100,1762300000000000101', '加班', 2, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000106, 1762300000000000101, '0,1762300000000000100,1762300000000000101', '换班', 3, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000107, 1762300000000000101, '0,1762300000000000100,1762300000000000101', '外出', 4, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000108, 1762300000000000102, '0,1762300000000000100,1762300000000000102', '转正', 1, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:20', NULL, NULL),
(1762300000000000109, 1762300000000000102, '0,1762300000000000100,1762300000000000102', '离职', 2, '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:21', NULL, NULL);

DROP TABLE IF EXISTS `sys_menu`;

CREATE TABLE `sys_menu` (
  `menu_id` bigint NOT NULL COMMENT '菜单ID',
  `menu_name` varchar(50) NOT NULL COMMENT '菜单名称',
  `parent_id` bigint DEFAULT '0' COMMENT '父菜单ID',
  `order_num` int DEFAULT '0' COMMENT '显示顺序',
  `path` varchar(200) DEFAULT '' COMMENT '路由地址',
  `component` varchar(255) DEFAULT NULL COMMENT '组件路径',
  `query_param` varchar(255) DEFAULT NULL COMMENT '路由参数',
  `is_frame` char(1) DEFAULT 'N' COMMENT '是否为外链（Y是 N否）',
  `is_cache` char(1) DEFAULT 'Y' COMMENT '是否缓存（Y缓存 N不缓存）',
  `menu_type` char(1) DEFAULT '' COMMENT '菜单类型（M目录 C菜单 F按钮）',
  `visible` char(1) DEFAULT '0' COMMENT '显示状态（0显示 1隐藏）',
  `status` char(1) DEFAULT '0' COMMENT '菜单状态（0正常 1停用）',
  `perms` varchar(100) DEFAULT NULL COMMENT '权限标识',
  `icon` varchar(100) DEFAULT '#' COMMENT '菜单图标',
  `active_menu` varchar(255) DEFAULT '' COMMENT '激活菜单路径',
  `ext` varchar(2000) DEFAULT '' COMMENT '扩展字段',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT '' COMMENT '备注',
  PRIMARY KEY (`menu_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='菜单权限表';

INSERT INTO `sys_menu` (`menu_id`, `menu_name`, `parent_id`, `order_num`, `path`, `component`, `query_param`, `is_frame`, `is_cache`, `menu_type`, `visible`, `status`, `perms`, `icon`, `active_menu`, `ext`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761400000000000001, '系统管理', 0, 100, 'system', NULL, '', '1', '0', 'M', '0', '0', '', 'system', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '系统管理目录'),
(1761400000000000002, '系统监控', 0, 110, 'monitor', NULL, '', '1', '0', 'M', '0', '0', '', 'monitor', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '系统监控目录'),
(1761400000000000003, '系统工具', 0, 120, 'tool', NULL, '', '1', '0', 'M', '0', '0', '', 'tool', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '系统工具目录'),
(1761400000000000005, '测试菜单', 0, 130, 'demo', NULL, '', '1', '0', 'M', '0', '0', '', 'star', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '测试菜单'),
(1761400000000000150, '系统信息', 1761400000000000001, 20, 'sysinfo', 'system/sysinfo/index', '', '1', '0', 'C', '0', '0', 'system:config:edit', 'form', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '系统信息菜单(标题/图标)'),
(1761400000000000100, '用户管理', 1761400000000000001, 1, 'user', 'system/user/index', '', '1', '0', 'C', '0', '0', 'system:user:list', 'user', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '用户管理菜单'),
(1761400000000000101, '角色管理', 1761400000000000001, 2, 'role', 'system/role/index', '', '1', '0', 'C', '0', '0', 'system:role:list', 'peoples', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '角色管理菜单'),
(1761400000000000102, '菜单管理', 1761400000000000001, 3, 'menu', 'system/menu/index', '', '1', '0', 'C', '0', '0', 'system:menu:list', 'tree-table', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '菜单管理菜单'),
(1761400000000000103, '部门管理', 1761400000000000001, 4, 'dept', 'system/dept/index', '', '1', '0', 'C', '0', '0', 'system:dept:list', 'tree', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '部门管理菜单'),
(1761400000000000104, '岗位管理', 1761400000000000001, 5, 'post', 'system/post/index', '', '1', '0', 'C', '0', '0', 'system:post:list', 'post', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '岗位管理菜单'),
(1761400000000000105, '字典管理', 1761400000000000001, 6, 'dict', 'system/dict/index', '', '1', '0', 'C', '0', '0', 'system:dict:list', 'dict', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '字典管理菜单'),
(1761400000000000106, '参数设置', 1761400000000000001, 7, 'config', 'system/config/index', '', '1', '0', 'C', '0', '0', 'system:config:list', 'edit', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '参数设置菜单'),
(1761400000000000107, '通知公告', 1761400000000000001, 8, 'notice', 'system/notice/index', '', '1', '0', 'C', '0', '0', 'system:notice:list', 'message', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '通知公告菜单'),
(1761400000000000108, '日志管理', 1761400000000000001, 9, 'log', '', '', '1', '0', 'M', '0', '0', '', 'log', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '日志管理菜单'),
(1761400000000000109, '在线用户', 1761400000000000002, 1, 'online', 'monitor/online/index', '', '1', '0', 'C', '0', '0', 'monitor:online:list', 'online', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '在线用户菜单'),
(1761400000000000113, '缓存监控', 1761400000000000002, 5, 'cache', 'monitor/cache/index', '', '1', '0', 'C', '0', '0', 'monitor:cache:list', 'redis', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '缓存监控菜单'),
(1761400000000000115, '代码生成', 1761400000000000003, 2, 'gen', 'tool/gen/index', '', '1', '0', 'C', '0', '0', 'tool:gen:list', 'code', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '代码生成菜单'),
(1761400000000000116, '修改生成配置', 1761400000000000003, 2, 'gen-edit/index/[tableId]', 'tool/gen/editTable', '', '1', '1', 'C', '1', '0', 'tool:gen:edit', '#', '/tool/gen', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000000118, '文件管理', 1761400000000000001, 10, 'oss', 'system/oss/index', '', '1', '0', 'C', '0', '0', 'system:oss:list', 'upload', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '文件管理菜单'),
(1761400000000000123, '客户端管理', 1761400000000000001, 11, 'client', 'system/client/index', '', '1', '0', 'C', '0', '0', 'system:client:list', 'international', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, '客户端管理菜单'),
(1761400000000000130, '分配用户', 1761400000000000001, 2, 'role-auth/user/[roleId]', 'system/role/authUser', '', '1', '1', 'C', '1', '0', 'system:role:edit', '#', '/system/role', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000000131, '分配角色', 1761400000000000001, 1, 'user-auth/role/[userId]', 'system/user/authRole', '', '1', '1', 'C', '1', '0', 'system:user:edit', '#', '/system/user', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000000133, '文件配置管理', 1761400000000000001, 10, 'oss-config', 'system/oss/config', '', '1', '1', 'C', '1', '0', 'system:ossConfig:list', '#', '/system/oss', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:30', NULL, NULL, ''),
(1761400000000000500, '操作日志', 1761400000000000108, 1, 'operlog', 'monitor/operlog/index', '', '1', '0', 'C', '0', '0', 'monitor:operlog:list', 'form', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, '操作日志菜单'),
(1761400000000000501, '登录日志', 1761400000000000108, 2, 'logininfo', 'monitor/logininfo/index', '', '1', '0', 'C', '0', '0', 'monitor:logininfo:list', 'logininfo', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, '登录日志菜单'),
(1761400000000001001, '用户查询', 1761400000000000100, 1, '', '', '', '1', '0', 'F', '0', '0', 'system:user:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001002, '用户新增', 1761400000000000100, 2, '', '', '', '1', '0', 'F', '0', '0', 'system:user:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001003, '用户修改', 1761400000000000100, 3, '', '', '', '1', '0', 'F', '0', '0', 'system:user:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001004, '用户删除', 1761400000000000100, 4, '', '', '', '1', '0', 'F', '0', '0', 'system:user:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001005, '用户导出', 1761400000000000100, 5, '', '', '', '1', '0', 'F', '0', '0', 'system:user:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001006, '用户导入', 1761400000000000100, 6, '', '', '', '1', '0', 'F', '0', '0', 'system:user:import', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001007, '重置密码', 1761400000000000100, 7, '', '', '', '1', '0', 'F', '0', '0', 'system:user:resetPwd', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001008, '角色查询', 1761400000000000101, 1, '', '', '', '1', '0', 'F', '0', '0', 'system:role:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001009, '角色新增', 1761400000000000101, 2, '', '', '', '1', '0', 'F', '0', '0', 'system:role:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001010, '角色修改', 1761400000000000101, 3, '', '', '', '1', '0', 'F', '0', '0', 'system:role:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001011, '角色删除', 1761400000000000101, 4, '', '', '', '1', '0', 'F', '0', '0', 'system:role:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001012, '角色导出', 1761400000000000101, 5, '', '', '', '1', '0', 'F', '0', '0', 'system:role:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001013, '菜单查询', 1761400000000000102, 1, '', '', '', '1', '0', 'F', '0', '0', 'system:menu:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001014, '菜单新增', 1761400000000000102, 2, '', '', '', '1', '0', 'F', '0', '0', 'system:menu:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001015, '菜单修改', 1761400000000000102, 3, '', '', '', '1', '0', 'F', '0', '0', 'system:menu:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001016, '菜单删除', 1761400000000000102, 4, '', '', '', '1', '0', 'F', '0', '0', 'system:menu:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001017, '部门查询', 1761400000000000103, 1, '', '', '', '1', '0', 'F', '0', '0', 'system:dept:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001018, '部门新增', 1761400000000000103, 2, '', '', '', '1', '0', 'F', '0', '0', 'system:dept:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001019, '部门修改', 1761400000000000103, 3, '', '', '', '1', '0', 'F', '0', '0', 'system:dept:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001020, '部门删除', 1761400000000000103, 4, '', '', '', '1', '0', 'F', '0', '0', 'system:dept:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001021, '岗位查询', 1761400000000000104, 1, '', '', '', '1', '0', 'F', '0', '0', 'system:post:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001022, '岗位新增', 1761400000000000104, 2, '', '', '', '1', '0', 'F', '0', '0', 'system:post:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001023, '岗位修改', 1761400000000000104, 3, '', '', '', '1', '0', 'F', '0', '0', 'system:post:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001024, '岗位删除', 1761400000000000104, 4, '', '', '', '1', '0', 'F', '0', '0', 'system:post:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001025, '岗位导出', 1761400000000000104, 5, '', '', '', '1', '0', 'F', '0', '0', 'system:post:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001026, '字典查询', 1761400000000000105, 1, '#', '', '', '1', '0', 'F', '0', '0', 'system:dict:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001027, '字典新增', 1761400000000000105, 2, '#', '', '', '1', '0', 'F', '0', '0', 'system:dict:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:31', NULL, NULL, ''),
(1761400000000001028, '字典修改', 1761400000000000105, 3, '#', '', '', '1', '0', 'F', '0', '0', 'system:dict:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001029, '字典删除', 1761400000000000105, 4, '#', '', '', '1', '0', 'F', '0', '0', 'system:dict:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001030, '字典导出', 1761400000000000105, 5, '#', '', '', '1', '0', 'F', '0', '0', 'system:dict:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001031, '参数查询', 1761400000000000106, 1, '#', '', '', '1', '0', 'F', '0', '0', 'system:config:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001032, '参数新增', 1761400000000000106, 2, '#', '', '', '1', '0', 'F', '0', '0', 'system:config:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001033, '参数修改', 1761400000000000106, 3, '#', '', '', '1', '0', 'F', '0', '0', 'system:config:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001034, '参数删除', 1761400000000000106, 4, '#', '', '', '1', '0', 'F', '0', '0', 'system:config:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001035, '参数导出', 1761400000000000106, 5, '#', '', '', '1', '0', 'F', '0', '0', 'system:config:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001036, '公告查询', 1761400000000000107, 1, '#', '', '', '1', '0', 'F', '0', '0', 'system:notice:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001037, '公告新增', 1761400000000000107, 2, '#', '', '', '1', '0', 'F', '0', '0', 'system:notice:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001038, '公告修改', 1761400000000000107, 3, '#', '', '', '1', '0', 'F', '0', '0', 'system:notice:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001039, '公告删除', 1761400000000000107, 4, '#', '', '', '1', '0', 'F', '0', '0', 'system:notice:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001040, '操作查询', 1761400000000000500, 1, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:operlog:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001041, '操作删除', 1761400000000000500, 2, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:operlog:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001042, '日志导出', 1761400000000000500, 4, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:operlog:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001044, '登录删除', 1761400000000000501, 2, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:logininfo:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001045, '日志导出', 1761400000000000501, 3, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:logininfo:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001048, '单条强退', 1761400000000000109, 3, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:online:forceLogout', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001050, '账户解锁', 1761400000000000501, 4, '#', '', '', '1', '0', 'F', '0', '0', 'monitor:logininfo:unlock', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001055, '生成查询', 1761400000000000115, 1, '#', '', '', '1', '0', 'F', '0', '0', 'tool:gen:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001056, '生成修改', 1761400000000000115, 2, '#', '', '', '1', '0', 'F', '0', '0', 'tool:gen:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001057, '生成删除', 1761400000000000115, 3, '#', '', '', '1', '0', 'F', '0', '0', 'tool:gen:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001058, '导入代码', 1761400000000000115, 2, '#', '', '', '1', '0', 'F', '0', '0', 'tool:gen:import', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001059, '预览代码', 1761400000000000115, 4, '#', '', '', '1', '0', 'F', '0', '0', 'tool:gen:preview', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001060, '生成代码', 1761400000000000115, 5, '#', '', '', '1', '0', 'F', '0', '0', 'tool:gen:code', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000001061, '客户端管理查询', 1761400000000000123, 1, '#', '', '', '1', '0', 'F', '0', '0', 'system:client:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001062, '客户端管理新增', 1761400000000000123, 2, '#', '', '', '1', '0', 'F', '0', '0', 'system:client:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001063, '客户端管理修改', 1761400000000000123, 3, '#', '', '', '1', '0', 'F', '0', '0', 'system:client:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001064, '客户端管理删除', 1761400000000000123, 4, '#', '', '', '1', '0', 'F', '0', '0', 'system:client:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001065, '客户端管理导出', 1761400000000000123, 5, '#', '', '', '1', '0', 'F', '0', '0', 'system:client:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001600, '文件查询', 1761400000000000118, 1, '#', '', '', '1', '0', 'F', '0', '0', 'system:oss:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001601, '文件上传', 1761400000000000118, 2, '#', '', '', '1', '0', 'F', '0', '0', 'system:oss:upload', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001602, '文件下载', 1761400000000000118, 3, '#', '', '', '1', '0', 'F', '0', '0', 'system:oss:download', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001603, '文件删除', 1761400000000000118, 4, '#', '', '', '1', '0', 'F', '0', '0', 'system:oss:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001620, '配置列表', 1761400000000000118, 5, '#', '', '', '1', '0', 'F', '0', '0', 'system:ossConfig:list', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001621, '配置添加', 1761400000000000118, 6, '#', '', '', '1', '0', 'F', '0', '0', 'system:ossConfig:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001622, '配置编辑', 1761400000000000118, 6, '#', '', '', '1', '0', 'F', '0', '0', 'system:ossConfig:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000001623, '配置删除', 1761400000000000118, 6, '#', '', '', '1', '0', 'F', '0', '0', 'system:ossConfig:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:32', NULL, NULL, ''),
(1761400000000011616, '工作流', 0, 140, 'workflow', '', '', '1', '0', 'M', '0', '0', '', 'workflow', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011618, '我的任务', 0, 150, 'task', '', '', '1', '0', 'M', '0', '0', '', 'my-task', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:35', NULL, NULL, ''),
(1761400000000011619, '我的待办', 1761400000000011618, 2, 'taskWaiting', 'workflow/task/taskWaiting', '', '1', '1', 'C', '0', '0', '', 'waiting', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:35', NULL, NULL, ''),
(1761400000000011620, '流程定义', 1761400000000011616, 3, 'processDefinition', 'workflow/processDefinition/index', '', '1', '1', 'C', '0', '0', 'workflow:definition:list', 'process-definition', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011621, '流程实例', 1761400000000011630, 1, 'processInstance', 'workflow/processInstance/index', '', '1', '1', 'C', '0', '0', 'workflow:instance:list', 'tree-table', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011622, '流程分类', 1761400000000011616, 1, 'category', 'workflow/category/index', '', '1', '0', 'C', '0', '0', 'workflow:category:list', 'category', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011623, '流程分类查询', 1761400000000011622, 1, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:category:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011624, '流程分类新增', 1761400000000011622, 2, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:category:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011625, '流程分类修改', 1761400000000011622, 3, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:category:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011626, '流程分类删除', 1761400000000011622, 4, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:category:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011627, '流程分类导出', 1761400000000011622, 5, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:category:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011629, '我发起的', 1761400000000011618, 1, 'myDocument', 'workflow/task/myDocument', '', '1', '1', 'C', '0', '0', 'workflow:instance:currentList', 'guide', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:35', NULL, NULL, ''),
(1761400000000011630, '流程监控', 1761400000000011616, 4, 'processMonitor', '', '', '1', '0', 'M', '0', '0', '', 'monitor', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011631, '待办任务', 1761400000000011630, 2, 'allTaskWaiting', 'workflow/task/allTaskWaiting', '', '1', '1', 'C', '0', '0', 'workflow:task:list', 'waiting', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011632, '我的已办', 1761400000000011618, 3, 'taskFinish', 'workflow/task/taskFinish', '', '1', '1', 'C', '0', '0', '', 'finish', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:35', NULL, NULL, ''),
(1761400000000011633, '我的抄送', 1761400000000011618, 4, 'taskCopyList', 'workflow/task/taskCopyList', '', '1', '1', 'C', '0', '0', '', 'my-copy', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:35', NULL, NULL, ''),
(1761400000000011638, '请假申请', 1761400000000000005, 1, 'leave', 'workflow/leave/index', '', '1', '0', 'C', '0', '0', 'workflow:leave:list', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '请假申请菜单'),
(1761400000000011639, '请假申请查询', 1761400000000011638, 1, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:leave:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011640, '请假申请新增', 1761400000000011638, 2, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:leave:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011641, '请假申请修改', 1761400000000011638, 3, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:leave:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011642, '请假申请删除', 1761400000000011638, 4, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:leave:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011643, '请假申请导出', 1761400000000011638, 5, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:leave:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1761400000000011644, '流程定义查询', 1761400000000011620, 1, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011645, '流程定义新增', 1761400000000011620, 2, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011646, '流程定义修改', 1761400000000011620, 3, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011647, '流程定义删除', 1761400000000011620, 4, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011648, '流程定义导出', 1761400000000011620, 5, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:export', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011649, '流程定义导入', 1761400000000011620, 6, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:import', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011650, '流程定义发布/取消发布', 1761400000000011620, 7, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:publish', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011651, '流程定义复制', 1761400000000011620, 8, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:copy', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011652, '流程定义激活/挂起', 1761400000000011620, 9, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:definition:active', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011653, '流程实例查询', 1761400000000011621, 1, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011654, '流程变量查询', 1761400000000011621, 2, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:variableQuery', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011655, '流程变量修改', 1761400000000011621, 3, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:variable', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011656, '流程实例激活/挂起', 1761400000000011621, 4, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:active', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011657, '流程实例删除', 1761400000000011621, 5, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011658, '流程实例作废', 1761400000000011621, 6, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:invalid', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011659, '流程实例撤销', 1761400000000011621, 7, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:instance:cancel', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011660, '待办任务修改', 1761400000000011631, 1, '#', '', '', '1', '0', 'F', '0', '0', 'workflow:task:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011700, '流程设计', 1761400000000011616, 5, 'design/index', 'workflow/processDefinition/design', '', '1', '1', 'C', '1', '0', 'workflow:leave:edit', '#', '/workflow/processDefinition', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011701, '请假申请', 1761400000000011616, 6, 'leaveEdit/[[id]]', 'workflow/leave/leaveEdit', '', '1', '1', 'C', '1', '0', 'workflow:leave:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011801, '流程表达式', 1761400000000011616, 2, 'spel', 'workflow/spel/index', '', '1', '0', 'C', '0', '0', 'workflow:spel:list', 'input', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', 1761100000000000001, '2026-07-31 08:19:33', '流程达式定义菜单'),
(1761400000000011802, '流程达式定义查询', 1761400000000011801, 1, '#', '', NULL, '1', '0', 'F', '0', '0', 'workflow:spel:query', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011803, '流程达式定义新增', 1761400000000011801, 2, '#', '', NULL, '1', '0', 'F', '0', '0', 'workflow:spel:add', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011804, '流程达式定义修改', 1761400000000011801, 3, '#', '', NULL, '1', '0', 'F', '0', '0', 'workflow:spel:edit', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1761400000000011805, '流程达式定义删除', 1761400000000011801, 4, '#', '', NULL, '1', '0', 'F', '0', '0', 'workflow:spel:remove', '#', '', '', 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:34', NULL, NULL, ''),
(1990000000000000001, '定时任务', 1761400000000000002, 1, 'job', 'monitor/job/index', NULL, '1', '0', 'C', '0', '0', 'monitor:job:list', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '定时任务菜单'),
(1990000000000000002, '调度日志', 1761400000000000002, 2, 'joblog', 'monitor/joblog/index', NULL, '1', '0', 'C', '0', '0', 'monitor:job:list', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '调度日志菜单'),
(1990000000000000003, '任务查询', 1990000000000000001, 1, '#', '', NULL, '1', '0', 'F', '0', '0', 'monitor:job:query', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1990000000000000004, '任务新增', 1990000000000000001, 2, '#', '', NULL, '1', '0', 'F', '0', '0', 'monitor:job:add', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1990000000000000005, '任务修改', 1990000000000000001, 3, '#', '', NULL, '1', '0', 'F', '0', '0', 'monitor:job:edit', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1990000000000000006, '任务删除', 1990000000000000001, 4, '#', '', NULL, '1', '0', 'F', '0', '0', 'monitor:job:remove', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, ''),
(1990000000000000007, '状态修改', 1990000000000000001, 5, '#', '', NULL, '1', '0', 'F', '0', '0', 'monitor:job:changeStatus', '#', NULL, NULL, 1761000000000000103, 1761100000000000001, '2026-07-31 08:19:33', NULL, NULL, '');

DROP TABLE IF EXISTS `sys_config`;

CREATE TABLE `sys_config` (
  `config_id` bigint NOT NULL COMMENT '参数主键',
  `config_name` varchar(100) DEFAULT '' COMMENT '参数名称',
  `config_key` varchar(100) DEFAULT '' COMMENT '参数键名',
  `config_value` varchar(500) DEFAULT '' COMMENT '参数键值',
  `config_type` char(1) DEFAULT 'N' COMMENT '系统内置（Y是 N否）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`config_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='参数配置表';

INSERT INTO `sys_config` (`config_id`, `config_name`, `config_key`, `config_value`, `config_type`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761700000000000001, '用户管理-账号初始密码', 'sys.user.initPassword', '123456', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, '初始化密码 123456'),
(1761700000000000002, '账号自助-是否开启用户注册功能', 'sys.account.registerUser', 'false', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, '是否开启注册用户功能（true开启，false关闭）'),
(1761700000000000003, 'OSS预览列表资源开关', 'sys.oss.previewListResource', 'true', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, 'true:开启, false:关闭'),
(1761700000000000004, '系统信息-系统标题', 'sys.title', 'SolidAxum', 'Y', 1761000000000000103, 1761100000000000001, '2026-10-13 10:00:00', NULL, NULL, '顶栏标题(医院全称+系统名),系统信息页可改'),
(1761700000000000005, '系统信息-系统图标', 'sys.icon', '', 'Y', 1761000000000000103, 1761100000000000001, '2026-10-13 10:00:00', NULL, NULL, '侧栏顶部图标 URL(OSS 上传),系统信息页可改');

DROP TABLE IF EXISTS `gen_table_column`;

CREATE TABLE `gen_table_column` (
  `column_id` bigint NOT NULL COMMENT '编号',
  `table_id` bigint DEFAULT NULL COMMENT '归属表编号',
  `column_name` varchar(200) DEFAULT NULL COMMENT '列名称',
  `column_comment` varchar(500) DEFAULT NULL COMMENT '列描述',
  `column_type` varchar(100) DEFAULT NULL COMMENT '列类型',
  `java_type` varchar(500) DEFAULT NULL COMMENT 'JAVA类型',
  `java_field` varchar(200) DEFAULT NULL COMMENT 'JAVA字段名',
  `is_pk` char(1) DEFAULT NULL COMMENT '是否主键（1是）',
  `is_increment` char(1) DEFAULT NULL COMMENT '是否自增（1是）',
  `is_required` char(1) DEFAULT NULL COMMENT '是否必填（1是）',
  `is_insert` char(1) DEFAULT NULL COMMENT '是否为插入字段（1是）',
  `is_edit` char(1) DEFAULT NULL COMMENT '是否编辑字段（1是）',
  `is_list` char(1) DEFAULT NULL COMMENT '是否列表字段（1是）',
  `is_query` char(1) DEFAULT NULL COMMENT '是否查询字段（1是）',
  `query_type` varchar(200) DEFAULT 'EQ' COMMENT '查询方式（等于、不等于、大于、小于、范围）',
  `html_type` varchar(200) DEFAULT NULL COMMENT '显示类型（文本框、文本域、下拉框、复选框、单选框、日期控件）',
  `dict_type` varchar(200) DEFAULT '' COMMENT '字典类型',
  `sort` int DEFAULT NULL COMMENT '排序',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`column_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='代码生成业务表字段';

DROP TABLE IF EXISTS `test_leave`;

CREATE TABLE `test_leave` (
  `id` bigint NOT NULL COMMENT 'id',
  `apply_code` varchar(50) NOT NULL COMMENT '申请编号',
  `leave_type` varchar(255) NOT NULL COMMENT '请假类型',
  `start_date` datetime NOT NULL COMMENT '开始时间',
  `end_date` datetime NOT NULL COMMENT '结束时间',
  `leave_days` int NOT NULL COMMENT '请假天数',
  `remark` varchar(255) DEFAULT NULL COMMENT '请假原因',
  `status` varchar(255) DEFAULT NULL COMMENT '状态',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='请假申请表';

INSERT INTO `test_leave` (`id`, `apply_code`, `leave_type`, `start_date`, `end_date`, `leave_days`, `remark`, `status`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`) VALUES
(7474738325492994049, '1782116490720', '1', '2026-06-22 16:21:31', '2026-06-29 16:21:31', 7, '测试测试', 'waiting', NULL, 1761100000000000001, '2026-06-22 16:21:31', 1761100000000000001, '2026-06-23 15:38:49'),
(7475025458015375361, '1782184948448', '1', '2026-06-23 11:22:28', '2026-06-26 11:22:28', 3, '123', 'waiting', NULL, 1761100000000000001, '2026-06-23 11:22:28', 1761100000000000001, '2026-06-23 15:57:19');

DROP TABLE IF EXISTS `sys_dict_type`;

CREATE TABLE `sys_dict_type` (
  `dict_id` bigint NOT NULL COMMENT '字典主键',
  `dict_name` varchar(100) DEFAULT '' COMMENT '字典名称',
  `dict_type` varchar(100) DEFAULT '' COMMENT '字典类型',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`dict_id`),
  UNIQUE KEY `dict_type` (`dict_type`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='字典类型表';

INSERT INTO `sys_dict_type` (`dict_id`, `dict_name`, `dict_type`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761500000000000001, '用户性别', 'sys_user_gender', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '用户性别列表'),
(1761500000000000002, '菜单状态', 'sys_show_hide', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '菜单状态列表'),
(1761500000000000003, '系统开关', 'sys_normal_disable', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '系统开关列表'),
(1761500000000000006, '系统是否', 'sys_yes_no', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '系统是否列表'),
(1761500000000000007, '通知类型', 'sys_notice_type', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '通知类型列表'),
(1761500000000000008, '通知状态', 'sys_notice_status', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '通知状态列表'),
(1761500000000000009, '操作类型', 'sys_oper_type', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '操作类型列表'),
(1761500000000000010, '系统状态', 'sys_common_status', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '登录状态列表'),
(1761500000000000011, '授权类型', 'sys_grant_type', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '认证授权类型'),
(1761500000000000012, '设备类型', 'sys_device_type', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:12', NULL, NULL, '客户端设备类型'),
(1761500000000000013, '业务状态', 'wf_business_status', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:42', NULL, NULL, '业务状态列表'),
(1761500000000000014, '表单类型', 'wf_form_type', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:42', NULL, NULL, '表单类型列表'),
(1761500000000000015, '任务状态', 'wf_task_status', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '任务状态');

DROP TABLE IF EXISTS `sys_role_dept`;

CREATE TABLE `sys_role_dept` (
  `role_id` bigint NOT NULL COMMENT '角色ID',
  `dept_id` bigint NOT NULL COMMENT '部门ID',
  PRIMARY KEY (`role_id`,`dept_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='角色和部门关联表';

DROP TABLE IF EXISTS `sys_dict_data`;

CREATE TABLE `sys_dict_data` (
  `dict_code` bigint NOT NULL COMMENT '字典编码',
  `dict_sort` int DEFAULT '0' COMMENT '字典排序',
  `dict_label` varchar(100) DEFAULT '' COMMENT '字典标签',
  `dict_value` varchar(100) DEFAULT '' COMMENT '字典键值',
  `dict_type` varchar(100) DEFAULT '' COMMENT '字典类型',
  `css_class` varchar(100) DEFAULT NULL COMMENT '样式属性（其他样式扩展）',
  `list_class` varchar(100) DEFAULT NULL COMMENT '表格回显样式',
  `is_default` char(1) DEFAULT 'N' COMMENT '是否默认（Y是 N否）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`dict_code`),
  KEY `idx_sys_dict_data_type` (`dict_type`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='字典数据表';

INSERT INTO `sys_dict_data` (`dict_code`, `dict_sort`, `dict_label`, `dict_value`, `dict_type`, `css_class`, `list_class`, `is_default`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761600000000000001, 1, '男', '0', 'sys_user_gender', '', '', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '性别男'),
(1761600000000000002, 2, '女', '1', 'sys_user_gender', '', '', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '性别女'),
(1761600000000000003, 3, '未知', '2', 'sys_user_gender', '', '', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '性别未知'),
(1761600000000000004, 1, '显示', '0', 'sys_show_hide', '', 'primary', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '显示菜单'),
(1761600000000000005, 2, '隐藏', '1', 'sys_show_hide', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '隐藏菜单'),
(1761600000000000006, 1, '正常', '0', 'sys_normal_disable', '', 'primary', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '正常状态'),
(1761600000000000007, 2, '停用', '1', 'sys_normal_disable', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '停用状态'),
(1761600000000000012, 1, '是', 'Y', 'sys_yes_no', '', 'primary', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '系统默认是'),
(1761600000000000013, 2, '否', 'N', 'sys_yes_no', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '系统默认否'),
(1761600000000000014, 1, '通知', '1', 'sys_notice_type', '', 'warning', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '通知'),
(1761600000000000015, 2, '公告', '2', 'sys_notice_type', '', 'success', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '公告'),
(1761600000000000016, 1, '正常', '0', 'sys_notice_status', '', 'primary', 'Y', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '正常状态'),
(1761600000000000017, 2, '关闭', '1', 'sys_notice_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '关闭状态'),
(1761600000000000018, 1, '新增', '1', 'sys_oper_type', '', 'info', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '新增操作'),
(1761600000000000019, 2, '修改', '2', 'sys_oper_type', '', 'info', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '修改操作'),
(1761600000000000020, 3, '删除', '3', 'sys_oper_type', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '删除操作'),
(1761600000000000021, 4, '授权', '4', 'sys_oper_type', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '授权操作'),
(1761600000000000022, 5, '导出', '5', 'sys_oper_type', '', 'warning', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '导出操作'),
(1761600000000000023, 6, '导入', '6', 'sys_oper_type', '', 'warning', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '导入操作'),
(1761600000000000024, 7, '强退', '7', 'sys_oper_type', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '强退操作'),
(1761600000000000025, 8, '生成代码', '8', 'sys_oper_type', '', 'warning', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '生成操作'),
(1761600000000000026, 9, '清空数据', '9', 'sys_oper_type', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '清空操作'),
(1761600000000000027, 1, '成功', '0', 'sys_common_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '正常状态'),
(1761600000000000028, 2, '失败', '1', 'sys_common_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '停用状态'),
(1761600000000000029, 99, '其他', '0', 'sys_oper_type', '', 'info', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:13', NULL, NULL, '其他操作'),
(1761600000000000030, 0, '密码认证', 'password', 'sys_grant_type', 'el-check-tag', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '密码认证'),
(1761600000000000031, 0, '短信认证', 'sms', 'sys_grant_type', 'el-check-tag', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '短信认证'),
(1761600000000000032, 0, '邮件认证', 'email', 'sys_grant_type', 'el-check-tag', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:14', NULL, NULL, '邮件认证'),
(1761600000000000033, 0, '小程序认证', 'xcx', 'sys_grant_type', 'el-check-tag', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, '小程序认证'),
(1761600000000000034, 0, '三方登录认证', 'social', 'sys_grant_type', 'el-check-tag', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, '三方登录认证'),
(1761600000000000035, 0, 'PC', 'pc', 'sys_device_type', '', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, 'PC'),
(1761600000000000036, 0, '安卓', 'android', 'sys_device_type', '', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, '安卓'),
(1761600000000000037, 0, 'iOS', 'ios', 'sys_device_type', '', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, 'iOS'),
(1761600000000000038, 0, '小程序', 'xcx', 'sys_device_type', '', 'default', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:15', NULL, NULL, '小程序'),
(1761600000000000039, 1, '已撤销', 'cancel', 'wf_business_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '已撤销'),
(1761600000000000040, 2, '草稿', 'draft', 'wf_business_status', '', 'info', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '草稿'),
(1761600000000000041, 3, '待审核', 'waiting', 'wf_business_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '待审核'),
(1761600000000000042, 4, '已完成', 'finish', 'wf_business_status', '', 'success', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '已完成'),
(1761600000000000043, 5, '已作废', 'invalid', 'wf_business_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '已作废'),
(1761600000000000044, 6, '已退回', 'back', 'wf_business_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '已退回'),
(1761600000000000045, 7, '已终止', 'termination', 'wf_business_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '已终止'),
(1761600000000000046, 1, '自定义表单', 'static', 'wf_form_type', '', 'success', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '自定义表单'),
(1761600000000000047, 2, '动态表单', 'dynamic', 'wf_form_type', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '动态表单'),
(1761600000000000048, 1, '撤销', 'cancel', 'wf_task_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '撤销'),
(1761600000000000049, 2, '通过', 'pass', 'wf_task_status', '', 'success', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '通过'),
(1761600000000000050, 3, '待审核', 'waiting', 'wf_task_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '待审核'),
(1761600000000000051, 4, '作废', 'invalid', 'wf_task_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:43', NULL, NULL, '作废'),
(1761600000000000052, 5, '退回', 'back', 'wf_task_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '退回'),
(1761600000000000053, 6, '终止', 'termination', 'wf_task_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '终止'),
(1761600000000000054, 7, '转办', 'transfer', 'wf_task_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '转办'),
(1761600000000000055, 8, '委托', 'depute', 'wf_task_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '委托'),
(1761600000000000056, 9, '抄送', 'copy', 'wf_task_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '抄送'),
(1761600000000000057, 10, '加签', 'sign', 'wf_task_status', '', 'primary', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '加签'),
(1761600000000000058, 11, '减签', 'sign_off', 'wf_task_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '减签'),
(1761600000000000059, 11, '超时', 'timeout', 'wf_task_status', '', 'danger', 'N', 1761000000000000103, 1761100000000000001, '2026-06-14 15:36:44', NULL, NULL, '超时');

DROP TABLE IF EXISTS `sys_oper_log`;

CREATE TABLE `sys_oper_log` (
  `oper_id` bigint NOT NULL COMMENT '日志主键',
  `title` varchar(50) DEFAULT '' COMMENT '模块标题',
  `business_type` int DEFAULT '0' COMMENT '业务类型（0其它 1新增 2修改 3删除）',
  `method` varchar(100) DEFAULT '' COMMENT '方法名称',
  `request_method` varchar(10) DEFAULT '' COMMENT '请求方式',
  `operator_type` int DEFAULT '0' COMMENT '操作类别（0其它 1后台用户 2手机端用户）',
  `oper_name` varchar(50) DEFAULT '' COMMENT '操作人员',
  `user_id` bigint DEFAULT NULL COMMENT '操作用户ID',
  `dept_id` bigint DEFAULT NULL COMMENT '操作部门ID',
  `dept_name` varchar(50) DEFAULT '' COMMENT '部门名称',
  `client_key` varchar(32) DEFAULT '' COMMENT '客户端',
  `device_type` varchar(32) DEFAULT '' COMMENT '设备类型',
  `browser` varchar(50) DEFAULT '' COMMENT '浏览器类型',
  `os` varchar(50) DEFAULT '' COMMENT '操作系统',
  `oper_url` varchar(255) DEFAULT '' COMMENT '请求URL',
  `oper_ip` varchar(128) DEFAULT '' COMMENT '主机地址',
  `oper_location` varchar(255) DEFAULT '' COMMENT '操作地点',
  `oper_param` varchar(4000) DEFAULT '' COMMENT '请求参数',
  `json_result` varchar(4000) DEFAULT '' COMMENT '返回参数',
  `status` int DEFAULT '0' COMMENT '操作状态（0正常 1异常）',
  `error_msg` varchar(4000) DEFAULT '' COMMENT '错误消息',
  `oper_time` datetime DEFAULT NULL COMMENT '操作时间',
  `cost_time` bigint DEFAULT '0' COMMENT '消耗时间',
  PRIMARY KEY (`oper_id`),
  KEY `idx_sys_oper_log_bt` (`business_type`),
  KEY `idx_sys_oper_log_uid` (`user_id`),
  KEY `idx_sys_oper_log_s` (`status`),
  KEY `idx_sys_oper_log_ot` (`oper_time`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='操作日志记录';

DROP TABLE IF EXISTS `flow_task`;

CREATE TABLE `flow_task` (
  `id` bigint NOT NULL COMMENT '主键id',
  `definition_id` bigint NOT NULL COMMENT '对应flow_definition表的id',
  `instance_id` bigint NOT NULL COMMENT '对应flow_instance表的id',
  `node_code` varchar(100) NOT NULL COMMENT '节点编码',
  `node_name` varchar(100) DEFAULT NULL COMMENT '节点名称',
  `node_type` tinyint(1) NOT NULL COMMENT '节点类型（0开始节点 1中间节点 2结束节点 3互斥网关 4并行网关）',
  `flow_status` varchar(20) NOT NULL COMMENT '流程状态（0待提交 1审批中 2审批通过 4终止 5作废 6撤销 8已完成 9已退回 10失效 11拿回）',
  `form_custom` char(1) DEFAULT 'N' COMMENT '审批表单是否自定义（Y是 N否）',
  `form_path` varchar(100) DEFAULT NULL COMMENT '审批表单路径',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` varchar(64) DEFAULT '' COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` varchar(64) DEFAULT '' COMMENT '更新人',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='待办任务表';


DROP TABLE IF EXISTS `sys_role_menu`;

CREATE TABLE `sys_role_menu` (
  `role_id` bigint NOT NULL COMMENT '角色ID',
  `menu_id` bigint NOT NULL COMMENT '菜单ID',
  PRIMARY KEY (`role_id`,`menu_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='角色和菜单关联表';

INSERT INTO `sys_role_menu` (`role_id`, `menu_id`) VALUES
(1761300000000000003, 1761400000000000001),
(1761300000000000003, 1761400000000000005),
(1761300000000000003, 1761400000000000100),
(1761300000000000003, 1761400000000000101),
(1761300000000000003, 1761400000000000102),
(1761300000000000003, 1761400000000000103),
(1761300000000000003, 1761400000000000104),
(1761300000000000003, 1761400000000000105),
(1761300000000000003, 1761400000000000106),
(1761300000000000003, 1761400000000000107),
(1761300000000000003, 1761400000000000108),
(1761300000000000003, 1761400000000000118),
(1761300000000000003, 1761400000000000123),
(1761300000000000003, 1761400000000000130),
(1761300000000000003, 1761400000000000131),
(1761300000000000003, 1761400000000000133),
(1761300000000000003, 1761400000000000500),
(1761300000000000003, 1761400000000000501),
(1761300000000000003, 1761400000000001001),
(1761300000000000003, 1761400000000001002),
(1761300000000000003, 1761400000000001003),
(1761300000000000003, 1761400000000001004),
(1761300000000000003, 1761400000000001005),
(1761300000000000003, 1761400000000001006),
(1761300000000000003, 1761400000000001007),
(1761300000000000003, 1761400000000001008),
(1761300000000000003, 1761400000000001009),
(1761300000000000003, 1761400000000001010),
(1761300000000000003, 1761400000000001011),
(1761300000000000003, 1761400000000001012),
(1761300000000000003, 1761400000000001013),
(1761300000000000003, 1761400000000001014),
(1761300000000000003, 1761400000000001015),
(1761300000000000003, 1761400000000001016),
(1761300000000000003, 1761400000000001017),
(1761300000000000003, 1761400000000001018),
(1761300000000000003, 1761400000000001019),
(1761300000000000003, 1761400000000001020),
(1761300000000000003, 1761400000000001021),
(1761300000000000003, 1761400000000001022),
(1761300000000000003, 1761400000000001023),
(1761300000000000003, 1761400000000001024),
(1761300000000000003, 1761400000000001025),
(1761300000000000003, 1761400000000001026),
(1761300000000000003, 1761400000000001027),
(1761300000000000003, 1761400000000001028),
(1761300000000000003, 1761400000000001029),
(1761300000000000003, 1761400000000001030),
(1761300000000000003, 1761400000000001031),
(1761300000000000003, 1761400000000001032),
(1761300000000000003, 1761400000000001033),
(1761300000000000003, 1761400000000001034),
(1761300000000000003, 1761400000000001035),
(1761300000000000003, 1761400000000001036),
(1761300000000000003, 1761400000000001037),
(1761300000000000003, 1761400000000001038),
(1761300000000000003, 1761400000000001039),
(1761300000000000003, 1761400000000001040),
(1761300000000000003, 1761400000000001041),
(1761300000000000003, 1761400000000001042),
(1761300000000000003, 1761400000000001043),
(1761300000000000003, 1761400000000001044),
(1761300000000000003, 1761400000000001045),
(1761300000000000003, 1761400000000001050),
(1761300000000000003, 1761400000000001061),
(1761300000000000003, 1761400000000001062),
(1761300000000000003, 1761400000000001063),
(1761300000000000003, 1761400000000001064),
(1761300000000000003, 1761400000000001065),
(1761300000000000003, 1761400000000001500),
(1761300000000000003, 1761400000000001501),
(1761300000000000003, 1761400000000001502),
(1761300000000000003, 1761400000000001503),
(1761300000000000003, 1761400000000001504),
(1761300000000000003, 1761400000000001505),
(1761300000000000003, 1761400000000001506),
(1761300000000000003, 1761400000000001507),
(1761300000000000003, 1761400000000001508),
(1761300000000000003, 1761400000000001509),
(1761300000000000003, 1761400000000001510),
(1761300000000000003, 1761400000000001511),
(1761300000000000003, 1761400000000001600),
(1761300000000000003, 1761400000000001601),
(1761300000000000003, 1761400000000001602),
(1761300000000000003, 1761400000000001603),
(1761300000000000003, 1761400000000001620),
(1761300000000000003, 1761400000000001621),
(1761300000000000003, 1761400000000001622),
(1761300000000000003, 1761400000000001623),
(1761300000000000003, 1761400000000011616),
(1761300000000000003, 1761400000000011618),
(1761300000000000003, 1761400000000011619),
(1761300000000000003, 1761400000000011622),
(1761300000000000003, 1761400000000011623),
(1761300000000000003, 1761400000000011629),
(1761300000000000003, 1761400000000011632),
(1761300000000000003, 1761400000000011633),
(1761300000000000003, 1761400000000011638),
(1761300000000000003, 1761400000000011639),
(1761300000000000003, 1761400000000011640),
(1761300000000000003, 1761400000000011641),
(1761300000000000003, 1761400000000011642),
(1761300000000000003, 1761400000000011643),
(1761300000000000003, 1761400000000011701),
(1761300000000000004, 1761400000000000005),
(1761300000000000004, 1761400000000001500),
(1761300000000000004, 1761400000000001501),
(1761300000000000004, 1761400000000001502),
(1761300000000000004, 1761400000000001503),
(1761300000000000004, 1761400000000001504),
(1761300000000000004, 1761400000000001505),
(1761300000000000004, 1761400000000001506),
(1761300000000000004, 1761400000000001507),
(1761300000000000004, 1761400000000001508),
(1761300000000000004, 1761400000000001509),
(1761300000000000004, 1761400000000001510),
(1761300000000000004, 1761400000000001511);

DROP TABLE IF EXISTS `flow_definition`;

CREATE TABLE `flow_definition` (
  `id` bigint NOT NULL COMMENT '主键id',
  `flow_code` varchar(40) NOT NULL COMMENT '流程编码',
  `flow_name` varchar(100) NOT NULL COMMENT '流程名称',
  `model_value` varchar(40) NOT NULL DEFAULT 'CLASSICS' COMMENT '设计器模型（CLASSICS经典模型 MIMIC仿钉钉模型）',
  `category` varchar(100) DEFAULT NULL COMMENT '流程类别',
  `version` varchar(20) NOT NULL COMMENT '流程版本',
  `is_publish` tinyint(1) NOT NULL DEFAULT '0' COMMENT '是否发布（0未发布 1已发布 9失效）',
  `form_custom` char(1) DEFAULT 'N' COMMENT '审批表单是否自定义（Y是 N否）',
  `form_path` varchar(100) DEFAULT NULL COMMENT '审批表单路径',
  `activity_status` tinyint(1) NOT NULL DEFAULT '1' COMMENT '流程激活状态（0挂起 1激活）',
  `listener_type` varchar(100) DEFAULT NULL COMMENT '监听器类型',
  `listener_path` varchar(400) DEFAULT NULL COMMENT '监听器路径',
  `ext` varchar(500) DEFAULT NULL COMMENT '业务详情 存业务表对象json字符串',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` varchar(64) DEFAULT '' COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` varchar(64) DEFAULT '' COMMENT '更新人',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志',
  `tenant_id` varchar(40) DEFAULT NULL COMMENT '租户id',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程定义表';

INSERT INTO `flow_definition` (`id`, `flow_code`, `flow_name`, `model_value`, `category`, `version`, `is_publish`, `form_custom`, `form_path`, `activity_status`, `listener_type`, `listener_path`, `ext`, `create_time`, `create_by`, `update_time`, `update_by`, `del_flag`, `tenant_id`) VALUES
(7473380005238673409, 'leave6', '请假申请-排他并行会签', 'CLASSICS', '1762300000000000103', '1', 1, 'N', '/workflow/leaveEdit/index', 1, NULL, NULL, NULL, '2026-06-18 22:24:02', '1761100000000000001', '2026-06-22 16:14:47', '1761100000000000001', '0', NULL),
(7474738325492994053, 'leave1', '请假申请-普通', 'CLASSICS', '1762300000000000103', '1', 1, 'N', '/workflow/leaveEdit/index', 1, NULL, NULL, NULL, '2026-06-22 16:22:08', '1761100000000000001', '2026-06-22 17:42:48', '1761100000000000001', '0', NULL),
(7474738325492994064, 'leave2', '请假申请-排他网关', 'CLASSICS', '1762300000000000103', '1', 1, 'N', '/workflow/leaveEdit/index', 1, NULL, NULL, NULL, '2026-06-22 16:22:48', '1761100000000000001', '2026-06-23 20:28:56', '1761100000000000001', '0', NULL),
(7474738325492994080, 'leave3', '请假申请-并行网关', 'CLASSICS', '1762300000000000103', '1', 1, 'N', '/workflow/leaveEdit/index', 1, NULL, NULL, NULL, '2026-06-22 16:22:57', '1761100000000000001', '2026-06-23 20:29:12', '1761100000000000001', '0', NULL),
(7474738325492994098, 'leave4', '请假申请-会签', 'CLASSICS', '1762300000000000103', '1', 1, 'N', '/workflow/leaveEdit/index', 1, NULL, NULL, NULL, '2026-06-22 16:23:04', '1761100000000000001', '2026-06-23 20:29:08', '1761100000000000001', '0', NULL),
(7474738325492994111, 'leave5', '请假申请-并行会签网关', 'CLASSICS', '1762300000000000103', '1', 1, 'N', '/workflow/leaveEdit/index', 1, NULL, NULL, NULL, '2026-06-22 16:23:09', '1761100000000000001', '2026-06-23 20:29:02', '1761100000000000001', '0', NULL);

-- 流程办理人重解析补偿任务表(outbox): 用户/角色/部门变更同事务写入,
-- 后台 worker 消费后对运行中实例重解析办理人(参考实现可靠补偿设计)
DROP TABLE IF EXISTS `flow_assignee_sync`;

CREATE TABLE `flow_assignee_sync` (
  `id` bigint NOT NULL COMMENT '主键(雪花)',
  `auth_type` varchar(16) NOT NULL COMMENT '变更对象类型: user/role/dept',
  `oper_type` varchar(16) NOT NULL COMMENT '操作类型: add/update/delete',
  `auth_code` varchar(64) DEFAULT NULL COMMENT '变更对象标识(用户名/角色key)',
  `user_ids` text COMMENT '波及用户ID(JSON数组)',
  `status` char(1) NOT NULL DEFAULT '0' COMMENT '0待处理 1已处理 2失败',
  `retry_count` int NOT NULL DEFAULT 0 COMMENT '已重试次数',
  `error_message` varchar(2000) DEFAULT NULL COMMENT '最近失败原因',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  PRIMARY KEY (`id`),
  KEY `idx_fas_status` (`status`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='流程办理人重解析补偿任务表';

DROP TABLE IF EXISTS `sys_social`;

CREATE TABLE `sys_social` (
  `id` bigint NOT NULL COMMENT '主键',
  `user_id` bigint NOT NULL COMMENT '用户ID',
  `auth_id` varchar(255) NOT NULL COMMENT '平台+平台唯一id',
  `source` varchar(255) NOT NULL COMMENT '用户来源',
  `open_id` varchar(255) DEFAULT NULL COMMENT '平台编号唯一id',
  `user_name` varchar(30) NOT NULL COMMENT '登录账号',
  `nick_name` varchar(30) DEFAULT '' COMMENT '用户昵称',
  `email` varchar(255) DEFAULT '' COMMENT '用户邮箱',
  `avatar` varchar(500) DEFAULT '' COMMENT '头像地址',
  `access_token` varchar(2000) NOT NULL COMMENT '用户的授权令牌',
  `expire_in` int DEFAULT NULL COMMENT '用户的授权令牌的有效期，部分平台可能没有',
  `refresh_token` varchar(2000) DEFAULT NULL COMMENT '刷新令牌，部分平台可能没有',
  `access_code` varchar(255) DEFAULT NULL COMMENT '平台的授权信息，部分平台可能没有',
  `union_id` varchar(255) DEFAULT NULL COMMENT '用户的 unionid',
  `scope` varchar(255) DEFAULT NULL COMMENT '授予的权限，部分平台可能没有',
  `token_type` varchar(255) DEFAULT NULL COMMENT '个别平台的授权信息，部分平台可能没有',
  `id_token` varchar(2000) DEFAULT NULL COMMENT 'id token，部分平台可能没有',
  `mac_algorithm` varchar(255) DEFAULT NULL COMMENT '小米平台用户的附带属性，部分平台可能没有',
  `mac_key` varchar(255) DEFAULT NULL COMMENT '小米平台用户的附带属性，部分平台可能没有',
  `code` varchar(255) DEFAULT NULL COMMENT '用户的授权code，部分平台可能没有',
  `oauth_token` varchar(255) DEFAULT NULL COMMENT 'Twitter平台用户的附带属性，部分平台可能没有',
  `oauth_token_secret` varchar(255) DEFAULT NULL COMMENT 'Twitter平台用户的附带属性，部分平台可能没有',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='社会化关系表';

DROP TABLE IF EXISTS `test_demo`;

CREATE TABLE `test_demo` (
  `id` bigint NOT NULL COMMENT '主键',
  `dept_id` bigint DEFAULT NULL COMMENT '部门id',
  `user_id` bigint DEFAULT NULL COMMENT '用户id',
  `order_num` int DEFAULT '0' COMMENT '排序号',
  `test_key` varchar(255) DEFAULT NULL COMMENT 'key键',
  `value` varchar(255) DEFAULT NULL COMMENT '值',
  `version` int DEFAULT '0' COMMENT '版本',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `create_by` bigint DEFAULT NULL COMMENT '创建人',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新人',
  `del_flag` int DEFAULT '0' COMMENT '删除标志',
  PRIMARY KEY (`id`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='测试单表';

INSERT INTO `test_demo` (`id`, `dept_id`, `user_id`, `order_num`, `test_key`, `value`, `version`, `create_dept`, `create_time`, `create_by`, `update_time`, `update_by`, `del_flag`) VALUES
(1762100000000000001, 1761000000000000102, 1761100000000000004, 1, '测试数据权限', '测试', 0, 1761000000000000103, '2026-06-14 15:36:17', 1761100000000000001, NULL, NULL, 0),
(1762100000000000002, 1761000000000000102, 1761100000000000003, 2, '子节点1', '111', 0, 1761000000000000103, '2026-06-14 15:36:17', 1761100000000000001, NULL, NULL, 0),
(1762100000000000003, 1761000000000000102, 1761100000000000003, 3, '子节点2', '222', 0, 1761000000000000103, '2026-06-14 15:36:17', 1761100000000000001, NULL, NULL, 0),
(1762100000000000004, 1761000000000000108, 1761100000000000004, 4, '测试数据', 'demo', 0, 1761000000000000103, '2026-06-14 15:36:17', 1761100000000000001, NULL, NULL, 0),
(1762100000000000005, 1761000000000000108, 1761100000000000003, 13, '子节点11', '1111', 0, 1761000000000000103, '2026-06-14 15:36:17', 1761100000000000001, NULL, NULL, 0),
(1762100000000000006, 1761000000000000108, 1761100000000000003, 12, '子节点22', '2222', 0, 1761000000000000103, '2026-06-14 15:36:17', 1761100000000000001, NULL, NULL, 0),
(1762100000000000007, 1761000000000000108, 1761100000000000003, 11, '子节点33', '3333', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762100000000000008, 1761000000000000108, 1761100000000000003, 10, '子节点44', '4444', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762100000000000009, 1761000000000000108, 1761100000000000003, 9, '子节点55', '5555', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762100000000000010, 1761000000000000108, 1761100000000000003, 8, '子节点66', '6666', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762100000000000011, 1761000000000000108, 1761100000000000003, 7, '子节点77', '7777', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762100000000000012, 1761000000000000108, 1761100000000000003, 6, '子节点88', '8888', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0),
(1762100000000000013, 1761000000000000108, 1761100000000000003, 5, '子节点99', '9999', 0, 1761000000000000103, '2026-06-14 15:36:18', 1761100000000000001, NULL, NULL, 0);

DROP TABLE IF EXISTS `sys_role`;

CREATE TABLE `sys_role` (
  `role_id` bigint NOT NULL COMMENT '角色ID',
  `role_name` varchar(30) NOT NULL COMMENT '角色名称',
  `role_key` varchar(100) NOT NULL COMMENT '角色权限字符串',
  `parent_id` bigint DEFAULT NULL COMMENT '上级角色ID(参考实现定制: 上级办理人解析)',
  `role_sort` int NOT NULL COMMENT '显示顺序',
  `data_scope` char(1) DEFAULT '1' COMMENT '数据范围（1：全部数据权限 2：自定数据权限 3：本部门数据权限 4：本部门及以下数据权限 5：仅本人数据权限 6：部门及以下或本人数据权限）',
  `menu_check_strictly` tinyint(1) DEFAULT '1' COMMENT '菜单树选择项是否关联显示',
  `dept_check_strictly` tinyint(1) DEFAULT '1' COMMENT '部门树选择项是否关联显示',
  `status` char(1) NOT NULL COMMENT '角色状态（0正常 1停用）',
  `del_flag` char(1) DEFAULT '0' COMMENT '删除标志（0代表存在 1代表删除）',
  `create_dept` bigint DEFAULT NULL COMMENT '创建部门',
  `create_by` bigint DEFAULT NULL COMMENT '创建者',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  `update_by` bigint DEFAULT NULL COMMENT '更新者',
  `update_time` datetime DEFAULT NULL COMMENT '更新时间',
  `remark` varchar(500) DEFAULT NULL COMMENT '备注',
  PRIMARY KEY (`role_id`),
  KEY `idx_sys_role_create_dept` (`create_dept`),
  KEY `idx_sys_role_create_by` (`create_by`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='角色信息表';

INSERT INTO `sys_role` (`role_id`, `role_name`, `role_key`, `role_sort`, `data_scope`, `menu_check_strictly`, `dept_check_strictly`, `status`, `del_flag`, `create_dept`, `create_by`, `create_time`, `update_by`, `update_time`, `remark`) VALUES
(1761300000000000001, '超级管理员', 'superadmin', 1, '1', 1, 1, '0', '0', 1761000000000000103, 1761100000000000001, '2026-06-14 15:35:55', NULL, NULL, '超级管理员'),
(7472533672114130955, '医务', 'med', 1, '1', 1, 1, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:41:48', 1761100000000000001, '2026-06-16 06:44:51', ''),
(7472533672114130957, '科主任', 'dept_mge', 2, '6', 1, 1, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:43:02', 1761100000000000001, '2026-06-22 06:27:36', ''),
(7472533672114130959, '护士长', 'nurse_mge', 2, '6', 1, 1, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:43:44', 1761100000000000001, '2026-06-16 06:45:16', ''),
(7472533672114130962, '医生', 'doctor', 3, '5', 1, 1, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:44:07', 1761100000000000001, '2026-06-16 06:45:25', ''),
(7472533672114130964, '护士', 'nurse', 3, '5', 1, 1, '0', '0', NULL, 1761100000000000001, '2026-06-16 06:44:21', 1761100000000000001, '2026-06-16 06:45:34', '');

SET FOREIGN_KEY_CHECKS = 1;
