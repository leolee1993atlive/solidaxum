-- 2026-10-31 B5 Task 4: 定向公告与已读回执
-- 影响表: sys_notice(新增 target_type / user_ids / send_status / send_time)
--         sys_notice_read(新表 + 唯一键 + 用户索引)
-- 是否幂等: 否(ALTER TABLE 加列);已执行则跳过 ALTER 段,重复执行会报列已存在。
--           CREATE TABLE/INDEX IF NOT EXISTS 段可重复执行。
-- 方言: SQLite;MySQL 见 sql/update/2026-10-31_b5_notice_target.sql
-- 说明: 与 sql/sqlite/solidaxum_init.sql 的 B5 公告段一一对应;全新建库不需要本脚本。

ALTER TABLE sys_notice ADD COLUMN target_type VARCHAR(10) DEFAULT 'ALL';
ALTER TABLE sys_notice ADD COLUMN user_ids VARCHAR(2000) DEFAULT NULL;
ALTER TABLE sys_notice ADD COLUMN send_status CHAR(1) DEFAULT '0';
ALTER TABLE sys_notice ADD COLUMN send_time DATETIME DEFAULT NULL;

CREATE TABLE IF NOT EXISTS sys_notice_read (
  id BIGINT NOT NULL,
  notice_id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  read_flag CHAR(1) DEFAULT '0',
  read_time DATETIME DEFAULT NULL,
  star_flag CHAR(1) DEFAULT '0',
  create_time DATETIME DEFAULT NULL,
  PRIMARY KEY (id),
  UNIQUE (notice_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_sys_notice_read_user ON sys_notice_read (user_id, read_flag);
