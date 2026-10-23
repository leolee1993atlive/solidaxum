use app_core::{
    error::{AppError, AppResult},
    model::{
        entity::{BaseEntity, BaseVo},
        status::{DelFlag, Status},
    },
    util::snowflake,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Post {
    pub post_id: Option<i64>,
    pub dept_id: i64,
    pub post_code: String,
    pub post_category: Option<String>,
    pub post_name: String,
    pub post_sort: i32,
    pub status: String,
    /// 删除标志（0代表存在 1代表删除）。对齐 Java SysPost @TableLogic delFlag。
    /// 不出现在 VO 中，对齐 Java 不序列化逻辑删除字段。
    pub del_flag: String,
    pub remark: Option<String>,
    #[serde(flatten)]
    pub base_entity: BaseEntity,
}

impl Post {
    /// 审计五列不手工盖章 —— 漏斗 insert 按 ctx 的 principal 统一
    /// 盖 create_*/update_*,这里只组装业务字段。
    pub fn from_create_bo(bo: PostCreateBo) -> AppResult<Self> {
        let dept_id = bo.dept_id.ok_or_else(|| {
            AppError::BadRequest("部门ID不能为空".to_string())
        })?;
        Ok(Self {
            post_id: Some(snowflake::next_id()),
            dept_id,
            post_code: bo.post_code,
            post_category: bo.post_category,
            post_name: bo.post_name,
            post_sort: bo.post_sort.unwrap_or(0),
            status: bo
                .status
                .unwrap_or_else(|| Status::Normal.as_str().to_string()),
            del_flag: DelFlag::Alive.as_str().to_string(),
            remark: bo.remark,
            base_entity: BaseEntity::default(),
        })
    }

    /// update_*/审计不手工盖章(漏斗 update 统一盖),只合并业务字段;
    /// create_* 保留加载时的原值随行回写。
    pub fn from_update_bo(&mut self, bo: PostUpdateBo) {
        self.dept_id = bo.dept_id.unwrap_or(self.dept_id);
        self.post_code = bo.post_code;
        self.post_category = bo.post_category;
        self.post_name = bo.post_name;
        self.post_sort = bo.post_sort.unwrap_or(self.post_sort);
        self.status = bo.status.unwrap_or_else(|| self.status.clone());
        self.remark = bo.remark;
    }

    pub fn to_list_vo(&self) -> PostListVo {
        PostListVo {
            post_id: self.post_id.unwrap_or_default(),
            dept_id: self.dept_id,
            post_code: self.post_code.clone(),
            post_category: self.post_category.clone(),
            post_name: self.post_name.clone(),
            post_sort: self.post_sort,
            status: self.status.clone(),
            remark: self.remark.clone(),
            dept_name: None,
            base_vo: BaseVo::from_base(&self.base_entity),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PostCreateBo {
    #[validate(required)]
    pub dept_id: Option<i64>,
    #[validate(length(min = 1, max = 64))]
    pub post_code: String,
    pub post_category: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub post_name: String,
    pub post_sort: Option<i32>,
    pub status: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateBo {
    #[validate(required)]
    pub post_id: Option<i64>,
    #[validate(required)]
    pub dept_id: Option<i64>,
    #[validate(length(min = 1, max = 64))]
    pub post_code: String,
    pub post_category: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub post_name: String,
    pub post_sort: Option<i32>,
    pub status: Option<String>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostListVo {
    pub post_id: i64,
    pub dept_id: i64,
    pub post_code: String,
    pub post_category: Option<String>,
    pub post_name: String,
    pub post_sort: i32,
    pub status: String,
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dept_name: Option<String>,
    #[serde(flatten)]
    pub base_vo: BaseVo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostPageQuery {
    #[serde(flatten)]
    pub page: app_core::vo::page::PageQuery,
    pub post_code: Option<String>,
    pub post_name: Option<String>,
    pub post_category: Option<String>,
    /// 按部门子树搜索(belongDeptId)。service 层展开子树为业务
    /// Condition 注入列表/导出过滤 —— 与漏斗行级数据范围正交(叠加)。
    pub belong_dept_id: Option<i64>,
    pub dept_id: Option<i64>,
    pub status: Option<String>,
}

/// 岗位导出行(对齐 Vue 岗位表格可见列)。
#[derive(Clone, Debug)]
pub struct PostExportRow {
    pub post_id: i64,
    pub post_code: String,
    pub post_name: String,
    pub post_sort: i32,
    pub status: String,
    pub create_time: String,
}

impl PostExportRow {
    pub fn from_post(post: &Post) -> Self {
        Self {
            post_id: post.post_id.unwrap_or_default(),
            post_code: post.post_code.clone(),
            post_name: post.post_name.clone(),
            post_sort: post.post_sort,
            status: post.status.clone(),
            create_time: post
                .base_entity
                .create_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        }
    }
}

impl app_core::util::excel::ExcelRow for PostExportRow {
    fn headers() -> Vec<&'static str> {
        vec![
            "岗位编号",
            "岗位编码",
            "岗位名称",
            "排序",
            "状态",
            "创建时间",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.post_id.to_string(),
            self.post_code.clone(),
            self.post_name.clone(),
            self.post_sort.to_string(),
            self.status.clone(),
            self.create_time.clone(),
        ]
    }

    fn from_record(_: &[String]) -> AppResult<Self> {
        Err(AppError::BadRequest("岗位不支持导入".to_string()))
    }
}
