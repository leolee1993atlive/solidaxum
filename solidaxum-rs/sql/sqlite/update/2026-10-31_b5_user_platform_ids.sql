-- 2026-10-31 B5 Task 2: sys_user 三平台推送 ID
-- 影响表: sys_user(新增 wechat_user_id / dingtalk_user_id / feishu_user_id)
-- 是否幂等: 否(ALTER TABLE 加列);已执行则跳过本段,重复执行会报列已存在。
-- 方言: SQLite;MySQL 见 sql/update/2026-10-31_b5_user_platform_ids.sql
-- 说明: 与 sql/sqlite/solidaxum_init.sql 的 B5 段一一对应;全新建库不需要本脚本。

ALTER TABLE sys_user ADD COLUMN wechat_user_id VARCHAR(128) DEFAULT NULL;
ALTER TABLE sys_user ADD COLUMN dingtalk_user_id VARCHAR(128) DEFAULT NULL;
ALTER TABLE sys_user ADD COLUMN feishu_user_id VARCHAR(128) DEFAULT NULL;
