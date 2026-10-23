//! notice 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysNoticeMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 公告是全站内容,数据权限过滤对它无业务含义。
//! 统一锚:**SysNoticeMapper 无 @DataPermission —— 公告是全站内容**。
//! 由此每个读方法显式 `without_row_scope()`;sys_notice 的 DataPolicy
//! 落 CONSTRAINED=false,无软删列。
//!
//! | 本文件方法 | Java 锚(SysNoticeController/ServiceImpl → SysNoticeMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | getInfo(Controller:61)→ `selectNoticeById`(ServiceImpl:59)→ selectVoById | ❌ | `without_row_scope()` |
//! | `insert` | add(Controller:75)→ `insertNotice`(ServiceImpl:85,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:108)→ `updateNotice`(ServiceImpl:99,写路径) | — | `ctx.update`(豁免实体;审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_id`(物理) | remove(Controller:121)→ `deleteNoticeByIds`(ServiceImpl:113)→ deleteByIds | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//! | `select_page` | list(Controller:49)→ `selectPageNoticeList`(ServiceImpl:46)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | select_page 的 create_by_name 子查询 | ServiceImpl `buildQueryWrapper` 的 createByName 参数(SysNoticeServiceImpl:86-87:`eq` 精确匹配 + `voOne` 单用户 → eq(createBy, userId);Rust 的 contains + `is_in` 集合是**先存形状差异**,未改语义) | ❌(sys_user 读亦无注解) | 搜索辅助读 `without_row_scope()`(post belong_subtree 同裁定:查询条件,非权限) |
//!
//! **create_by_name 搜索辅助读**:按用户名解析 user_id 集是搜索意图
//! ("想搜谁发的"),不是权限("能看谁的")—— 与漏斗行级范围正交,故
//! sys_user 子查询显式 `without_row_scope()`(sys_user 是受辖实体,
//! 此处以搜索语义豁免);软删过滤由漏斗照施 —— Java 的
//! `userMapper.lambda().eq(...).voOne()` 同样被 SysUser 的 @TableLogic
//! 自动追加 del_flag='0',两边存活语义一致(钉测见
//! `create_by_name_excludes_soft_deleted_users`)。

use app_core::{
    model::entity::BaseEntity, repo::str_trim_or_empty, vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError, DataSource,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
        DeriveRelation, EntityTrait, EnumIter, Order, PrimaryKeyTrait,
        QueryFilter, QueryOrder,
    },
};

use crate::{
    notice::model::notice::{Notice, NoticeListVo, NoticePageQuery},
    user::repository::user_repo,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_notice")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub notice_id: i64,
    pub notice_title: String,
    pub notice_type: String,
    pub notice_content: Option<String>,
    pub status: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for Notice {
    fn from(m: &Model) -> Self {
        Notice {
            notice_id: Some(m.notice_id),
            notice_title: m.notice_title.clone(),
            notice_type: m.notice_type.clone(),
            notice_content: m.notice_content.clone(),
            status: m.status.clone(),
            remark: m.remark.clone(),
            base_entity: BaseEntity {
                create_dept: m.create_dept,
                create_by: m.create_by,
                create_time: m.create_time,
                update_by: m.update_by,
                update_time: m.update_time,
            },
        }
    }
}

/// Notice → ActiveModel。审计五列只搬运领域层已持有的值;**写入时由
/// 漏斗统一盖章**(insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn notice_to_active_model(notice: &Notice) -> ActiveModel {
    ActiveModel {
        notice_id: match notice.notice_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        notice_title: Set(notice.notice_title.clone()),
        notice_type: Set(notice.notice_type.clone()),
        notice_content: Set(notice.notice_content.clone()),
        status: Set(notice.status.clone()),
        create_dept: Set(notice.base_entity.create_dept),
        create_by: Set(notice.base_entity.create_by),
        create_time: Set(notice.base_entity.create_time),
        update_by: Set(notice.base_entity.update_by),
        update_time: Set(notice.base_entity.update_time),
        remark: Set(notice.remark.clone()),
    }
}

