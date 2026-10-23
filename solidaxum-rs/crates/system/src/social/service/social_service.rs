//! social 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造(`data_source().context(Principal::..)`)并
//! 透传;social 是全豁免模块(SysSocialMapper 无 @DataPermission),
//! 读在 repo 内显式 without_row_scope,软删写的审计由漏斗盖。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
};
use app_data::DataContext;

use crate::social::{
    model::social::{Social, SocialVo},
    repository::social_repo::SocialRepository,
};

#[derive(Clone)]
pub struct SocialService {
    social_repo: SocialRepository,
}

impl SocialService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            social_repo: SocialRepository::new(infra.data_source()),
        }
    }

    pub async fn list_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<Vec<SocialVo>> {
        let socials = self.social_repo.select_by_user_id(ctx, user_id).await?;
        Ok(socials.iter().map(Social::to_vo).collect())
    }

    pub async fn delete_by_id_for_user(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        user_id: i64,
    ) -> AppResult<()> {
        self.social_repo
            .soft_delete_by_id_for_user(ctx, id, user_id)
            .await?;
        Ok(())
    }

    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Social> {
        self.social_repo
            .select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("社交账号不存在 (id={})", id))
            })
    }
}
