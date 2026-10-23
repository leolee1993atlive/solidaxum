//! message 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造(`data_source().context(Principal::..)`)并
//! 透传;message 是全豁免模块(SysMessageMapper 无 @DataPermission),
//! 读在 repo 内显式 without_row_scope。

use app_core::{error::AppResult, infra::Infrastructure};
use app_data::DataContext;

use crate::message::{
    model::message::MessageBoxVo, repository::message_repo::MessageRepository,
};

#[derive(Clone)]
pub struct MessageService {
    message_repo: MessageRepository,
}

impl MessageService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            message_repo: MessageRepository::new(infra.data_source()),
        }
    }

    /// 查询当前用户消息盒子数据（按 system/notice/workflow 分类）。
    pub async fn query_message_box(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<MessageBoxVo> {
        Ok(self.message_repo.select_message_box(ctx, user_id).await?)
    }
}