#[derive(Clone)]
pub struct NoticeRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl NoticeRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,dict_type_repo 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:SysNoticeMapper 无 @DataPermission —— 公告是全站内容
    /// (Java 锚 selectNoticeById ServiceImpl:59 → selectVoById)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        notice_id: &i64,
    ) -> Result<Option<Notice>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*notice_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Notice::from))
    }

    /// 新增公告。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 insertNotice ServiceImpl:85)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        notice: &Notice,
    ) -> Result<(), DataError> {
        let active = notice_to_active_model(notice);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层
    /// 原值),WHERE = 主键(豁免实体),0 行 → NotFound(原"先查后改"
    /// 预检随漏斗 WHERE 删除,service 侧先读拼领域对象,语义相同;
    /// Java 锚 updateNotice ServiceImpl:99)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        notice: &Notice,
        notice_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = notice_to_active_model(notice);
        active.notice_id = Set(*notice_id);
        ctx.update(active).await?;
        Ok(())
    }

    /// 物理删除公告(sys_notice 无 @TableLogic,Java 侧同为物理删)。
    /// 漏斗 delete:WHERE = 主键,0 行 → NotFound(Java 锚
    /// deleteNoticeByIds ServiceImpl:113)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        notice_id: &i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(*notice_id).await
    }

    /// 分页查询公告列表。
    ///
    /// 豁免裁定:SysNoticeMapper 无 @DataPermission —— 公告是全站内容
    /// (Java 锚 list → selectPageNoticeList ServiceImpl:46 →
    /// selectVoPage;排序在 Select 构件上,total 为全量口径)。
    /// create_by_name 的用户名子查询见模块头的搜索辅助读裁定。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &NoticePageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<NoticeListVo>, DataError> {
        let notice_title = str_trim_or_empty(&query.notice_title);
        let notice_type = str_trim_or_empty(&query.notice_type);
        let status = str_trim_or_empty(&query.status);
        let create_by_name = str_trim_or_empty(&query.create_by_name);

        let mut select = Entity::find();

        if !notice_title.is_empty() {
            select = select.filter(Column::NoticeTitle.contains(&notice_title));
        }
        if !notice_type.is_empty() {
            select = select.filter(Column::NoticeType.eq(&notice_type));
        }
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }
        if !create_by_name.is_empty() {
            // 搜索辅助读:按用户名解析 user_id 集合(查询条件,非权限;
            // post belong_subtree_condition 同裁定)。漏斗照施 sys_user
            // 的软删存活语义(对齐 Java @TableLogic)。
            let users = ctx
                .select(user_repo::Entity::find().filter(
                    user_repo::Column::UserName.contains(&create_by_name),
                ))
                .without_row_scope()
                .all()
                .await?;
            let user_ids: Vec<i64> =
                users.into_iter().map(|u| u.user_id).collect();
            if user_ids.is_empty() {
                return Ok(TableDataInfo::empty());
            }
            select = select.filter(Column::CreateBy.is_in(user_ids));
        }

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<NoticeListVo> = page
            .rows
            .iter()
            .map(Notice::from)
            .map(|n: Notice| n.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("notice_id") {
        "notice_id" => Column::NoticeId,
        "notice_title" => Column::NoticeTitle,
        "notice_type" => Column::NoticeType,
        "create_time" => Column::CreateTime,
        _ => Column::NoticeId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}

#[cfg(test)]
mod ddl_tests {
    /// PG 建表脚本。只在测试里读,不进生产二进制。
    const PG_INIT: &str =
        include_str!("../../../../../sql/postgresql/solidaxum_init.sql");

    /// `Model::notice_content` 是 `Option<String>`,PG 侧的列就必须是 text。
    ///
    /// 这条曾经不成立:PG 脚本把 MySQL 的 `longblob` 直译成了 `BYTEA`,于是
    /// PostgreSQL 上建公告返回「字段 "notice_content" 的类型为 bytea, 但表达式
    /// 的类型为 text」、读列表返回解码失败,而 MySQL 上一切正常 —— 驱动对
    /// String 与 blob 互转宽容,所以单跑 MySQL 完全看不出来。RuoYi 官方的 PG
    /// 脚本用的也是 text。
    #[test]
    fn pg_notice_content_is_text_not_bytea() {
        assert!(
            PG_INIT.contains("notice_content text,"),
            "PG 建表脚本里 sys_notice.notice_content 必须是 text"
        );
    }

    /// `ensure_unique` 按 user_name / phone_number / email 三列查存在性,
    /// 三列都必须有索引 —— 缺一个就是 20 万行的全表扫描。
    ///
    /// 这条曾经不成立:两份 DDL 都只索引了前两列(上游 RuoYi 的脚本同样漏了
    /// email),于是 `PUT /system/user` 每次扫全表。压测实测:MySQL 上该查询
    /// 232.67 ms、占整段数据库时间的 96.6%,补上索引后降到 1.0 ms。
    #[test]
    fn unique_check_columns_are_indexed_in_both_dialects() {
        const MY_INIT: &str =
            include_str!("../../../../../sql/solidaxum_init.sql");
        for col in ["user_name", "phone_number", "email"] {
            assert!(
                MY_INIT.contains(&format!("`idx_sys_user_{}`", idx_name(col))),
                "MySQL 建表脚本缺 sys_user({col}) 索引"
            );
            assert!(
                PG_INIT.contains(&format!(
                    "idx_sys_user_{} ON sys_user ({col})",
                    idx_name(col)
                )),
                "PG 建表脚本缺 sys_user({col}) 索引"
            );
        }
    }

    /// 索引名与列名不是一一对应(phone_number 的索引叫 idx_sys_user_phone)。
    fn idx_name(col: &str) -> &str {
        match col {
            "phone_number" => "phone",
            other => other,
        }
    }

    /// 同一处直译错误会波及每一个 longblob 列。本 schema 没有真正的二进制列,
    /// 冒出 BYTEA 就说明又有一列被直译了。
    #[test]
    fn pg_schema_has_no_bytea_columns() {
        assert!(
            !PG_INIT.to_ascii_uppercase().contains("BYTEA"),
            "PG 建表脚本不该出现 BYTEA —— 本 schema 没有二进制列"
        );
    }
}
