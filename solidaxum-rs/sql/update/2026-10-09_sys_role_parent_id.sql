-- 来源需求:参考实现 sys_role 补 parent_id 列(上级办理人解析前置)
-- 影响表: sys_role
-- 是否幂等: 是(information_schema 判断,列已存在则跳过)
-- 方言: MySQL;达梦 / SQLite / PostgreSQL 存量库请按本文件意图自行调整。
-- 重复执行: 可任意重入,列存在时整段无副作用。
--
-- sys_role 增加 parent_id(参考实现定制列, 上级角色)。
-- 幂等: 列已存在时跳过(MySQL 8.0 无 IF NOT EXISTS for ADD COLUMN 的通用写法, 用 information_schema 判断)。
-- 动态 DDL 里的中文 COMMENT 依赖会话字符集, 先钉死(客户端默认 latin1 时会双重编码)。
SET NAMES utf8mb4;
SET @col := (
  SELECT COUNT(*) FROM information_schema.COLUMNS
  WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'sys_role' AND COLUMN_NAME = 'parent_id'
);
SET @ddl := IF(@col = 0,
  'ALTER TABLE `sys_role` ADD COLUMN `parent_id` bigint DEFAULT NULL COMMENT ''上级角色ID'' AFTER `role_key`',
  'SELECT 1');
PREPARE s FROM @ddl; EXECUTE s; DEALLOCATE PREPARE s;
