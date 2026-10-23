//! 请假单服务 —— leave 链经 `ctx.transaction` 单闭包执行。
//!
//! # 事务形状:守卫提升事务外,写半边单闭包
//!
//! 同 definition/instance/task/category:**守卫(纯读/前置校验)提升事务外,
//! 写半边单闭包** —— edit 的存在性 NotFound 在闭包外以原文案抛出,闭包内
//! 只剩 insert/update 的 DataError;add 无守卫,单闭包单写。
//!
//! # 0 行语义
//!
//! `update_by_id` / `update_by_id_in_txn`:漏斗 update 的 WHERE = 主键 +
//! 软删存活(test_leave 无软删列,即纯主键),0 行 → `DataError::NotFound`
//! (404;触发点是「读后被并发删除」的竞争窗口)。
//! `delete_by_ids` 硬删 0 行 → Ok(0),返回值被忽略。

use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    util::snowflake,
};

use crate::{
    dto::flow_params::FlowParams,
    handler::to_ctx,
    model::dto::{FlowInstanceBizExtBo, LeaveAddBo, LeaveEditBo},
    repository::{entity::test_leave, leave_repo},
    service::instance_service::InstanceService,
};

/// 执行面全部经请求期 `DataContext`,不再持有数据库连接。
#[derive(Clone)]
pub struct LeaveService {
    ins_service: InstanceService,
}

impl LeaveService {
    pub fn new(ins_service: InstanceService) -> Self {
        Self { ins_service }
    }

    fn now() -> chrono::NaiveDateTime {
        chrono::Local::now().naive_local()
    }

