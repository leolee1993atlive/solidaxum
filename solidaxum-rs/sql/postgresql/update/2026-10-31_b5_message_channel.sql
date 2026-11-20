-- 2026-10-31 B5: 消息渠道配置与三 IM 发送日志
-- 影响面: sys_message_channel(新表+唯一键+种子) + sys_message_send_log(新表+索引)
--          + sys_menu(6 条:消息渠道页面 + 3 按钮、消息发送历史页面 + 1 按钮)
-- 是否幂等: 是(CREATE TABLE IF NOT EXISTS + ON CONFLICT DO NOTHING + 先删后插菜单)
-- 方言: PostgreSQL;MySQL 见 sql/update/2026-10-31_b5_message_channel.sql
-- 说明: 与 sql/postgresql/solidaxum_init.sql 的 B5 段一一对应;全新建库不需要本脚本。

CREATE TABLE IF NOT EXISTS sys_message_channel (
  channel_id BIGINT NOT NULL,
  channel VARCHAR(32) NOT NULL,
  channel_name VARCHAR(64) NOT NULL,
  enabled VARCHAR(1) NOT NULL DEFAULT '0',
  client_id VARCHAR(255) DEFAULT NULL,
  client_secret VARCHAR(500) DEFAULT NULL,
  agent_id VARCHAR(64) DEFAULT NULL,
  agent_secret VARCHAR(500) DEFAULT NULL,
  base_url VARCHAR(500) DEFAULT NULL,
  domain VARCHAR(255) DEFAULT NULL,
  del_flag VARCHAR(1) NOT NULL DEFAULT '0',
  create_dept BIGINT NULL, create_by BIGINT NULL, create_time TIMESTAMPTZ NULL,
  update_by BIGINT NULL, update_time TIMESTAMPTZ NULL,
  remark VARCHAR(500) NULL,
  PRIMARY KEY (channel_id),
  CONSTRAINT uk_sys_message_channel_channel UNIQUE (channel)
);
COMMENT ON TABLE sys_message_channel IS '消息渠道配置表';

INSERT INTO sys_message_channel (channel_id, channel, channel_name, enabled, del_flag) VALUES
 (1762000000000000001, 'DINGTALK', '钉钉', '0', '0'),
 (1762000000000000002, 'WECHAT', '企业微信', '0', '0'),
 (1762000000000000003, 'FEISHU', '飞书', '0', '0')
ON CONFLICT (channel) DO NOTHING;

CREATE TABLE IF NOT EXISTS sys_message_send_log (
  log_id BIGINT NOT NULL,
  tenant_id VARCHAR(20) DEFAULT '000000',
  batch_id VARCHAR(64) DEFAULT NULL,
  biz_type VARCHAR(64) DEFAULT NULL,
  biz_id VARCHAR(64) DEFAULT NULL,
  channel VARCHAR(32) NOT NULL,
  recipient_user_ids TEXT,
  content TEXT,
  status VARCHAR(16) NOT NULL,
  skip_reason VARCHAR(128) DEFAULT NULL,
  error_message VARCHAR(2000) DEFAULT NULL,
  provider_response VARCHAR(1000) DEFAULT NULL,
  create_dept BIGINT NULL, create_by BIGINT NULL, create_time TIMESTAMPTZ NULL,
  update_by BIGINT NULL, update_time TIMESTAMPTZ NULL,
  PRIMARY KEY (log_id)
);
CREATE INDEX IF NOT EXISTS idx_sys_message_send_log_channel_time ON sys_message_send_log (channel, create_time);
CREATE INDEX IF NOT EXISTS idx_sys_message_send_log_biz ON sys_message_send_log (biz_type, biz_id);
COMMENT ON TABLE sys_message_send_log IS '消息发送日志表';

-- 菜单/按钮种子(幂等:先删同 id 再插)
DELETE FROM sys_menu WHERE menu_id IN (1761400000000000160, 1761400000000001070, 1761400000000001071, 1761400000000001072, 1761400000000000170, 1761400000000001073);
INSERT INTO sys_menu (menu_id, menu_name, parent_id, order_num, path, component, query_param, is_frame, is_cache, menu_type, visible, status, perms, icon, active_menu, ext, create_time, remark) VALUES
 (1761400000000000160, '消息渠道', 1761400000000000001, 13, 'messageChannel', 'system/messageChannel/index', '', '1', '0', 'C', '0', '0', 'system:messageChannel:list', 'message', '', '', NOW(), '消息渠道配置菜单'),
 (1761400000000001070, '渠道查询', 1761400000000000160, 1, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:list', '#', '', '', NOW(), ''),
 (1761400000000001071, '渠道修改', 1761400000000000160, 2, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:edit', '#', '', '', NOW(), ''),
 (1761400000000001072, '渠道测试', 1761400000000000160, 3, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:test', '#', '', '', NOW(), '');

-- B5 消息发送历史菜单(系统管理 → 消息发送历史)
INSERT INTO sys_menu (menu_id, menu_name, parent_id, order_num, path, component, query_param, is_frame, is_cache, menu_type, visible, status, perms, icon, active_menu, ext, create_time, remark) VALUES
 (1761400000000000170, '消息发送历史', 1761400000000000001, 14, 'messageLog', 'system/messageLog/index', '', '1', '0', 'C', '0', '0', 'system:messageChannel:list', 'log', '', '', NOW(), '消息发送历史菜单'),
 (1761400000000001073, '历史查询', 1761400000000000170, 1, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:list', '#', '', '', NOW(), '');
