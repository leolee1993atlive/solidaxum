//! translation 服务(**全豁免**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档;错误统一 `AppError`(经 `From<DataError>`)。**裁定(无 Java Mapper 对应物)**:
//! Java 侧翻译走 `@Translation(USER_ID_TO_NAME / DEPT_ID_TO_NAME /
//! OSS_ID_TO_URL)` 处理器,内部是无注解的批量 id 查询 —— **翻译数据
//! 非行级**,列表页里越权行的 id 照样要翻出名字,否则 VO 缺列。统一锚:
//! **翻译数据非行级(无 Java Mapper 对应物)**。由此每个读方法显式
//! `without_row_scope()`(sys_user/sys_dept 是受辖实体,此处以翻译语义
//! 豁免 —— 与列表页主体数据的范围口径正交)。
//!
//! | 本文件方法 | Java 锚(@Translation 处理器) | 漏斗路径 |
//! |---|---|---|
//! | `user_id_to_name` | `USER_ID_TO_NAME`(UserNameTranslationImpl → 无注解批量 user 查询) | `without_row_scope()`;软删过滤漏斗照施 |
//! | `dept_id_to_name` | `DEPT_ID_TO_NAME`(DeptNameTranslationImpl → 无注解批量 dept 查询) | `without_row_scope()`;同上 |
//! | `oss_id_to_url` | `OSS_ID_TO_URL`(OssUrlTranslationImpl → 无注解批量 oss 查询) | `without_row_scope()`(sys_oss 豁免实体) |
//! | `dict_value_to_label` | `DICT_TYPE_TO_LABEL`(走字典缓存,不经 DB) | 缓存旁路在 dict_data_service,落库读豁免 |
//!
//! **整行读投影**:逃生舱无 into_tuple 终端,`select_only().column(..)`
//! 投影不可用,整行读后内存投影(menu `select_menu_ids_by_role_id`
//! 同款);id 集合已收窄,整行读的额外列开销可忽略。

use std::collections::HashMap;

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
};
use app_data::{
    DataContext,
    sea_orm::{ColumnTrait, EntityTrait, QueryFilter},
};

use crate::{
    dept::repository::dept_repo,
    dict::service::dict_data_service::DictDataService,
    oss::repository::oss_repo, user::repository::user_repo,
};

#[derive(Clone)]
pub struct TranslationService {
    dict_data_service: Option<DictDataService>,
}

impl TranslationService {
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            dict_data_service: Some(DictDataService::new(infra)),
        }
    }

    /// 测试用构造函数 (不依赖 Redis;dict 翻译面不可用,同原 with_db 语义)
    #[doc(hidden)]
    pub fn without_dict_cache() -> Self {
        Self {
            dict_data_service: None,
        }
    }

    /// user_id → user_name (批量)
    ///
    /// 豁免裁定:翻译数据非行级(无 Java Mapper 对应物,@Translation
    /// 处理器是无注解批量查询)→ `without_row_scope()`;软删过滤由
    /// 漏斗照施(原手工 DelFlag 过滤已删)。
    pub async fn user_id_to_name(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<HashMap<i64, String>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = ctx
            .select(
                user_repo::Entity::find()
                    .filter(user_repo::Column::UserId.is_in(ids.to_vec())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(rows.into_iter().map(|u| (u.user_id, u.user_name)).collect())
    }

    /// dept_id → dept_name (批量)
    ///
    /// 豁免裁定:翻译数据非行级(无 Java Mapper 对应物)→
    /// `without_row_scope()`;软删过滤由漏斗照施(原手工 DelFlag 已删)。
    pub async fn dept_id_to_name(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<HashMap<i64, String>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = ctx
            .select(
                dept_repo::Entity::find()
                    .filter(dept_repo::Column::DeptId.is_in(ids.to_vec())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(rows.into_iter().map(|d| (d.dept_id, d.dept_name)).collect())
    }

    /// oss_id → 访问 URL (批量, 对齐 Java `@Translation(OSS_ID_TO_URL)`)
    ///
    /// 豁免裁定:翻译数据非行级(无 Java Mapper 对应物;sys_oss 本就是
    /// 豁免实体)→ `without_row_scope()`。
    pub async fn oss_id_to_url(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<HashMap<i64, String>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = ctx
            .select(
                oss_repo::Entity::find()
                    .filter(oss_repo::Column::OssId.is_in(ids.to_vec())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(rows.into_iter().map(|o| (o.oss_id, o.url)).collect())
    }

    /// 单值 oss_id → URL
    pub async fn oss_id_to_url_single(
        &self,
        ctx: &DataContext<'_>,
        id: Option<i64>,
    ) -> AppResult<Option<String>> {
        let Some(id) = id else {
            return Ok(None);
        };
        Ok(self.oss_id_to_url(ctx, &[id]).await?.get(&id).cloned())
    }

    /// dict_type + dict_value → dict_label (批量, 使用 dict_data_service 缓存)
    #[allow(dead_code)]
    pub async fn dict_value_to_label(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
        values: &[&str],
    ) -> AppResult<HashMap<String, String>> {
        let svc = self.dict_data_service.as_ref().ok_or_else(|| {
            AppError::Internal("字典翻译不可用 (无Redis)".into())
        })?;
        let all = svc.get_by_type(ctx, dict_type).await?;
        let map: HashMap<String, String> = all
            .iter()
            .map(|d| (d.dict_value.clone(), d.dict_label.clone()))
            .collect();
        let result: HashMap<String, String> = values
            .iter()
            .filter_map(|v| {
                map.get(*v).map(|label| (v.to_string(), label.clone()))
            })
            .collect();
        Ok(result)
    }
}
