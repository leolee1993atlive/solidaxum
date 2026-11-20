-- 2026-10-31 B5 Task 4: 定向公告与已读回执
-- 影响表: sys_notice(新增 target_type / user_ids / send_status / send_time)
--         sys_notice_read(新表 + 唯一键 + 用户索引)
-- 是否幂等: 否(ALTER TABLE 加列);已执行则跳过 ALTER 段,重复执行会报列已存在。
--           CREATE TABLE IF NOT EXISTS 段可重复执行。
-- 方言: MySQL;其他方言见 sql/{dm,postgresql,sqlite}/update/
-- 说明: 与 sql/solidaxum_init.sql 的 B5 公告段一一对应;全新建库不需要本脚本。
SET NAMES utf8mb4;

ALTER TABLE `sys_notice`
  ADD COLUMN `target_type` varchar(10) DEFAULT 'ALL' COMMENT '目标类型（ALL全部 USER指定用户）' AFTER `status`,
  ADD COLUMN `user_ids` varchar(2000) DEFAULT NULL COMMENT '目标用户ID串（逗号分隔）' AFTER `target_type`,
  ADD COLUMN `send_status` char(1) DEFAULT '0' COMMENT '发送状态（0草稿 1已发布 2已撤回）' AFTER `user_ids`,
  ADD COLUMN `send_time` datetime DEFAULT NULL COMMENT '发送时间' AFTER `send_status`;

CREATE TABLE IF NOT EXISTS `sys_notice_read` (
  `id` bigint NOT NULL COMMENT '主键',
  `notice_id` bigint NOT NULL COMMENT '公告ID',
  `user_id` bigint NOT NULL COMMENT '用户ID',
  `read_flag` char(1) DEFAULT '0' COMMENT '已读标志（0未读 1已读）',
  `read_time` datetime DEFAULT NULL COMMENT '已读时间',
  `star_flag` char(1) DEFAULT '0' COMMENT '星标标志（0否 1是）',
  `create_time` datetime DEFAULT NULL COMMENT '创建时间',
  PRIMARY KEY (`id`),
  UNIQUE KEY `uk_sys_notice_read_notice_user` (`notice_id`, `user_id`),
  KEY `idx_sys_notice_read_user` (`user_id`, `read_flag`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='公告已读回执表';
