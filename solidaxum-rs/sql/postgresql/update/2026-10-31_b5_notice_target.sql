-- 2026-10-31 B5 Task 4: 定向公告与已读回执
-- 影响表: sys_notice(新增 target_type / user_ids / send_status / send_time)
--         sys_notice_read(新表 + 唯一键 + 用户索引)
-- 是否幂等: 是(ADD COLUMN IF NOT EXISTS + CREATE TABLE IF NOT EXISTS)
-- 方言: PostgreSQL;MySQL 见 sql/update/2026-10-31_b5_notice_target.sql
-- 说明: 与 sql/postgresql/solidaxum_init.sql 的 B5 公告段一一对应;全新建库不需要本脚本。

ALTER TABLE sys_notice ADD COLUMN IF NOT EXISTS target_type varchar(10) DEFAULT 'ALL';
ALTER TABLE sys_notice ADD COLUMN IF NOT EXISTS user_ids varchar(2000) DEFAULT NULL;
ALTER TABLE sys_notice ADD COLUMN IF NOT EXISTS send_status varchar(1) DEFAULT '0';
ALTER TABLE sys_notice ADD COLUMN IF NOT EXISTS send_time TIMESTAMPTZ DEFAULT NULL;
COMMENT ON COLUMN sys_notice.target_type IS '目标类型（ALL全部 USER指定用户）';
COMMENT ON COLUMN sys_notice.user_ids IS '目标用户ID串（逗号分隔）';
COMMENT ON COLUMN sys_notice.send_status IS '发送状态（0草稿 1已发布 2已撤回）';
COMMENT ON COLUMN sys_notice.send_time IS '发送时间';

CREATE TABLE IF NOT EXISTS sys_notice_read (
  id bigint NOT NULL,
  notice_id bigint NOT NULL,
  user_id bigint NOT NULL,
  read_flag varchar(1) DEFAULT '0',
  read_time TIMESTAMPTZ DEFAULT NULL,
  star_flag varchar(1) DEFAULT '0',
  create_time TIMESTAMPTZ DEFAULT NULL,
  PRIMARY KEY (id)
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_notice_read_notice_user ON sys_notice_read (notice_id, user_id);
CREATE INDEX IF NOT EXISTS idx_sys_notice_read_user ON sys_notice_read (user_id, read_flag);
COMMENT ON COLUMN sys_notice_read.id IS '主键';
COMMENT ON COLUMN sys_notice_read.notice_id IS '公告ID';
COMMENT ON COLUMN sys_notice_read.user_id IS '用户ID';
COMMENT ON COLUMN sys_notice_read.read_flag IS '已读标志（0未读 1已读）';
COMMENT ON COLUMN sys_notice_read.read_time IS '已读时间';
COMMENT ON COLUMN sys_notice_read.star_flag IS '星标标志（0否 1是）';
COMMENT ON COLUMN sys_notice_read.create_time IS '创建时间';
COMMENT ON TABLE sys_notice_read IS '公告已读回执表';
