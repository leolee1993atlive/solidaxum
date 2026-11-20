-- 2026-10-31 B5: 消息渠道配置与三 IM 发送日志
-- 影响面: sys_message_channel(新表+唯一键+种子) + sys_message_send_log(新表+索引)
--          + sys_menu(6 条:消息渠道页面 + 3 按钮、消息发送历史页面 + 1 按钮)
-- 是否幂等: 是(CREATE TABLE IF NOT EXISTS + REPLACE INTO 种子/菜单)
-- 方言: MySQL;其他方言见 sql/{dm,postgresql,sqlite}/update/
-- 说明: 与 sql/solidaxum_init.sql 的 B5 段一一对应;全新建库不需要本脚本。
SET NAMES utf8mb4;

CREATE TABLE IF NOT EXISTS sys_message_channel (
  channel_id BIGINT NOT NULL COMMENT '渠道配置ID',
  channel VARCHAR(32) NOT NULL COMMENT '渠道标识(DINGTALK/WECHAT/FEISHU)',
  channel_name VARCHAR(64) NOT NULL COMMENT '渠道名称',
  enabled CHAR(1) NOT NULL DEFAULT '0' COMMENT '是否启用（1是 0否）',
  client_id VARCHAR(255) DEFAULT NULL COMMENT '客户端ID(AppKey/CorpId/AppId)',
  client_secret VARCHAR(500) DEFAULT NULL COMMENT '客户端密钥(AES密文)',
  agent_id VARCHAR(64) DEFAULT NULL COMMENT '应用AgentId',
  agent_secret VARCHAR(500) DEFAULT NULL COMMENT '应用密钥(AES密文)',
  base_url VARCHAR(500) DEFAULT NULL COMMENT '自定义API基址',
  domain VARCHAR(255) DEFAULT NULL COMMENT '自定义域名',
  del_flag CHAR(1) NOT NULL DEFAULT '0' COMMENT '删除标志（0存在 2删除）',
  create_dept BIGINT NULL, create_by BIGINT NULL, create_time DATETIME NULL,
  update_by BIGINT NULL, update_time DATETIME NULL,
  remark VARCHAR(500) NULL,
  PRIMARY KEY (channel_id),
  UNIQUE KEY uk_sys_message_channel_channel (channel)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='消息渠道配置表';

REPLACE INTO sys_message_channel (channel_id, channel, channel_name, enabled, del_flag) VALUES
 (1762000000000000001, 'DINGTALK', '钉钉', '0', '0'),
 (1762000000000000002, 'WECHAT', '企业微信', '0', '0'),
 (1762000000000000003, 'FEISHU', '飞书', '0', '0');

CREATE TABLE IF NOT EXISTS sys_message_send_log (
  log_id BIGINT NOT NULL COMMENT '日志ID',
  tenant_id VARCHAR(20) DEFAULT '000000' COMMENT '租户ID',
  batch_id VARCHAR(64) DEFAULT NULL COMMENT '批次ID',
  biz_type VARCHAR(64) DEFAULT NULL COMMENT '业务类型',
  biz_id VARCHAR(64) DEFAULT NULL COMMENT '业务ID',
  channel VARCHAR(32) NOT NULL COMMENT '渠道(SSE/APP/SMS/WECHAT/DINGTALK/FEISHU)',
  recipient_user_ids TEXT COMMENT '收件人用户ID JSON数组',
  content LONGTEXT COMMENT '发送内容',
  status VARCHAR(16) NOT NULL COMMENT '状态(SKIPPED/SUCCESS/FAILED)',
  skip_reason VARCHAR(128) DEFAULT NULL COMMENT '跳过原因',
  error_message VARCHAR(2000) DEFAULT NULL COMMENT '错误信息(已脱敏)',
  provider_response VARCHAR(1000) DEFAULT NULL COMMENT '平台响应(截断)',
  create_dept BIGINT NULL, create_by BIGINT NULL, create_time DATETIME NULL,
  update_by BIGINT NULL, update_time DATETIME NULL,
  PRIMARY KEY (log_id),
  KEY idx_sys_message_send_log_channel_time (channel, create_time),
  KEY idx_sys_message_send_log_biz (biz_type, biz_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='消息发送日志表';

-- 菜单/按钮种子(幂等)
REPLACE INTO sys_menu (menu_id, menu_name, parent_id, order_num, path, component, query_param, is_frame, is_cache, menu_type, visible, status, perms, icon, active_menu, ext, create_time, remark) VALUES
 (1761400000000000160, '消息渠道', 1761400000000000001, 13, 'messageChannel', 'system/messageChannel/index', '', '1', '0', 'C', '0', '0', 'system:messageChannel:list', 'message', '', '', NOW(), '消息渠道配置菜单'),
 (1761400000000001070, '渠道查询', 1761400000000000160, 1, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:list', '#', '', '', NOW(), ''),
 (1761400000000001071, '渠道修改', 1761400000000000160, 2, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:edit', '#', '', '', NOW(), ''),
 (1761400000000001072, '渠道测试', 1761400000000000160, 3, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:test', '#', '', '', NOW(), '');

-- B5 消息发送历史菜单(系统管理 → 消息发送历史)
REPLACE INTO sys_menu (menu_id, menu_name, parent_id, order_num, path, component, query_param, is_frame, is_cache, menu_type, visible, status, perms, icon, active_menu, ext, create_time, remark) VALUES
 (1761400000000000170, '消息发送历史', 1761400000000000001, 14, 'messageLog', 'system/messageLog/index', '', '1', '0', 'C', '0', '0', 'system:messageChannel:list', 'log', '', '', NOW(), '消息发送历史菜单'),
 (1761400000000001073, '历史查询', 1761400000000000170, 1, '', NULL, '', '1', '0', 'F', '0', '0', 'system:messageChannel:list', '#', '', '', NOW(), '');