    /// (ctx 参数是漏斗约定的执行面入口,计入后触发 clippy 参数上限,
    /// 显式豁免。)
    #[allow(clippy::too_many_arguments)]
    pub async fn list_page(
        &self,
        ctx: &app_data::DataContext<'_>,
        leave_type: Option<&str>,
        status: Option<&str>,
        start_leave_days: Option<i32>,
        end_leave_days: Option<i32>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<test_leave::Model>> {
        Ok(leave_repo::select_page(
            ctx,
            leave_type,
            status,
            start_leave_days,
            end_leave_days,
            page_no,
            page_size,
        )
        .await?)
    }

    /// 导出用全量查询(不分页,见 `leave_repo::select_list` 的说明)。
    /// 调用方须先经 [`Self::count_for_export`] + `guard_export_rows` 把住行数。
    pub async fn list_all_for_export(
        &self,
        ctx: &app_data::DataContext<'_>,
        leave_type: Option<&str>,
        status: Option<&str>,
        start_leave_days: Option<i32>,
        end_leave_days: Option<i32>,
    ) -> AppResult<Vec<test_leave::Model>> {
        Ok(leave_repo::select_list(
            ctx,
            leave_type,
            status,
            start_leave_days,
            end_leave_days,
        )
        .await?)
    }

    /// 导出闸门用的精确行数(见 `leave_repo::count_by_filters` 的说明)。
    pub async fn count_for_export(
        &self,
        ctx: &app_data::DataContext<'_>,
        leave_type: Option<&str>,
        status: Option<&str>,
        start_leave_days: Option<i32>,
        end_leave_days: Option<i32>,
    ) -> AppResult<u64> {
        Ok(leave_repo::count_by_filters(
            ctx,
            leave_type,
            status,
            start_leave_days,
            end_leave_days,
        )
        .await?)
    }

    pub async fn get_by_id(
        &self,
        ctx: &app_data::DataContext<'_>,
        id: i64,
    ) -> AppResult<Option<test_leave::Model>> {
        Ok(leave_repo::select_by_id(ctx, id).await?)
    }

    pub async fn add(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: LeaveAddBo,
        user: &CurrentUser,
    ) -> AppResult<test_leave::Model> {
        let now = Self::now();
        let end = now + chrono::Duration::days(i64::from(bo.leave_days));
        let user_id = user.get_user_id();
        let model = test_leave::Model {
            id: snowflake::next_id(),
            leave_type: bo.leave_type,
            remark: bo.remark,
            status: Some(bo.status.unwrap_or_else(|| "draft".to_string())),
            apply_code: Some(
                chrono::Local::now().timestamp_millis().to_string(),
            ),
            start_date: Some(now),
            end_date: Some(end),
            leave_days: Some(bo.leave_days),
            create_dept: None,
            create_time: Some(now),
            create_by: Some(user_id),
            update_time: Some(now),
            update_by: Some(user_id),
        };
        // 写半边单闭包
        ctx.transaction(|tx| {
            let model = model.clone();
            Box::pin(async move {
                leave_repo::insert_in_txn(tx, &model).await?;
                Ok(())
            })
        })
        .await?;
        Ok(model)
    }

    pub async fn edit(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: LeaveEditBo,
        user: &CurrentUser,
    ) -> AppResult<test_leave::Model> {
        // 守卫提升(读自身 + NotFound 文案)
        let mut model =
            leave_repo::select_by_id(ctx, bo.id).await?.ok_or_else(|| {
                AppError::NotFound(format!("请假记录不存在 (id={})", bo.id))
            })?;

        model.leave_type = bo.leave_type;
        model.leave_days = Some(bo.leave_days);
        model.remark = bo.remark;
        model.status = bo.status.or(model.status);
        model.update_time = Some(Self::now());
        model.update_by = Some(user.get_user_id());

        // 写半边单闭包
        ctx.transaction(|tx| {
            let model = model.clone();
            Box::pin(async move {
                leave_repo::update_by_id_in_txn(tx, &model).await?;
                Ok(())
            })
        })
        .await?;
        Ok(model)
    }

    pub async fn delete(
        &self,
        ctx: &app_data::DataContext<'_>,
        ids: &[i64],
        _user: &CurrentUser,
    ) -> AppResult<()> {
        leave_repo::delete_by_ids(ctx, ids).await?;
        Ok(())
    }

    pub async fn submit_and_start(
        &self,
        dctx: &app_data::DataContext<'_>,
        bo: LeaveAddBo,
        user: &CurrentUser,
    ) -> AppResult<test_leave::Model> {
        // 对齐 Java TestLeaveServiceImpl#submitAndFlowStart:
        // 前端流程定义下拉选值通过 flowCode 传入,为空时回退到 "leave1"。
        // 在 bo 被 add() 消费前取出,避免 use-after-move。
        let flow_code = bo
            .flow_code
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "leave1".to_string());
        // 🔴 带 id 就地更新、不带才新建 —— 对齐 Java 的 `leaveMapper.insertOrUpdate(leave)`
        // (TestLeaveServiceImpl:157)。此前一律走 add():从「修改」页点提交会新建一条
        // 单据并在**新 id** 上发起流程,原草稿原地不动仍是 draft,列表凭空多出一行。
        let mut model = match bo.id {
            Some(id) => {
                let mut m = self
                    .edit(
                        dctx,
                        LeaveEditBo {
                            id,
                            leave_type: bo.leave_type,
                            leave_days: bo.leave_days,
                            remark: bo.remark,
                            // 状态在下面统一置 waiting,这里不动既有值。
                            status: None,
                        },
                        user,
                    )
                    .await?;
                // 起止日期按「提交时刻 + 天数」重算,与 add() 同一规则。
                // edit() 刻意不碰日期(LEV10 的有意分歧,那是纯编辑语义),
                // 但这里是重新发起:不重算就会留下 leave_days=5 而 end-start=1 天
                // 的自相矛盾记录,列表两列直接对不上。
                let now = Self::now();
                m.start_date = Some(now);
                m.end_date = Some(
                    now + chrono::Duration::days(i64::from(bo.leave_days)),
                );
                m
            }
            None => self.add(dctx, bo, user).await?,
        };
        model.status = Some("waiting".to_string());
        leave_repo::update_by_id(dctx, &model).await?;

        let business_id = model.id.to_string();
        let ctx = to_ctx(user);
        // 首节点 `${initiator}` / `initiatorParent` / 条件表达式都依赖服务端
        // 注入的发起人变量;instance_service::start_work_flow 会注入,但本方法
        // 直调 start() 绕过——不补则首节点办理人解析为空、待办无人可见。
        let params = FlowParams {
            flow_code: Some(flow_code),
            variable: crate::service::instance_service::build_start_variables(
                build_start_variables(model.leave_days),
                user.0.user_id,
                user.0.dept_id,
                &business_id,
            ),
            ..Default::default()
        };
        let instance = self
            .ins_service
            .start(dctx, &business_id, &params, &ctx)
            .await?;
        self.ins_service
            .save_biz_ext(
                dctx,
                instance.id.unwrap_or(0),
                &business_id,
                &FlowInstanceBizExtBo {
                    business_title: Some(format!(
                        "{}请假{}天",
                        model.leave_type,
                        model.leave_days.unwrap_or(0)
                    )),
                    ..Default::default()
                },
                &ctx,
            )
            .await?;

        Ok(model)
    }
}

