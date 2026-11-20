//! 公告已读回执仓储(`sys_notice_read`)。
//!
//! 公告是全站内容(SysNoticeMapper 无 @DataPermission),回执表只按
//! `(notice_id,user_id)` 表达个人已读/星标状态,同样不参与行级范围;
//! 读显式 `without_row_scope()`,写由漏斗盖 `create_time`。

use app_data::sea_orm::{
    self, ActiveModelBehavior,
    ActiveValue::{NotSet, Set},
    ColumnTrait, Condition, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter,
};
use app_data::{DataContext, DataError, DataSource};

use app_core::util::snowflake;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_notice_read")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub notice_id: i64,
    pub user_id: i64,
    pub read_flag: String,
    pub read_time: Option<DateTimeUtc>,
    pub star_flag: String,
    pub create_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone)]
pub struct NoticeReadRepository {
    ds: std::sync::Arc<DataSource>,
}

impl NoticeReadRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 查询指定用户在一批公告上的回执行。
    pub async fn select_by_notice_ids(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        notice_ids: &[i64],
    ) -> Result<Vec<Model>, DataError> {
        if notice_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = ctx
            .select(
                Entity::find()
                    .filter(Column::UserId.eq(user_id))
                    .filter(Column::NoticeId.is_in(notice_ids.to_vec())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(rows)
    }

    /// 为公告的指定用户补建回执行(已存在则跳过),发布时使用。
    pub async fn create_missing(
        &self,
        ctx: &DataContext<'_>,
        notice_id: i64,
        user_ids: &[i64],
    ) -> Result<(), DataError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let existing = ctx
            .select(Entity::find().filter(Column::NoticeId.eq(notice_id)))
            .without_row_scope()
            .all()
            .await?;
        let existing: std::collections::HashSet<i64> =
            existing.into_iter().map(|m| m.user_id).collect();
        let rows: Vec<ActiveModel> = user_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .filter(|uid| !existing.contains(uid))
            .map(|user_id| ActiveModel {
                id: Set(snowflake::next_id()),
                notice_id: Set(notice_id),
                user_id: Set(user_id),
                read_flag: Set("0".to_string()),
                read_time: NotSet,
                star_flag: Set("0".to_string()),
                create_time: NotSet,
            })
            .collect();
        ctx.insert_many(rows).await?;
        Ok(())
    }

    /// 标记已读(幂等):存在则更新,不存在则插入。
    pub async fn mark_read(
        &self,
        ctx: &DataContext<'_>,
        notice_id: i64,
        user_id: i64,
        read_time: DateTimeUtc,
    ) -> Result<(), DataError> {
        let affected = ctx
            .update_where::<Entity>(
                Condition::all()
                    .add(Column::NoticeId.eq(notice_id))
                    .add(Column::UserId.eq(user_id)),
                ActiveModel {
                    read_flag: Set("1".to_string()),
                    read_time: Set(Some(read_time)),
                    ..Default::default()
                },
            )
            .await?;
        if affected > 0 {
            return Ok(());
        }
        // 并发首读:唯一键 (notice_id,user_id) 冲突说明对方已插入已读行,
        // 语义等价,吞掉 Conflict 保持幂等。
        match ctx
            .insert_returning_nothing(ActiveModel {
                id: Set(snowflake::next_id()),
                notice_id: Set(notice_id),
                user_id: Set(user_id),
                read_flag: Set("1".to_string()),
                read_time: Set(Some(read_time)),
                star_flag: Set("0".to_string()),
                create_time: NotSet,
            })
            .await
        {
            Ok(()) | Err(DataError::Conflict) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
