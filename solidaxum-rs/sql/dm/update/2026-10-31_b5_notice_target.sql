-- 2026-10-31 B5 Task 4: 定向公告与已读回执
-- 影响表: sys_notice(新增 target_type / user_ids / send_status / send_time)
--         sys_notice_read(新表 + 唯一键 + 用户索引)
-- 是否幂等: 否(ALTER TABLE 加列);已执行则跳过 ALTER 段,重复执行会报列已存在。
--           CREATE TABLE IF NOT EXISTS 段可重复执行。
-- 方言: 达梦;MySQL 见 sql/update/2026-10-31_b5_notice_target.sql
-- 说明: 与 sql/dm/solidaxum_init.sql 的 B5 公告段一一对应;全新建库不需要本脚本。

ALTER TABLE sys_notice ADD COLUMN target_type VARCHAR(10) DEFAULT 'ALL';
ALTER TABLE sys_notice ADD COLUMN user_ids VARCHAR(2000) NULL;
ALTER TABLE sys_notice ADD COLUMN send_status CHAR(1) DEFAULT '0';
ALTER TABLE sys_notice ADD COLUMN send_time DATETIME NULL;
COMMENT ON COLUMN sys_notice.target_type IS '目标类型（ALL全部 USER指定用户）';
COMMENT ON COLUMN sys_notice.user_ids IS '目标用户ID串（逗号分隔）';
COMMENT ON COLUMN sys_notice.send_status IS '发送状态（0草稿 1已发布 2已撤回）';
COMMENT ON COLUMN sys_notice.send_time IS '发送时间';

CREATE TABLE IF NOT EXISTS sys_notice_read (
  id BIGINT NOT NULL,
  notice_id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  read_flag CHAR(1) DEFAULT '0',
  read_time DATETIME NULL,
  star_flag CHAR(1) DEFAULT '0',
  create_time DATETIME NULL,
  PRIMARY KEY (id),
  UNIQUE (notice_id, user_id)
);
CREATE INDEX idx_sys_notice_read_user ON sys_notice_read (user_id, read_flag);
COMMENT ON COLUMN sys_notice_read.id IS '主键';
COMMENT ON COLUMN sys_notice_read.notice_id IS '公告ID';
COMMENT ON COLUMN sys_notice_read.user_id IS '用户ID';
COMMENT ON COLUMN sys_notice_read.read_flag IS '已读标志（0未读 1已读）';
COMMENT ON COLUMN sys_notice_read.read_time IS '已读时间';
COMMENT ON COLUMN sys_notice_read.star_flag IS '星标标志（0否 1是）';
COMMENT ON COLUMN sys_notice_read.create_time IS '创建时间';
COMMENT ON TABLE sys_notice_read IS '公告已读回执表';