/// 发起流程时注入的流程变量。
///
/// Vue 侧由前端在 `startWorkFlow` 时传 `taskVariables`(leaveEdit.vue:245-262),
/// 本仓走后端发起(submitAndFlowStart),这层变量在移植时被漏掉了 ——
/// 于是 leave2 的排他网关条件 `le@@leaveDays|2` 找不到变量,
/// `eval_condition` 抛「条件变量不存在」并向上传播 → 提交必 500。
///
/// ⚠️ 只补 `leaveDays`。leave4/leave5 依赖的 `userList` 是 demo 数据问题
/// (Vue 硬编码 ['1','3','4'],本库无此用户),不在这里编造。
fn build_start_variables(
    leave_days: Option<i32>,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut variable = std::collections::HashMap::new();
    variable.insert(
        "leaveDays".to_string(),
        serde_json::json!(leave_days.unwrap_or(0)),
    );
    variable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::condition::eval_condition;

    #[test]
    fn start_variables_contain_leave_days() {
        let v = build_start_variables(Some(3));
        assert_eq!(v.get("leaveDays"), Some(&serde_json::json!(3)));
    }

    /// leave2 的网关条件实测为 `le@@leaveDays|2` / `gt@@leaveDays|2`
    /// (flow_skip.skip_condition,连真实库查得)。eval_condition 在变量缺失时
    /// 抛「条件变量不存在」并向上传播 → 500。这条测试钉住「必须注入」。
    #[test]
    fn leave2_gateway_condition_evaluates_with_injected_variable() {
        let v = build_start_variables(Some(5));
        assert!(eval_condition("gt@@leaveDays|2", &v).unwrap());
        assert!(!eval_condition("le@@leaveDays|2", &v).unwrap());
        // 反向:不注入就是 Err,不是 false —— 这正是交付版 500 的成因
        assert!(
            eval_condition(
                "gt@@leaveDays|2",
                &std::collections::HashMap::new()
            )
            .is_err()
        );
    }

    #[test]
    fn missing_leave_days_falls_back_to_zero() {
        let v = build_start_variables(None);
        assert_eq!(v.get("leaveDays"), Some(&serde_json::json!(0)));
    }

    /// 发起人变量注入是 submit_and_start 的硬约定:首节点 `${initiator}`
    /// 解析、initiatorParent / preStepParent 回退全靠它。走查实测漏注会导致
    /// 首节点办理人为空(flow_user 0 行)、待办列表查不到任务。这里钉住
    /// 「leave 变量必须经 instance_service::build_start_variables 合并」。
    #[test]
    fn submit_variables_merge_initiator() {
        let v = crate::service::instance_service::build_start_variables(
            build_start_variables(Some(1)),
            1761100000000000001,
            Some(1761000000000000100),
            "BIZ-9",
        );
        assert_eq!(
            v.get("initiator"),
            Some(&serde_json::json!("1761100000000000001"))
        );
        assert_eq!(
            v.get("initiatorDeptId"),
            Some(&serde_json::json!(1761000000000000100i64))
        );
        assert_eq!(v.get("businessId"), Some(&serde_json::json!("BIZ-9")));
        assert_eq!(v.get("leaveDays"), Some(&serde_json::json!(1)));
    }
}
