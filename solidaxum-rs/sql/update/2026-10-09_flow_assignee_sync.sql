-- 来源需求:参考实现 Outbox 办理人补偿机制
-- 影响表: flow_assignee_sync(新增)
-- 是否幂等: 是(information_schema 判断,表已存在则跳过)
-- 方言: MySQL;达梦 / SQLite / PostgreSQL 存量库请按本文件意图自行调整
--   (建表语句见 sql/dm、sql/sqlite、sql/postgresql 下的 solidaxum_init.sql)。
-- 重复执行: 可任意重入,表存在时整段无副作用。
--
-- flow_assignee_sync: 流程办理人重解析补偿任务表(outbox)。
-- 用户/角色/部门变更同事务写入,后台 worker 消费后对运行中实例
-- 重解析办理人并 diff 增删 flow_user(参考实现可靠补偿设计,
-- 替代旧 Java 系统脆弱的 @Async 事件链)。
-- 动态 DDL 里的中文 COMMENT 依赖会话字符集, 先钉死(客户端默认 latin1 时会双重编码)。
SET NAMES utf8mb4;
SET @tbl := (
  SELECT COUNT(*) FROM information_schema.TABLES
  WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'flow_assignee_sync'
);
SET @ddl := IF(@tbl = 0,
  'CREATE TABLE `flow_assignee_sync` (
    `id` bigint NOT NULL COMMENT ''主键(雪花)'',
    `auth_type` varchar(16) NOT NULL COMMENT ''变更对象类型: user/role/dept'',
    `oper_type` varchar(16) NOT NULL COMMENT ''操作类型: add/update/delete'',
    `auth_code` varchar(64) DEFAULT NULL COMMENT ''变更对象标识(用户名/角色key)'',
    `user_ids` text COMMENT ''波及用户ID(JSON数组)'',
    `status` char(1) NOT NULL DEFAULT ''0'' COMMENT ''0待处理 1已处理 2失败'',
    `retry_count` int NOT NULL DEFAULT 0 COMMENT ''已重试次数'',
    `error_message` varchar(2000) DEFAULT NULL COMMENT ''最近失败原因'',
    `create_time` datetime DEFAULT NULL COMMENT ''创建时间'',
    `update_time` datetime DEFAULT NULL COMMENT ''更新时间'',
    PRIMARY KEY (`id`),
    KEY `idx_fas_status` (`status`)
  ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT=''流程办理人重解析补偿任务表''',
  'SELECT 1');
PREPARE s FROM @ddl; EXECUTE s; DEALLOCATE PREPARE s;
