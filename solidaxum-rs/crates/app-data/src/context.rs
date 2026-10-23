//! 漏斗本体:DataSource(池)→ DataContext(连接)→ TxnContext(事务)。
//! 两个公开 Context 类型都刻意不 impl ConnectionTrait —— 否则 `.all(ctx)`
//! 一句绕过上缝(设计 6.3);擦除 Conn/Txn 分支的活由 crate 私有的
//! [`ContextCore`] 承担(见其 impl 处的说明)。
use std::sync::Arc;

use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, Condition, ConnectionTrait,
    EntityTrait, IntoActiveModel,
};
use tokio::sync::OnceCell;

use crate::{
    count::CountMode,
    error::DataError,
    page::{Page, Paging},
    policy::DataPolicy,
    principal::{Principal, ScopeFilter, ScopeResolver},
};

pub struct DataSource {
    conn: sea_orm::DatabaseConnection,
    resolver: Option<Arc<dyn ScopeResolver>>,
    /// 该数据源上未显式指定口径时的分页计数方式。
    count_mode: CountMode,
}

impl DataSource {
    pub async fn connect(
        opt: sea_orm::ConnectOptions,
    ) -> Result<Self, DataError> {
        Ok(Self {
            conn: sea_orm::Database::connect(opt).await?,
            resolver: None,
            count_mode: CountMode::default(),
        })
    }

    /// 装配用:直接包已有连接(core::infra / admin 装配层构造 DataSource 的正道)。
    /// 构造它必须先**持有** `DatabaseConnection`(连接归装配层/admin 所有,
    /// 业务 crate 命名不出该类型),不构成执行能力泄漏。
    pub fn from_connection(conn: sea_orm::DatabaseConnection) -> Self {
        Self {
            conn,
            resolver: None,
            count_mode: CountMode::default(),
        }
    }

    pub fn with_resolver(mut self, r: Arc<dyn ScopeResolver>) -> Self {
        self.resolver = Some(r);
        self
    }

    /// 换掉本数据源的默认计数口径。
    ///
    /// 默认是上限计数(1 万)。改成 [`CountMode::Exact`] 会让**所有**未显式
    /// 指定口径的分页都走全量精确计数,代价与结果集大小成正比 —— 只有确实
    /// 需要全局精确总数的部署才该这么配(典型是全表都在万级以下,上限
    /// 永远触不到,配 exact 只是把这件事写明)。
    pub fn with_count_mode(mut self, mode: CountMode) -> Self {
        self.count_mode = mode;
        self
    }

    /// 装配层出口(from_connection 家族):克隆内部连接句柄
    /// (`DatabaseConnection` 是池句柄,克隆廉价、同池同库)换装 resolver,
    /// 返回**只读派生**的新 DataSource —— 母体不受染。
    ///
    /// 破解的是装配鸡蛋环:`Infrastructure::new` → ServiceContainer(resolver
    /// 依赖 PermissionService)→ 带 resolver 的主 ds → `set_data_source`。
    /// core 去连接化后装配层只有一个 ds,母体留作 resolver 自有的 unscoped
    /// 通道(scope_resolver.rs 递归防御的第一参),派生体带 resolver 投给
    /// `Infrastructure::set_data_source`。**内部可变性 setter 方案被否决**:
    /// 它会让母体(那条 unscoped 通道)也拿到 resolver,解析经 system_context
    /// 不会递归、但经任何带 principal 的读就会自我触发 —— 递归防御被破坏,
    /// 故只取只读派生。
    pub fn derive_with_resolver(
        &self,
        r: Arc<dyn ScopeResolver>,
    ) -> DataSource {
        DataSource {
            conn: self.conn.clone(),
            resolver: Some(r),
            count_mode: self.count_mode,
        }
    }

    /// 行级范围 + 软删除 + 审计(审计人 = principal.user_id)
    pub fn context(&self, principal: Principal) -> DataContext<'_> {
        DataContext {
            core: ContextCore::new(
                Exec::Conn(&self.conn),
                Some(principal),
                self.resolver.clone(),
                self.count_mode,
            ),
        }
    }

    /// 跳过行级范围,保留软删除 + 审计(审计人 = 系统)。名字扎眼、可 grep(设计 7.3)。
    pub fn system_context(&self) -> DataContext<'_> {
        DataContext {
            core: ContextCore::new(
                Exec::Conn(&self.conn),
                None,
                self.resolver.clone(),
                self.count_mode,
            ),
        }
    }

    /// 仅测试:按实体建表。`Schema` 是 app-data 的内部细节,不经受限
    /// re-export,业务侧永远拿不到这个名字;`DbBackend` 仅作为
    /// `into_sql` 的方言参数随 test-util 受限转出(见 sea_orm.rs)。
    #[cfg(any(test, feature = "test-util"))]
    pub async fn create_table<E>(&self, entity: E) -> Result<(), DataError>
    where
        E: EntityTrait,
    {
        let backend = self.conn.get_database_backend();
        let stmt =
            sea_orm::Schema::new(backend).create_table_from_entity(entity);
        // fork 的 ConnectionTrait::execute 直接收 &StatementBuilder,方言由连接自判
        self.conn.execute(&stmt).await?;
        Ok(())
    }

    /// 仅测试脚手架(crate 私有,test-util 同门):[`crate::testkit`] 的
    /// `execute_raw` 在 DataSource 上的执行出口 —— DDL/种子字符串 SQL 经
    /// 内部连接执行。与 [`Self::create_table`] 同门:crate 外命名不出
    /// 本方法,不构成绕过漏斗的公共旁路。
    #[cfg(any(test, feature = "test-util"))]
    pub(crate) async fn execute_raw_sql(
        &self,
        sql: &str,
    ) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
        self.conn.execute_unprepared(sql).await
    }
}

/// 请求级句柄(持连接)。**只有一个字段**,所有逻辑在 ContextCore 上写一份。
pub struct DataContext<'a> {
    core: ContextCore<'a>,
}

/// 事务级句柄(持事务)。与 DataContext 同构,**但没有 transaction() 方法**。
pub struct TxnContext<'a> {
    core: ContextCore<'a>,
}

// ---- 两个 Context 的共享内部逻辑;pub(crate) 是因为 select_builder.rs 要借它 ----
pub(crate) struct ContextCore<'a> {
    exec: Exec<'a>,
    principal: Option<Principal>,
    resolver: Option<Arc<dyn ScopeResolver>>,
    scope: OnceCell<ScopeFilter>,
    count_mode: CountMode,
}

pub(crate) enum Exec<'a> {
    Conn(&'a sea_orm::DatabaseConnection),
    Txn(&'a sea_orm::DatabaseTransaction),
}

impl<'a> ContextCore<'a> {
    fn new(
        exec: Exec<'a>,
        principal: Option<Principal>,
        resolver: Option<Arc<dyn ScopeResolver>>,
        count_mode: CountMode,
    ) -> Self {
        Self {
            exec,
            principal,
            resolver,
            scope: OnceCell::new(),
            count_mode,
        }
    }
}

/// 连接/事务的统一执行视图。
///
/// 设计原稿这里是 `exec() -> &dyn sea_orm::ConnectionTrait`,但 workspace 用的
/// vendored fork 给 `ConnectionTrait` 加了泛型方法(`execute<S: StatementBuilder>`
/// / `query_*<S>`),trait 因此**不可对象安全**,`&dyn` 形态编不过。改为在
/// crate 私有的 `ContextCore` 上直接 impl —— 两个公开 Context 类型仍然不 impl
/// (那是上缝,漏斗强制性的全部意义),业务侧既拿不到 `ContextCore`(私有字段、
/// 无构造器),也命名不出 `ConnectionTrait`(受限 re-export 剔除),不构成旁路。
#[async_trait::async_trait]
impl ConnectionTrait for ContextCore<'_> {
    fn get_database_backend(&self) -> sea_orm::DbBackend {
        match &self.exec {
            Exec::Conn(c) => c.get_database_backend(),
            Exec::Txn(t) => t.get_database_backend(),
        }
    }

    async fn execute_raw(
        &self,
        stmt: sea_orm::Statement,
    ) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
        match &self.exec {
            Exec::Conn(c) => c.execute_raw(stmt).await,
            Exec::Txn(t) => t.execute_raw(stmt).await,
        }
    }

    async fn execute_unprepared(
        &self,
        sql: &str,
    ) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
        match &self.exec {
            Exec::Conn(c) => c.execute_unprepared(sql).await,
            Exec::Txn(t) => t.execute_unprepared(sql).await,
        }
    }

    async fn query_one_raw(
        &self,
        stmt: sea_orm::Statement,
    ) -> Result<Option<sea_orm::QueryResult>, sea_orm::DbErr> {
        match &self.exec {
            Exec::Conn(c) => c.query_one_raw(stmt).await,
            Exec::Txn(t) => t.query_one_raw(stmt).await,
        }
    }

    async fn query_all_raw(
        &self,
        stmt: sea_orm::Statement,
    ) -> Result<Vec<sea_orm::QueryResult>, sea_orm::DbErr> {
        match &self.exec {
            Exec::Conn(c) => c.query_all_raw(stmt).await,
            Exec::Txn(t) => t.query_all_raw(stmt).await,
        }
    }
}

impl ContextCore<'_> {
    /// 开事务要的是连接本身。只有 DataContext 会调它,而 DataContext 永远持
    /// `Exec::Conn` —— Txn 分支不可达,但不写 unreachable!(),留 Unsupported
    /// 与本 crate 其余 fail-closed 保持同一处置。
    pub(crate) fn conn(
        &self,
    ) -> Result<&sea_orm::DatabaseConnection, DataError> {
        match &self.exec {
            Exec::Conn(c) => Ok(*c),
            Exec::Txn(_) => Err(DataError::Unsupported(
                "事务上下文不能再开事务(嵌套事务应是编译错误,走到这里说明结构被改坏了)".into(),
            )),
        }
    }

    /// ScopeFilter 惰性解析 + 请求内缓存(设计 7.2)。
    /// system_context(principal = None)→ All;无 resolver → Unsupported(fail-closed)。
    pub(crate) async fn scope_filter(&self) -> Result<&ScopeFilter, DataError> {
        self.scope
            .get_or_try_init(|| async {
                match (&self.principal, &self.resolver) {
                    (Some(p), Some(r)) => r.resolve(p).await,
                    (Some(_), None) => Err(DataError::Unsupported(
                        "DataSource 未配置 ScopeResolver,拒绝隐式放行".into(),
                    )),
                    (None, _) => Ok(ScopeFilter::All),
                }
            })
            .await
    }

    /// 审计人:principal.user_id,system_context 下为 0(系统)。
    pub(crate) fn principal_id(&self) -> i64 {
        self.principal.as_ref().map(|p| p.user_id).unwrap_or(0) // 0 = 系统
    }

    /// 审计的 create_dept 取的是部门,不是人。system_context 下为 None → 不盖。
    pub(crate) fn principal_dept_id(&self) -> Option<i64> {
        self.principal.as_ref().and_then(|p| p.dept_id)
    }

    /// 本上下文未显式指定口径时的分页计数方式,来自数据源。
    pub(crate) fn count_mode(&self) -> CountMode {
        self.count_mode
    }

    // 主键 + 软删除 + 行级范围(CONSTRAINED 时)。system_context 的
    // ScopeFilter 恒为 All → 只剩软删除;越权/已删一律 NotFound,不泄露存在性。
    pub(crate) async fn find_by_id<E>(
        &self,
        id: i64,
    ) -> Result<E::Model, DataError>
    where
        E: DataPolicy,
        // 单列 i64 主键(38 实体全部如此,多主键等真实出现再扩)。
        // 这是**约束**不是注释 —— 少了它 E::find_by_id(id) 过不了类型检查。
        E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = i64>,
    {
        use sea_orm::{ColumnTrait, QueryFilter};

        let mut sel = E::find_by_id(id);
        if let Some((col, alive)) = E::soft_delete() {
            sel = sel.filter(col.eq(alive));
        }
        if E::CONSTRAINED {
            let sf = self.scope_filter().await?.clone();
            let (d, o) = (E::dept_column(), E::owner_column());
            if let Some(c) = crate::principal::scope_condition(d, o, &sf)? {
                sel = sel.filter(c);
            }
        }
        sel.one(self).await?.ok_or(DataError::NotFound)
    }

    // 分页。条件合成与 find_by_id 完全同构:软删除声明了就无条件过滤,
    // 行级范围仅在 CONSTRAINED 时施加(ScopeFilter 只在需要时解析,不引入
    // "不需要也解析"的缝)。rows 走 fetch_page,页序从 0 起。
    //
    // total 的口径由 `mode` 决定,默认**上限计数**而非全量 COUNT ——
    // 依据与实测见 crate::count 模块头。
    pub(crate) async fn page<E>(
        &self,
        page_num: i64,
        page_size: i64,
        cond: Condition,
        mode: CountMode,
    ) -> Result<Page<E::Model>, DataError>
    where
        E: DataPolicy,
        // 这是**约束**不是注释:fork 的 PaginatorTrait 给 Select<E> 的实现要求
        // Model Send + Sync(selector 跨 await 点)。find_by_id 走的是 Select 固有
        // 方法(`.one(&impl ConnectionTrait)`),没这道坎;派生宏生成的 Model 全满足。
        E::Model: Send + Sync,
    {
        use sea_orm::{ColumnTrait, PaginatorTrait, QueryFilter, QueryTrait};

        let paging = Paging::new(page_num, page_size);
        // 用户是否加了过滤条件,**只有这里知道** —— 往下走一步条件就被合进
        // Select 再也拆不出来了。目录估算不认 WHERE,所以这个事实要在此刻取下。
        let user_filtered = !cond.is_empty();
        let mut sel = E::find().filter(cond);
        if let Some((col, alive)) = E::soft_delete() {
            sel = sel.filter(col.eq(alive));
        }
        // 行级范围会把结果集砍成任意小的一块,一旦施加,表级估算就完全不可比。
        let mut scoped = false;
        if E::CONSTRAINED {
            let sf = self.scope_filter().await?.clone();
            let (d, o) = (E::dept_column(), E::owner_column());
            if let Some(c) = crate::principal::scope_condition(d, o, &sf)? {
                sel = sel.filter(c);
                scoped = true;
            }
        }

        let (total, total_capped) =
            crate::count::count_total(self, sel.clone().into_query(), mode)
                .await?;
        let paginator = sel.paginate(self, paging.limit());
        let rows = paginator.fetch_page(paging.page() - 1).await?;

        // 触顶了才去问目录,且只在"除软删除外没有别的条件"时问 ——
        // 有用户条件或行级范围时,整表估算与本次查询无关,给了就是误导。
        let total_approx = if total_capped && !user_filtered && !scoped {
            crate::count::approx_row_count(self, E::default().table_name())
                .await?
        } else {
            None
        };

        Ok(Page {
            rows,
            total,
            total_capped,
            total_approx,
        })
    }

    // 审计盖章(RuoYi 约定插入即全盖)+ 唯一键冲突 → Conflict。
    // Send 是 fork 的 ActiveModelTrait::insert(async_trait 脱糖)要求的;
    // ActiveModelBehavior / IntoActiveModel 是 insert 自身的 where 子句。
    // EntityTrait<ActiveModel = A> 把泛型 A 钉回实体的 ActiveModel ——
    // 盖章闭包(AuditColumns<E::ActiveModel>)只认那个类型。
    pub(crate) async fn insert<A>(
        &self,
        mut am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.stamp_insert_audit(&mut am);
        am.insert(self).await.map_err(DataError::classify_unique)
    }

    /// 插入盖章的共享半边(insert / insert_returning_nothing / insert_many
    /// 三个入口同源,杜绝「批量版悄悄少盖审计」的漂移)。
    fn stamp_insert_audit<A>(&self, am: &mut A)
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        let now = chrono::Utc::now();
        let by = self.principal_id();
        // RuoYi 约定插入时审计列全盖(与 Java BaseEntity 的填充一致),不做 NotSet 判定
        if let Some(a) = <A::Entity as DataPolicy>::audit_columns() {
            // create_dept 盖的是部门,不是人;system_context 无部门 → 不盖
            if let (Some(f), Some(dept)) =
                (a.create_dept, self.principal_dept_id())
            {
                f(am, dept);
            }
            if let Some(f) = a.create_by {
                f(am, by);
            }
            if let Some(f) = a.create_time {
                f(am, now);
            }
            if let Some(f) = a.update_by {
                f(am, by);
            }
            if let Some(f) = a.update_time {
                f(am, now);
            }
        }
    }

    /// 不回读的单条插入。`ActiveModelTrait::insert` 在不支持 RETURNING 的
    /// 后端(MySQL / 达梦 / SQLite —— 即本项目全部部署形态)会在 INSERT
    /// 之后再发一条 `find_by_id` 把行读回来
    /// (vendor/sea-orm/src/executor/insert.rs:526)。调用方不要那个 Model
    /// 时,那次 SELECT 是纯浪费 —— 本方法走 `exec_without_returning`,
    /// 只发一条语句。盖章与冲突分类与 `insert` 同口径。
    ///
    /// **与 `insert` 的唯一语义差**:绕过 `ActiveModelTrait::insert`
    /// (active_model.rs:225-236)⇒ **不走 `ActiveModelBehavior` 的
    /// `before_save` / `after_save` 钩子**。当前仓内 42 个实体全是
    /// `impl ActiveModelBehavior for ActiveModel {}` 默认空实现,两条路径
    /// 行为一致;**若将来有实体实现这两个钩子,本方法与 `insert_many`
    /// 会静默跳过** —— 那时要么在此手工调一次 before_save,要么把该实体
    /// 的写路径钉回 `insert`。
    pub(crate) async fn insert_returning_nothing<A>(
        &self,
        mut am: A,
    ) -> Result<(), DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        self.stamp_insert_audit(&mut am);
        <A::Entity as EntityTrait>::insert(am)
            .exec_without_returning(self)
            .await
            .map_err(DataError::classify_unique)?;
        Ok(())
    }

    /// 批量插入:单条多值 INSERT,不回读。空集合 no-op 返回 0。
    /// 盖章逐行施加(与 `insert` 同口径);唯一键冲突分类同上。
    /// **不走 `ActiveModelBehavior` 钩子**,理由与取舍见
    /// [`Self::insert_returning_nothing`] 的说明。
    /// 整批原子:任一行冲突则整条语句失败,已盖章的其余行也不落库
    /// (逐行 `insert` 时前几行会先落 —— 事务内无差别,连接路径下
    /// 本方法更严)。
    pub(crate) async fn insert_many<A>(
        &self,
        ams: Vec<A>,
    ) -> Result<u64, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        if ams.is_empty() {
            return Ok(0);
        }
        let stamped: Vec<A> = ams
            .into_iter()
            .map(|mut am| {
                self.stamp_insert_audit(&mut am);
                am
            })
            .collect();
        <A::Entity as EntityTrait>::insert_many(stamped)
            .exec_without_returning(self)
            .await
            .map_err(DataError::classify_unique)
    }

    // 免盖章插入(test-util 面)—— 设计 9 节第 4 条"种子数据与断言
    // 全部经 DataContext/TxnContext/system_context()"要求的种子原语。
    // system 测试的范围交叉用例要种 create_by=7 这类显式审计值
    // (user_repo_test.rs:56、role_repo_test.rs:91-95),漏斗 insert 无条件
    // 盖章会把它顶掉。唯一豁免的是盖章:唯一键冲突分类与 insert 同口径;
    // 插入路径本来就不施软删/范围过滤,没有第二道要豁免的门。
    #[cfg(any(test, feature = "test-util"))]
    pub(crate) async fn insert_unstamped<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        am.insert(self).await.map_err(DataError::classify_unique)
    }

    // update。审计盖 update_*,行级范围进 WHERE(设计 7.4 二)。
    // 不走 ActiveModelTrait::update —— 那条路没有 filter 的落点,越权行会被
    // 静默改掉;这里的形状由 2.0.2 的类型定死(E::update → validate → filter → exec)。
    //
    // skip_row_scope 是写侧显式豁免开关(设计 7.4 修正):true 仅由公开入口
    // `update_without_row_scope` 传入,跳过行级范围**一项** —— 软删存活、审计
    // 盖章、空 SET 守卫、RecordNotUpdated→NotFound 全部照旧。两个公开入口
    // 共享同一具身(update_query),杜绝豁免版与受辖版漂移。
    pub(crate) async fn update<A>(
        &self,
        am: A,
        skip_row_scope: bool,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        // ValidatedUpdateOne::exec 要求的回写 bound(executor/update.rs:47)
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        // exec() 直接返回更新后的 Model(executor/update.rs:47)—— 不需要回读。
        // 无匹配行时 sea-orm 返回 DbErr::RecordNotUpdated,这就是"0 行"的判定钩子。
        self.update_query(am, skip_row_scope)
            .await?
            .exec(self)
            .await
            .map_err(|e| match e {
                sea_orm::DbErr::RecordNotUpdated => DataError::NotFound,
                other => DataError::classify_unique(other),
            })
    }

    /// [`ContextCore::update`](Self::update) 的构造半边:审计盖章 → 空 SET
    /// 守卫 → validate → 软删存活 →(可选)行级范围。拆成独立函数有两重
    /// 动机:① 两个公开入口(update / update_without_row_scope)共享同一
    /// 构造序列,不给"豁免版悄悄少盖审计"这类漂移留缝;② SQL 断言要
    /// **只 build 不执行**(设计 9.1,同 SelectQuery::into_sql 的口径)——
    /// crate 内测试模块是本模块的子模块,可直接调用到这个私有函数。
    async fn update_query<A>(
        &self,
        mut am: A,
        skip_row_scope: bool,
    ) -> Result<sea_orm::ValidatedUpdateOne<A>, DataError>
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        use sea_orm::{
            ActiveValue, ColumnTrait, Iterable, PrimaryKeyToColumn, QueryFilter,
        };

        let now = chrono::Utc::now();
        let by = self.principal_id();
        if let Some(a) = <A::Entity as DataPolicy>::audit_columns() {
            if let Some(f) = a.update_by {
                f(&mut am, by);
            }
            if let Some(f) = a.update_time {
                f(&mut am, now);
            }
        }

        // 空 SET 守卫(fail-closed):vendored 的 exec_update_and_return_updated
        // 在 SET 为空时跳过整条语句 —— 连同下面拼进 WHERE 的行级范围 —— 退化为
        // 无范围的 find_by_id 回读(executor/update.rs 的 is_noop 分支),越权/
        // 已删行会拿到 Ok(model)。判定与 `Update::one` 造 SET 的口径一致
        // (query/update.rs):主键列不计,只有 `ActiveValue::Set` 会进 SET。
        // 审计盖章在前:有审计列的实体盖章后 SET 非空,不受此守卫影响。
        // 豁免路径同样过此守卫 —— 豁免的是行级范围,不是 no-op 语义。
        let has_non_pk_set =
            <<A::Entity as EntityTrait>::Column as Iterable>::iter().any(
                |col| {
                    <A::Entity as EntityTrait>::PrimaryKey::from_column(col)
                        .is_none()
                        && matches!(am.get(col), ActiveValue::Set(_))
                },
            );
        if !has_non_pk_set {
            return Err(DataError::Unsupported(
                "空 SET 更新被拒绝:update 的 ActiveModel 除主键外无任何 Set 字段"
                    .into(),
            ));
        }

        // validate() 把 UpdateOne 转成 ValidatedUpdateOne —— 只有后者能 .filter()
        // (query/update.rs:42 / :148),而且它的 WHERE 里已经带上了主键条件。
        let mut q = <A::Entity as EntityTrait>::update(am).validate()?;

        // 软删除存活过滤(对齐 RuoYi @TableLogic 的写路径语义):已删行对本路径
        // 不可见,更新已删行 → 无匹配 → NotFound。恢复(del_flag 2→0)与物理清理
        // 已删行需要显式旁路,回收站类需求出现时再设计(设计 7.4)。
        if let Some((col, alive)) = <A::Entity as DataPolicy>::soft_delete() {
            q = q.filter(col.eq(alive));
        }

        // 行级范围进 WHERE(设计 7.4 二):与主键条件 AND,越权行匹配不到。
        // !skip_row_scope 短路在前 —— 豁免路径连 ScopeFilter 都不解析(与读侧
        // without_row_scope 同口径:不该白付一次 resolver 查库,也不该在
        // DataSource 未装 resolver 时被判成 Unsupported)。
        if !skip_row_scope && <A::Entity as DataPolicy>::CONSTRAINED {
            let sf = self.scope_filter().await?.clone();
            if let Some(c) = crate::principal::scope_condition(
                <A::Entity as DataPolicy>::dept_column(),
                <A::Entity as DataPolicy>::owner_column(),
                &sf,
            )? {
                q = q.filter(c);
            }
        }

        Ok(q)
    }

    // 物理删除(软删除是业务层的 update del_flag,不走这里)。
    // 行级范围同样进 WHERE,越权行 0 行受影响 → NotFound,绝不静默 Ok。
    pub(crate) async fn delete<E>(&self, id: i64) -> Result<(), DataError>
    where
        E: DataPolicy,
        E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = i64>,
    {
        use sea_orm::{ColumnTrait, QueryFilter};
        // delete_by_id 返回 ValidatedDeleteOne,它 impl 了 QueryFilter
        // (query/delete.rs:167),所以这里可以直接 filter —— 与 update 不同,
        // 不需要 validate()。
        let mut q = E::delete_by_id(id);
        // 软删除存活过滤,同 update:已删行的物理删除也拒绝(显式旁路另行设计)
        if let Some((col, alive)) = E::soft_delete() {
            q = q.filter(col.eq(alive));
        }
        if E::CONSTRAINED {
            let sf = self.scope_filter().await?.clone();
            if let Some(c) = crate::principal::scope_condition(
                E::dept_column(),
                E::owner_column(),
                &sf,
            )? {
                q = q.filter(c);
            }
        }
        let res = q.exec(self).await?;
        if res.rows_affected == 0 {
            return Err(DataError::NotFound);
        }
        Ok(())
    }

    // 按条件批量删除 —— 仅限豁免实体(CONSTRAINED=false 且无
    // 软删除列)。受辖实体的删除必须走 delete_by_id(范围进 WHERE),这里
    // fail-closed 拒绝,防止把行级范围绕成"条件里自己拼"。关联表(复合主键)
    // 的 delete-then-insert 模式是本原语的唯一动机。
    // 0 行返回 Ok(0) —— 批量语义无 NotFound 约定。
    pub(crate) async fn delete_where<E>(
        &self,
        cond: Condition,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        use sea_orm::QueryFilter;
        if E::CONSTRAINED {
            return Err(DataError::Unsupported(
                "delete_where 不接受受辖实体(CONSTRAINED=true):行级范围必须\
                 经 delete_by_id 的 WHERE 施加,不得由调用方在条件里自拼"
                    .into(),
            ));
        }
        if E::soft_delete().is_some() {
            return Err(DataError::Unsupported(
                "delete_where 不接受带软删除列的实体:批量物理删会绕过\
                 @TableLogic 的存活语义"
                    .into(),
            ));
        }
        let res = E::delete_many().filter(cond).exec(self).await?;
        Ok(res.rows_affected)
    }

    // 按条件批量更新 —— 单语句条件 UPDATE 的漏斗形态,恢复 CAS 平价:
    // 转办三条件单语句 update_many,先写者保(此前 select+逐行 update
    // 的后写者胜语义由此消除)。WHERE = cond AND 软删存活(声明了软删的
    // 实体)—— cond
    // 是**唯一**的调用方 WHERE 来源,ActiveModel 里的主键字段忽略:
    // 构造 update_many 前把主键列清成 NotSet,vendor 的 set 只取 Set 列,
    // 主键不进 SET(行 id 不被改写)。受辖实体 fail-closed(同
    // delete_where 哲学:行级范围必须经 update/update_without_row_scope
    // 的 WHERE 施加,不得由调用方在条件里自拼)。**不盖审计**:Set 什么
    // 落什么,update_by/update_time 由调用方手工携带(category 删除的
    // tombstone 惯用法恰好依赖审计列随 ActiveModel 携带)。空 SET 守卫同
    // update_query 的动机:vendor 的 is_noop 分支对空 SET 静默 Ok(0)——
    // 连同 WHERE 一起跳过,fail-closed 拒。0 行 → Ok(0) —— 批量语义无
    // NotFound 约定。
    pub(crate) async fn update_where<E>(
        &self,
        cond: Condition,
        mut am: E::ActiveModel,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        use sea_orm::{
            ActiveValue, ColumnTrait, Iterable, PrimaryKeyToColumn, QueryFilter,
        };

        if E::CONSTRAINED {
            return Err(DataError::Unsupported(
                "update_where 不接受受辖实体(CONSTRAINED=true):行级范围\
                 必须经 update/update_without_row_scope 的 WHERE 施加,不得\
                 由调用方在条件里自拼"
                    .into(),
            ));
        }

        // 主键不进 SET:cond 是唯一 WHERE 来源。先清 NotSet —— vendor 的
        // UpdateMany::set 跳过 NotSet,主键的 Set 值被忽略(行 id 不被改写)。
        for col in <E::Column as Iterable>::iter() {
            if <E::PrimaryKey as PrimaryKeyToColumn>::from_column(col).is_some()
            {
                am.not_set(col);
            }
        }

        // 空 SET 守卫(fail-closed):is_noop 分支对空 SET 静默 Ok(0) 连
        // WHERE 一起跳过 —— 判定与 update_query 同口径(主键不计,只有
        // ActiveValue::Set 进 SET)。
        let has_non_pk_set = <E::Column as Iterable>::iter().any(|col| {
            <E::PrimaryKey as PrimaryKeyToColumn>::from_column(col).is_none()
                && matches!(am.get(col), ActiveValue::Set(_))
        });
        if !has_non_pk_set {
            return Err(DataError::Unsupported(
                "空 SET 更新被拒绝:update_where 的 ActiveModel 除主键外\
                 无任何 Set 字段"
                    .into(),
            ));
        }

        let mut q = E::update_many().set(am);
        if let Some((col, alive)) = E::soft_delete() {
            q = q.filter(col.eq(alive));
        }
        let res = q.filter(cond).exec(self).await?;
        Ok(res.rows_affected)
    }

    // 按条件批量软删 —— Java `@TableLogic` 的批量写路径平价
    // (MyBatis-Plus deleteByMap/delete(Wrapper) 走 UPDATE del_flag,同此)。
    // WHERE = cond AND 软删存活:已删行不重复计数、不被新 dead 值重写。
    // fail-closed 双检查与 delete_where 同风格:受辖实体(CONSTRAINED=true)
    // 拒绝 —— 行级范围必须经 update/delete 的 WHERE 施加,不得由调用方在
    // 条件里自拼;无软删列实体拒绝 —— 没有 @TableLogic 可对齐,物理删走
    // delete_where。**不盖审计**:A 组实体的审计列类型与漏斗签名不匹配
    // (flow/policy.rs 头注释的裁定),update_by/update_time 由调用方手工盖
    // (Java 侧同为业务代码显式 set)。dead 值原样落库("1"/"2" 均可),
    // 取值是调用方的事。0 行返回 Ok(0) —— 批量语义无 NotFound 约定。
    pub(crate) async fn soft_delete_where<E>(
        &self,
        cond: Condition,
        dead: &str,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        use sea_orm::{ColumnTrait, QueryFilter};
        if E::CONSTRAINED {
            return Err(DataError::Unsupported(
                "soft_delete_where 不接受受辖实体(CONSTRAINED=true):行级范围\
                 必须经 update/delete 的 WHERE 施加,不得由调用方在条件里自拼"
                    .into(),
            ));
        }
        let Some((col, alive)) = E::soft_delete() else {
            return Err(DataError::Unsupported(
                "soft_delete_where 不接受无软删除列的实体:无 @TableLogic 可对齐,\
                 物理删请用 delete_where"
                    .into(),
            ));
        };
        let res = E::update_many()
            .col_expr(col, sea_orm::sea_query::Expr::value(dead))
            .filter(col.eq(alive))
            .filter(cond)
            .exec(self)
            .await?;
        Ok(res.rows_affected)
    }
}

impl DataContext<'_> {
    /// crate 内部(meta 模块)借共享执行核 —— 系统视图(information_schema/
    /// ALL_TABLES 等)没有 DataPolicy,元数据查询不经门控 API,直接走
    /// ContextCore 的原始执行面。刻意 pub(crate):业务 crate 命名不出
    /// ContextCore,公开面不扩大。
    pub(crate) fn core(&self) -> &ContextCore<'_> {
        &self.core
    }

    /// 惰性解析 + 请求内缓存,委托到 core。公开是因为测试用例要数解析次数。
    pub async fn scope_filter(&self) -> Result<&ScopeFilter, DataError> {
        self.core.scope_filter().await
    }

    /// 事务唯一入口。闭包 Ok → commit,Err → rollback(设计 7.5)。
    /// TxnContext 没有 transaction() —— 嵌套是编译错误,DM/ODBC 无 SAVEPOINT
    /// 的分歧因此不存在。
    pub async fn transaction<F, T>(&self, f: F) -> Result<T, DataError>
    where
        F: for<'c> FnOnce(
            &'c TxnContext<'c>,
        ) -> futures::future::BoxFuture<
            'c,
            Result<T, DataError>,
        >,
    {
        use sea_orm::TransactionTrait;
        let txn = self.core.conn()?.begin().await?;
        let child = TxnContext {
            core: ContextCore::new(
                Exec::Txn(&txn),
                self.core.principal.clone(),
                self.core.resolver.clone(),
                self.core.count_mode,
            ),
        };
        // 父已解析 → 预填子,整请求一次解析
        if let Some(sf) = self.core.scope.get() {
            let _ = child.core.scope.set(sf.clone());
        }
        match f(&child).await {
            Ok(v) => {
                txn.commit().await?;
                Ok(v)
            }
            Err(e) => {
                // 回滚失败**不能**用 `?` —— 那会顶掉触发回滚的业务错误:闭包返回
                // Conflict、同时连接已断导致 rollback 报错时,调用方只会看到
                // Backend(连接错误),409 变 500 且日志里没有冲突的任何痕迹。
                // 业务错误始终优先;连接真断了的话达梦/MySQL 也会在断连时隐式回滚。
                //
                // 这里刻意不记日志:app-data 一旦引入 tracing,就得往
                // crates/admin/src/main.rs 的 EnvFilter 默认值里加 `app_data=debug`
                // (否则事件被丢弃),而那是业务 crate —— 数据层不为此越权改它。
                let _ = txn.rollback().await;
                Err(e)
            }
        }
    }

    pub async fn find_by_id<E>(&self, id: i64) -> Result<E::Model, DataError>
    where
        E: DataPolicy,
        E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = i64>,
    {
        self.core.find_by_id::<E>(id).await
    }

    /// 分页查询:策略(软删除 + 行级范围)照施加,size 经 Paging 钳制(1..=200)。
    ///
    /// total 走数据源配置的口径(默认上限计数,1 万):不到上限时就是精确值,超出时返回上限
    /// 并置 [`Page::total_capped`]。要精确总数请走 [`Self::page_exact`]。
    pub async fn page<E>(
        &self,
        page_num: i64,
        page_size: i64,
        cond: Condition,
    ) -> Result<Page<E::Model>, DataError>
    where
        E: DataPolicy,
        E::Model: Send + Sync,
    {
        self.core
            .page::<E>(page_num, page_size, cond, self.core.count_mode)
            .await
    }

    /// 分页查询,total 为**全量精确计数**。
    ///
    /// 代价与结果集大小成正比(20 万行实测 PG 7.9 ms / MySQL 47.7 ms),
    /// 只给确实需要精确总数的场景 —— 典型是与第三方数据对账。
    /// 常规列表用 [`Self::page`]。
    pub async fn page_exact<E>(
        &self,
        page_num: i64,
        page_size: i64,
        cond: Condition,
    ) -> Result<Page<E::Model>, DataError>
    where
        E: DataPolicy,
        E::Model: Send + Sync,
    {
        self.core
            .page::<E>(page_num, page_size, cond, CountMode::Exact)
            .await
    }

    /// 表级估算行数:`pg_class.reltuples` / `information_schema.tables.table_rows`,
    /// 读统计目录、不扫表,代价与表大小无关。
    ///
    /// **不认 WHERE、不精确**(MySQL 实测偏差 1.4%),给的是整张表的量级 ——
    /// 用来回答"这张表大概多少条、量级对不对",不能拿去算页数。
    /// 方言不支持(SQLite / 达梦)、表不存在、或统计从未采集过时返回 None。
    pub async fn approx_row_count<E>(&self) -> Result<Option<i64>, DataError>
    where
        E: DataPolicy,
    {
        crate::count::approx_row_count(&self.core, E::default().table_name())
            .await
    }

    /// select 逃生舱入口(设计 6.4):保留 SeaORM 完整构造力,
    /// 策略(软删除 + 行级范围三态)在 SelectQuery 执行前施加。
    pub fn select<E>(
        &self,
        sel: sea_orm::Select<E>,
    ) -> crate::select_builder::SelectQuery<'_, E>
    where
        E: DataPolicy,
    {
        crate::select_builder::SelectQuery::new(&self.core, sel)
    }

    pub async fn insert<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.insert(am).await
    }

    /// 不回读的单条插入(委托到 core;语义见 ContextCore 上的实现说明)。
    pub async fn insert_returning_nothing<A>(
        &self,
        am: A,
    ) -> Result<(), DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        self.core.insert_returning_nothing(am).await
    }

    /// 批量插入(委托到 core;空集合 no-op)。
    pub async fn insert_many<A>(&self, ams: Vec<A>) -> Result<u64, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        self.core.insert_many(ams).await
    }

    /// **仅 test-util**:免盖章插入(设计 9 节第 4 条的种子原语)。
    /// 审计列取 ActiveModel 里调用方显式 `Set` 的值原样落库,
    /// 漏斗不盖 —— system 测试种"create_by=7 的越权行"全靠它表达。
    /// 其余语义与 `insert` 同口径(唯一键冲突 → Conflict)。生产代码拿不到
    /// 这个入口:默认 feature 下它不存在,trybuild 守着门控。
    #[cfg(any(test, feature = "test-util"))]
    pub async fn insert_unstamped<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.insert_unstamped(am).await
    }

    /// 更新:审计盖 update_*,行级范围进 WHERE,无匹配行 → NotFound(设计 7.4)。
    /// 委托到 core。
    pub async fn update<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.update(am, false).await
    }

    /// 写侧显式豁免行级范围(设计 7.4 修正)—— 对齐 Java `DataPermissionHelper
    /// .ignore`(SysProfileController:72/:98):自服务写路径(改自己资料)的
    /// "本人行"语义由业务 WHERE(user_id = 本人)表达,不经数据范围。
    /// 与读侧 without_row_scope 同一处置哲学:名字扎眼、可 grep、非静默默认。
    /// 软删除存活过滤与审计盖章保留(Java 的 ignore 不影响 @TableLogic);
    /// 空 SET 守卫与 RecordNotUpdated→NotFound 语义不变。仅跳过行级范围一项。
    pub async fn update_without_row_scope<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.update(am, true).await
    }

    /// 物理删除:行级范围进 WHERE,0 行受影响 → NotFound。委托到 core。
    pub async fn delete<E>(&self, id: i64) -> Result<(), DataError>
    where
        E: DataPolicy,
        E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = i64>,
    {
        self.core.delete::<E>(id).await
    }

    /// 按条件批量删(仅限豁免实体,受辖/带软删 fail-closed 拒绝)。
    /// 0 行 → Ok(0),批量语义无 NotFound 约定。委托到 core。
    pub async fn delete_where<E>(
        &self,
        cond: Condition,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        self.core.delete_where::<E>(cond).await
    }

    /// 按条件批量更新:单语句条件 UPDATE,WHERE = cond AND
    /// 软删存活;ActiveModel 的主键字段忽略(cond 是唯一 WHERE 来源);
    /// 不盖审计(Set 什么落什么);受辖实体 fail-closed;0 行 → Ok(0)。
    /// 完整语义见 ContextCore 上 update_where 实现处的注释。
    pub async fn update_where<E>(
        &self,
        cond: Condition,
        am: E::ActiveModel,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        self.core.update_where::<E>(cond, am).await
    }

    /// 按条件批量软删(@TableLogic 平价,仅限豁免 + 带软删列实体):
    /// WHERE = cond AND 软删存活,命中行置 dead 值,返回存活命中数;
    /// 受辖/无软删列实体 fail-closed 拒绝。不盖审计 —— update_*
    /// 由调用方手工盖。0 行 → Ok(0)。委托到 core。
    pub async fn soft_delete_where<E>(
        &self,
        cond: Condition,
        dead: &str,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        self.core.soft_delete_where::<E>(cond, dead).await
    }
}

impl TxnContext<'_> {
    /// 惰性解析 + 请求内缓存,委托到 core(父已解析时被 transaction 预填)。
    pub async fn scope_filter(&self) -> Result<&ScopeFilter, DataError> {
        self.core.scope_filter().await
    }

    pub async fn find_by_id<E>(&self, id: i64) -> Result<E::Model, DataError>
    where
        E: DataPolicy,
        E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = i64>,
    {
        self.core.find_by_id::<E>(id).await
    }

    /// 分页查询,与 DataContext 同构(委托到 core,total 同样走数据源配置的口径)。
    /// 事务内分页读的是事务快照 —— 未提交的插入/删除对同一事务里的 page 可见。
    pub async fn page<E>(
        &self,
        page_num: i64,
        page_size: i64,
        cond: Condition,
    ) -> Result<Page<E::Model>, DataError>
    where
        E: DataPolicy,
        E::Model: Send + Sync,
    {
        self.core
            .page::<E>(page_num, page_size, cond, self.core.count_mode)
            .await
    }

    /// 分页查询,total 为全量精确计数。与 [`DataContext::page_exact`] 同构。
    pub async fn page_exact<E>(
        &self,
        page_num: i64,
        page_size: i64,
        cond: Condition,
    ) -> Result<Page<E::Model>, DataError>
    where
        E: DataPolicy,
        E::Model: Send + Sync,
    {
        self.core
            .page::<E>(page_num, page_size, cond, CountMode::Exact)
            .await
    }

    /// select 逃生舱入口(设计 6.4),与 DataContext 同构。
    pub fn select<E>(
        &self,
        sel: sea_orm::Select<E>,
    ) -> crate::select_builder::SelectQuery<'_, E>
    where
        E: DataPolicy,
    {
        crate::select_builder::SelectQuery::new(&self.core, sel)
    }

    pub async fn insert<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.insert(am).await
    }

    /// 不回读的单条插入(委托到 core;语义见 ContextCore 上的实现说明)。
    pub async fn insert_returning_nothing<A>(
        &self,
        am: A,
    ) -> Result<(), DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        self.core.insert_returning_nothing(am).await
    }

    /// 批量插入(委托到 core;空集合 no-op)。
    pub async fn insert_many<A>(&self, ams: Vec<A>) -> Result<u64, DataError>
    where
        A: ActiveModelTrait + ActiveModelBehavior + Send,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
    {
        self.core.insert_many(ams).await
    }

    /// 更新,与 DataContext 同构(委托到 core)。事务内的更新/回滚由
    /// `transaction` 闭包的 Ok/Err 统一决定。
    pub async fn update<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.update(am, false).await
    }

    /// 写侧显式豁免行级范围,与 DataContext 同构(委托到 core)。Java 锚与
    /// 完整语义见 [`DataContext::update_without_row_scope`] 的文档 —— 对齐
    /// `DataPermissionHelper.ignore`(SysProfileController:72/:98),仅跳过
    /// 行级范围,软删/审计/空 SET 守卫照旧。
    pub async fn update_without_row_scope<A>(
        &self,
        am: A,
    ) -> Result<<A::Entity as EntityTrait>::Model, DataError>
    where
        A: ActiveModelTrait,
        A::Entity: DataPolicy + EntityTrait<ActiveModel = A>,
        <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    {
        self.core.update(am, true).await
    }

    /// 物理删除,与 DataContext 同构(委托到 core)。
    pub async fn delete<E>(&self, id: i64) -> Result<(), DataError>
    where
        E: DataPolicy,
        E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = i64>,
    {
        self.core.delete::<E>(id).await
    }

    /// 按条件批量删,与 DataContext 同构(委托到 core)。关联表的
    /// delete-then-insert 事务里由这里承担 DELETE 半边。
    pub async fn delete_where<E>(
        &self,
        cond: Condition,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        self.core.delete_where::<E>(cond).await
    }

    /// 按条件批量更新,与 DataContext 同构(委托到 core)。
    /// 事务内的提交/回滚由 `transaction` 闭包的 Ok/Err 统一决定;完整
    /// 语义(主键忽略/不盖审计/受辖 fail-closed/0 行 Ok(0))见
    /// [`DataContext::update_where`]。
    pub async fn update_where<E>(
        &self,
        cond: Condition,
        am: E::ActiveModel,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        self.core.update_where::<E>(cond, am).await
    }

    /// 按条件批量软删,与 DataContext 同构(委托到 core)。事务内的软删
    /// 由 `transaction` 闭包的 Ok/Err 统一决定提交/回滚。
    pub async fn soft_delete_where<E>(
        &self,
        cond: Condition,
        dead: &str,
    ) -> Result<u64, DataError>
    where
        E: DataPolicy,
    {
        self.core.soft_delete_where::<E>(cond, dead).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use sea_orm::{ColumnTrait, EntityTrait, QueryOrder}; // crate 内部测试,不走受限模块

    use crate::{
        error::DataError,
        principal::{Principal, ScopeFilter},
        testing,
    };

    struct CountingResolver(AtomicUsize);
    #[async_trait::async_trait]
    impl crate::ScopeResolver for CountingResolver {
        async fn resolve(
            &self,
            _p: &Principal,
        ) -> Result<ScopeFilter, DataError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ScopeFilter::All)
        }
    }

    #[tokio::test]
    async fn resolver_is_lazy_and_cached_per_context() {
        let ds = testing::sqlite_memory().await;
        let r = Arc::new(CountingResolver(AtomicUsize::new(0)));
        let ds = ds.with_resolver(r.clone());
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        // 一次也没解析过
        assert_eq!(r.0.load(Ordering::SeqCst), 0);
        ctx.scope_filter().await.unwrap(); // 首次
        ctx.scope_filter().await.unwrap(); // 缓存
        ctx.scope_filter().await.unwrap();
        assert_eq!(
            r.0.load(Ordering::SeqCst),
            1,
            "OnceCell:每请求最多解析一次"
        );
    }

    /// `derive_with_resolver` 判例(装配层鸡蛋环的破解出口)。
    /// 派生体 resolver 生效(受辖读施加范围);母体不受染(context() 仍
    /// fail-closed Unsupported —— 母体正是 resolver 自有 unscoped 通道的
    /// 形态,scope_resolver.rs 的递归防御第一参);连接句柄共享(同池同库)。
    #[tokio::test]
    async fn derive_with_resolver_takes_effect_and_leaves_mother_uncontaminated()
     {
        use testing::{FixedResolver, seed};
        let mother = testing::sqlite_memory().await;
        seed(&mother, vec![(1, 10, 1, "0"), (2, 99, 2, "0")]).await;

        let derived = mother.derive_with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));

        // 派生体:resolver 生效 —— 越权部门(99)对受辖读 NotFound
        let dctx = derived.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        assert!(
            matches!(
                dctx.find_by_id::<testing::sandbox::Entity>(2)
                    .await
                    .unwrap_err(),
                DataError::NotFound
            ),
            "派生体必须带 resolver 施加行级范围"
        );
        assert!(
            dctx.find_by_id::<testing::sandbox::Entity>(1).await.is_ok(),
            "范围内行对派生体可见"
        );

        // 母体不受染:context() 仍 fail-closed(无 resolver 不得静默 All)
        let err = mother
            .context(Principal {
                user_id: 1,
                dept_id: Some(10),
            })
            .scope_filter()
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "derive 是只读派生,母体(装配层的 unscoped 通道)不得拿到 resolver"
        );

        // 连接句柄共享:母体种的行,派生体 system_context 可见
        let row = derived
            .system_context()
            .find_by_id::<testing::sandbox::Entity>(1)
            .await
            .unwrap();
        assert_eq!(row.id, 1, "派生体与母体共享同一连接句柄(同池同库)");
    }

    #[tokio::test]
    async fn context_without_resolver_fails_closed() {
        let ds = testing::sqlite_memory().await;
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: None,
        });
        let err = ctx.scope_filter().await.unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "无 resolver 不得静默 All"
        );
    }

    #[tokio::test]
    async fn transaction_prefills_child_scope_from_parent() {
        let ds = testing::sqlite_memory().await;
        let r = Arc::new(CountingResolver(AtomicUsize::new(0)));
        let ds = ds.with_resolver(r.clone());
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        // 父先解析一次(计数 = 1)
        ctx.scope_filter().await.unwrap();
        // 子 context 读到的是预填值,不得再触发 resolver
        ctx.transaction(|tx| {
            Box::pin(async move {
                tx.scope_filter().await?;
                Ok::<(), DataError>(())
            })
        })
        .await
        .unwrap();
        // 预填被删的话,子会自己解析一次 → 计数变 2,此断言变红
        assert_eq!(
            r.0.load(Ordering::SeqCst),
            1,
            "事务子 context 必须复用父级已解析的 ScopeFilter"
        );
    }

    #[tokio::test]
    async fn transaction_commits_on_ok_and_rolls_back_on_err() {
        use testing::sandbox_row;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();

        ctx.transaction(|tx| {
            Box::pin(async move {
                tx.insert(sandbox_row(1, Some(10), Some(1))).await?;
                tx.insert(sandbox_row(2, Some(10), Some(1))).await?;
                Ok::<(), DataError>(())
            })
        })
        .await
        .unwrap();

        // 回滚分支:先插 3 再报错 → 3 不落库
        let _ = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    tx.insert(sandbox_row(3, Some(10), Some(1))).await?;
                    Err::<(), DataError>(DataError::NotFound)
                })
            })
            .await;

        assert!(ctx.find_by_id::<testing::sandbox::Entity>(1).await.is_ok());
        assert!(
            ctx.find_by_id::<testing::sandbox::Entity>(3).await.is_err(),
            "Err 分支整体回滚"
        );
    }

    // ---- 读路径(运行时语义组)----

    #[tokio::test]
    async fn find_by_id_applies_soft_delete_and_scope() {
        use testing::{FixedResolver, seed};
        // 行: id=1 dept=10 owner=1 del_flag='0'
        //      id=2 dept=99 owner=2 del_flag='0'  ← 越权部门
        //      id=3 dept=10 owner=3 del_flag='2'  ← 已删
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 10, 1, "0"), (2, 99, 2, "0"), (3, 10, 3, "2")])
            .await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        assert!(ctx.find_by_id::<testing::sandbox::Entity>(1).await.is_ok());
        assert!(
            matches!(
                ctx.find_by_id::<testing::sandbox::Entity>(2)
                    .await
                    .unwrap_err(),
                DataError::NotFound
            ),
            "越权部门 → NotFound(不泄露存在性)"
        );
        assert!(
            matches!(
                ctx.find_by_id::<testing::sandbox::Entity>(3)
                    .await
                    .unwrap_err(),
                DataError::NotFound
            ),
            "软删除 → NotFound"
        );
        // system_context 跳过行级范围,但软删除仍在
        let sctx = ds.system_context();
        assert!(sctx.find_by_id::<testing::sandbox::Entity>(2).await.is_ok());
        assert!(matches!(
            sctx.find_by_id::<testing::sandbox::Entity>(3)
                .await
                .unwrap_err(),
            DataError::NotFound
        ));
    }

    #[tokio::test]
    async fn unconstrained_driver_requires_declaration() {
        use testing::FixedResolver;
        let ds = testing::sqlite_memory().await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        // dict_entry CONSTRAINED=false,未声明落点 → 执行期 fail-closed(设计 6.4)
        let err = ctx
            .select(testing::dict_entry::Entity::find())
            .all()
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Unsupported(_)));
        // 显式豁免后放行
        let rows = ctx
            .select(testing::dict_entry::Entity::find())
            .without_row_scope()
            .all()
            .await
            .unwrap();
        assert_eq!(rows.len(), 0);
    }

    /// `without_row_scope()` 与不受约束驱动都不需要范围 —— 就**不该**触发
    /// ScopeResolver。这里用「压根没配 resolver 的 DataSource」把它钉死:
    /// 若 apply() 还像早先那样无条件解析,这条会拿到 Unsupported。
    /// 现实后果不止是报错:设计 7.2 的 resolver 要查 sys_role_dept 并展开科室树,
    /// 无条件解析等于给每个只读字典表的请求白搭一次查库。
    #[tokio::test]
    async fn without_row_scope_does_not_need_a_resolver() {
        let ds = testing::sqlite_memory().await; // 刻意不 with_resolver
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let rows = ctx
            .select(testing::dict_entry::Entity::find())
            .without_row_scope()
            .all()
            .await
            .expect("显式豁免范围的查询不该要求 resolver");
        assert_eq!(rows.len(), 0);

        // 反面对照:受约束驱动仍然必须有 resolver,fail-closed 不变
        let err = ctx
            .select(testing::sandbox::Entity::find())
            .all()
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "受约束驱动无 resolver 仍须 fail-closed"
        );
    }

    // ---- 读路径(SQL 断言组,只 build 不执行,设计 9.1)----

    /// constrain_via 的全部职责:把范围条件落到**声明的那个实体**的列上,且带表限定。
    #[tokio::test]
    async fn constrain_via_puts_scope_on_the_declared_entity() {
        use testing::FixedResolver;
        let ds = testing::sqlite_memory().await.with_resolver(Arc::new(
            FixedResolver(ScopeFilter::DeptIn(vec![10, 20])),
        ));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });

        let sql = ctx
            .select(testing::dict_entry::Entity::find())
            .constrain_via::<testing::sandbox::Entity>()
            .into_sql(sea_orm::DbBackend::Sqlite)
            .await
            .unwrap();

        // 驱动是 dict_entry,范围却落在 sandbox 的列上 —— 这就是"落点声明"的含义
        assert!(sql.contains("FROM \"dict_entry\""));
        assert!(sql.contains("\"sandbox\".\"dept_id\" IN (10, 20)"));
        // dict_entry 自己没有软删除列 → 不该凭空加过滤
        assert!(!sql.contains("del_flag"));
    }

    /// **受约束驱动 + constrain_via 必须报错,不许静默生效。**
    ///
    /// 曾经的行为:三态只走一条分支,Via 把驱动 sandbox 自身的
    /// `"sandbox"."dept_id" IN (10)` 整条顶掉,只剩 `"sys_user"."dept_id" IN (10)`。
    /// 调用方若真的 join 了 sys_user(constrain_via 本就为 join 而设),SQL 正常
    /// 执行,而 sandbox 行不再受调用者部门限制 —— 静默的跨部门泄漏。
    #[tokio::test]
    async fn constrain_via_on_constrained_driver_is_rejected() {
        use testing::FixedResolver;
        let ds = testing::sqlite_memory().await.with_resolver(Arc::new(
            FixedResolver(ScopeFilter::DeptIn(vec![10])),
        ));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });

        // 基线:同一驱动不声明落点时,自身范围在
        let base = ctx
            .select(testing::sandbox::Entity::find())
            .into_sql(sea_orm::DbBackend::Sqlite)
            .await
            .unwrap();
        assert!(base.contains("\"sandbox\".\"dept_id\" IN (10)"));

        // 加一句 constrain_via → 拒绝,而不是"悄悄换掉落点"
        let err = ctx
            .select(testing::sandbox::Entity::find())
            .constrain_via::<testing::sys_user::Entity>()
            .into_sql(sea_orm::DbBackend::Sqlite)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Unsupported(_)));

        // 执行路径同样拒绝(不只是 into_sql 这条 build 路径)
        let err = ctx
            .select(testing::sandbox::Entity::find())
            .constrain_via::<testing::sys_user::Entity>()
            .all()
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Unsupported(_)));
    }

    #[tokio::test]
    async fn constrain_via_with_self_only_uses_declared_owner_column() {
        use testing::FixedResolver;
        let ds = testing::sqlite_memory()
            .await
            .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))));
        let ctx = ds.context(Principal {
            user_id: 7,
            dept_id: Some(10),
        });
        let sql = ctx
            .select(testing::dict_entry::Entity::find())
            .constrain_via::<testing::sandbox::Entity>()
            .into_sql(sea_orm::DbBackend::Sqlite)
            .await
            .unwrap();
        assert!(sql.contains("\"sandbox\".\"owner_id\" = 7"));
    }

    /// 写路径软删除语义(对齐 @TableLogic):已删行对 update/delete 均不可见。
    /// 部门在本侧、仅 del_flag='2' 的行,越权与否都表现为 NotFound。
    #[tokio::test]
    async fn update_on_soft_deleted_row_is_not_found() {
        use testing::{FixedResolver, seed};

        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 10, 1, "2")]).await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let mut am = testing::sandbox::ActiveModel {
            id: Set(1),
            ..Default::default()
        };
        am.del_flag = Set("0".into()); // "恢复"尝试:必须被存活过滤挡下
        let err = ctx.update(am).await.unwrap_err();
        assert!(
            matches!(err, DataError::NotFound),
            "已删行不可经普通 update 恢复(需显式旁路)"
        );
    }

    #[tokio::test]
    async fn delete_on_soft_deleted_row_is_not_found() {
        use testing::{FixedResolver, seed};
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(2, 10, 1, "2")]).await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let err = ctx.delete::<testing::sandbox::Entity>(2).await.unwrap_err();
        assert!(
            matches!(err, DataError::NotFound),
            "已删行的物理删除同样拒绝,静默清库不可接受"
        );
    }

    /// **已知缺口,有意用测试钉住(设计 11 节末行那条风险的具体形状)**:
    /// `constrain_via` 只负责合成条件,**不校验调用方真的 join 了 E2**。
    /// 没 join 就是执行期后端报错 —— 仍然 fail-closed(不会静默放行全表),
    /// 但既不是编译错误也不是 `DataError::Unsupported`。
    #[tokio::test]
    async fn constrain_via_without_actual_join_fails_at_execution() {
        use testing::FixedResolver;
        let ds = testing::sqlite_memory().await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let err = ctx
            .select(testing::dict_entry::Entity::find())
            .constrain_via::<testing::sandbox::Entity>()
            .all()
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Backend(_)),
            "未 join 就声明落点 → 后端报错(no such column),不静默放行"
        );
    }

    // ---- 分页 ----

    #[tokio::test]
    async fn page_applies_scope_and_counts_total() {
        use sea_orm::Condition;
        use testing::{FixedResolver, seed};
        // Condition::new() 不存在 —— sea-query 的 Condition 只有 all()/any()/not()。
        // 空的 all() 就是"无附加条件"。
        let empty = || Condition::all();

        let ds = testing::sqlite_memory().await;
        let rows: Vec<(i64, i64, i64, &str)> =
            (1..=25).map(|i| (i, 10, 1, "0")).collect();
        seed(&ds, rows).await;
        seed(&ds, vec![(100, 99, 2, "0")]).await; // 越权部门,不计入
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });

        let p1 = ctx
            .page::<testing::sandbox::Entity>(1, 10, empty())
            .await
            .unwrap();
        assert_eq!(p1.total, 25, "total 是过滤后的总数,越权那行不计入");
        assert_eq!(p1.rows.len(), 10);

        let p3 = ctx
            .page::<testing::sandbox::Entity>(3, 10, empty())
            .await
            .unwrap();
        assert_eq!(p3.rows.len(), 5, "第 3 页只剩 5 行");

        // size=9999 被钳到 200;库里符合范围的只有 25 行 → 全量返回 25
        let clamped = ctx
            .page::<testing::sandbox::Entity>(1, 9999, empty())
            .await
            .unwrap();
        assert_eq!(clamped.rows.len(), 25);
        assert_eq!(clamped.total, 25);

        // page 超出范围 → 空页,不报错
        let overflow = ctx
            .page::<testing::sandbox::Entity>(99, 10, empty())
            .await
            .unwrap();
        assert!(overflow.rows.is_empty());
        assert_eq!(overflow.total, 25, "total 与页码无关");

        // 默认口径是上限计数(1 万),25 行远未触顶 → 与精确计数逐位相同
        assert!(!p1.total_capped, "未触顶时 total 就是精确值");
        assert_eq!(p1.total_approx, None, "未触顶不去问目录");
        let exact = ctx
            .page_exact::<testing::sandbox::Entity>(1, 10, empty())
            .await
            .unwrap();
        assert_eq!(exact.total, p1.total, "未触顶时两条口径必须给出同一个数");
        assert!(!exact.total_capped);
    }

    /// 上限计数的三段语义,用小上限在小数据上钉死(生产上限 1 万,
    /// 塞一万行只为验同一段逻辑不划算):
    /// 上限内 → 精确且不置标记;恰好等于上限 → 精确且不置标记(边界不能早触发);
    /// 超出上限 → 返回上限并置标记,同时精确口径仍能拿到真值。
    #[tokio::test]
    async fn capped_count_truncates_only_beyond_the_cap() {
        use testing::seed;
        let ds = testing::sqlite_memory().await;
        let rows: Vec<(i64, i64, i64, &str)> =
            (1..=25).map(|i| (i, 10, 1, "0")).collect();
        seed(&ds, rows).await;
        let ctx = ds.system_context();
        let sel = || testing::sandbox::Entity::find();

        let under = ctx.select(sel()).total_cap(30).page(1, 5).await.unwrap();
        assert_eq!(under.total, 25);
        assert!(!under.total_capped);

        // 边界:总数 == 上限。子查询取 cap + 1 行只取到 25 → 不算触顶。
        let at = ctx.select(sel()).total_cap(25).page(1, 5).await.unwrap();
        assert_eq!(at.total, 25);
        assert!(!at.total_capped, "恰好等于上限不是截断");

        let over = ctx.select(sel()).total_cap(10).page(1, 5).await.unwrap();
        assert_eq!(over.total, 10, "触顶只承认上限");
        assert!(over.total_capped);
        assert_eq!(over.rows.len(), 5, "截断的只是总数,行照常取");

        // 要真值就显式要:对账场景走这条
        let exact = ctx.select(sel()).exact_total().page(1, 5).await.unwrap();
        assert_eq!(exact.total, 25);
        assert!(!exact.total_capped);
    }

    /// 数据源级默认口径:没显式指定口径的 `page()` 应当跟随 `DataSource` 的
    /// 配置,而不是恒为 `CountMode::default()`。压测的「同口径基线」靠这条
    /// 把 axum 换回与 RuoYi 相同的全量计数。
    #[tokio::test]
    async fn data_source_count_mode_becomes_the_default_for_page() {
        use sea_orm::Condition;
        use testing::seed;

        use crate::count::CountMode;
        let empty = || Condition::all();
        let rows = || -> Vec<(i64, i64, i64, &'static str)> {
            (1..=25).map(|i| (i, 10, 1, "0")).collect()
        };

        // 不配 → 沿用 Capped(1 万),25 行远未触顶
        let ds = testing::sqlite_memory().await;
        seed(&ds, rows()).await;
        let dflt = ds
            .system_context()
            .page::<testing::sandbox::Entity>(1, 5, empty())
            .await
            .unwrap();
        assert_eq!(dflt.total, 25);
        assert!(!dflt.total_capped);

        // 配成小上限 → page() 跟着触顶
        let ds = testing::sqlite_memory().await;
        seed(&ds, rows()).await;
        let ds = ds.with_count_mode(CountMode::Capped(10));
        let capped = ds
            .system_context()
            .page::<testing::sandbox::Entity>(1, 5, empty())
            .await
            .unwrap();
        assert_eq!(capped.total, 10, "总数跟随数据源口径触顶");
        assert!(capped.total_capped);

        // 配成 Exact → 不带 LIMIT 的全量计数,拿回真值
        let ds = testing::sqlite_memory().await;
        seed(&ds, rows()).await;
        let ds = ds.with_count_mode(CountMode::Exact);
        let exact = ds
            .system_context()
            .page::<testing::sandbox::Entity>(1, 5, empty())
            .await
            .unwrap();
        assert_eq!(exact.total, 25);
        assert!(!exact.total_capped);
    }

    /// 生产装配里业务查询走的是 `derive_with_resolver` 的派生体,口径必须
    /// 跟着派生过去 —— 只配在母体上等于没配。
    #[tokio::test]
    async fn derive_with_resolver_carries_count_mode() {
        use sea_orm::Condition;
        use testing::seed;

        use crate::count::CountMode;
        let empty = || Condition::all();
        let ds = testing::sqlite_memory().await;
        let rows: Vec<(i64, i64, i64, &str)> =
            (1..=3).map(|i| (i, 10, 1, "0")).collect();
        seed(&ds, rows).await;

        // cap = 0 是最容易分辨的口径:任何非空结果都会触顶
        let ds0 = ds.with_count_mode(CountMode::Capped(0));
        let ds1 = ds0.derive_with_resolver(Arc::new(testing::FixedResolver(
            ScopeFilter::All,
        )));
        let p = ds1
            .context(Principal {
                user_id: 1,
                dept_id: Some(10),
            })
            .page::<testing::sandbox::Entity>(1, 5, empty())
            .await
            .unwrap();
        assert_eq!(p.total, 0);
        assert!(p.total_capped, "派生体必须继承母体的计数口径");
    }

    /// `select()` 建造器的初始口径同样来自数据源,而不是硬编码的
    /// `CountMode::default()` —— 否则同一个进程里 `page()` 与
    /// `select().page()` 会给出两种总数。
    #[tokio::test]
    async fn select_builder_starts_from_data_source_count_mode() {
        use testing::seed;

        use crate::count::CountMode;
        let ds = testing::sqlite_memory().await;
        let rows: Vec<(i64, i64, i64, &str)> =
            (1..=25).map(|i| (i, 10, 1, "0")).collect();
        seed(&ds, rows).await;
        let ds = ds.with_count_mode(CountMode::Capped(10));
        let ctx = ds.system_context();

        let p = ctx
            .select(testing::sandbox::Entity::find())
            .page(1, 5)
            .await
            .unwrap();
        assert_eq!(p.total, 10);
        assert!(p.total_capped);

        // 逐调用点的显式指定仍然优先于数据源默认值
        let exact = ctx
            .select(testing::sandbox::Entity::find())
            .exact_total()
            .page(1, 5)
            .await
            .unwrap();
        assert_eq!(exact.total, 25);
        assert!(!exact.total_capped);
    }

    /// SQLite 没有统计目录,`approx_row_count` 必须**返回 None 而不是报错**
    /// —— 分页触顶时它是可选的展示位,方言不支持就该静默退化。
    #[tokio::test]
    async fn approx_row_count_degrades_to_none_without_catalog() {
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        let est = ctx
            .approx_row_count::<testing::sandbox::Entity>()
            .await
            .unwrap();
        assert_eq!(est, None, "SQLite 无目录估算 → None,不报错");
    }

    /// 目录估算**不认 WHERE**,所以只有"这个查询选出的就是整张表"时才能给。
    ///
    /// 判定不靠调用方声明,而是把同样的策略施加到空查询上比行集指纹。这条钉的
    /// 就是那个判据:带了用户条件的查询必须拿不到估算。SQLite 上估算恒为 None,
    /// 断言因此落在**指纹是否相等**上 —— 相等才轮得到去问目录。
    #[tokio::test]
    async fn approx_estimate_is_refused_once_a_user_filter_is_added() {
        use testing::seed;

        use crate::sea_orm::{ColumnTrait, QueryFilter, QueryTrait};
        let ds = testing::sqlite_memory().await;
        let rows: Vec<(i64, i64, i64, &str)> =
            (1..=25).map(|i| (i, 10, 1, "0")).collect();
        seed(&ds, rows).await;
        let ctx = ds.system_context();
        let backend = sea_orm::DbBackend::Sqlite;

        // 基线:空查询 + 策略(system_context 无行级范围,只剩软删除)
        let base = crate::count::row_set_key(
            backend,
            testing::sandbox::Entity::find()
                .filter(testing::sandbox::Column::DelFlag.eq("0"))
                .into_query(),
        );
        // 同形状的查询 → 指纹相等 → 可以去问目录
        let same = crate::count::row_set_key(
            backend,
            testing::sandbox::Entity::find()
                .filter(testing::sandbox::Column::DelFlag.eq("0"))
                .into_query(),
        );
        assert_eq!(base, same, "没加条件 ⇒ 行集指纹逐位相等");

        // 加了用户条件 → 指纹不同 → 不给估算
        let filtered = crate::count::row_set_key(
            backend,
            testing::sandbox::Entity::find()
                .filter(testing::sandbox::Column::DelFlag.eq("0"))
                .filter(testing::sandbox::Column::DeptId.eq(10))
                .into_query(),
        );
        assert_ne!(base, filtered, "加了条件 ⇒ 指纹必须不同,整表估算不可用");

        // 绑定值不同也算不同(Statement 的 PartialEq 覆盖 values):
        // 只比 SQL 文本的话,del_flag='0' 与 del_flag='2' 会被误判成同一个查询。
        let other_value = crate::count::row_set_key(
            backend,
            testing::sandbox::Entity::find()
                .filter(testing::sandbox::Column::DelFlag.eq("2"))
                .into_query(),
        );
        assert_ne!(base, other_value, "绑定值不同也必须判为不同的行集");

        // 端到端:触顶 + 无条件,SQLite 无目录 → 静默退化成 None,不报错
        let p = ctx
            .select(testing::sandbox::Entity::find())
            .total_cap(10)
            .page(1, 5)
            .await
            .unwrap();
        assert!(p.total_capped);
        assert_eq!(p.total_approx, None);
    }

    /// 逃生舱分页(设计 6.4 ORDER BY 裁定):排序在 Select 构件上,
    /// 策略注入与窄 API 同源 —— 过滤后 total、Paging 钳制、三态范围。
    #[tokio::test]
    async fn escape_hatch_page_keeps_order_and_applies_scope() {
        use testing::{FixedResolver, seed};

        use crate::sea_orm::QueryOrder; // order_by_desc;Column 只作值传递,无需 ColumnTrait
        let ds = testing::sqlite_memory().await;
        let rows: Vec<(i64, i64, i64, &str)> =
            (1..=12).map(|i| (i, 10, 1, "0")).collect();
        seed(&ds, rows).await;
        seed(&ds, vec![(100, 99, 2, "0")]).await; // 越权部门,不入 total
        seed(&ds, vec![(101, 10, 1, "2")]).await; // 已删,不入 total
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });

        let sel = testing::sandbox::Entity::find()
            .order_by_desc(testing::sandbox::Column::Id);
        let p1 = ctx.select(sel).page(1, 5).await.unwrap();
        assert_eq!(
            p1.total, 12,
            "total 是过滤后总数(种子 14 行:12 可见 + 1 越权部门 + 1 已软删,后两者不计入)"
        );
        assert_eq!(p1.rows.len(), 5);
        assert_eq!(p1.rows[0].id, 12, "desc 排序在分页中保持");

        let p3 = ctx
            .select(
                testing::sandbox::Entity::find()
                    .order_by_desc(testing::sandbox::Column::Id),
            )
            .page(3, 5)
            .await
            .unwrap();
        assert_eq!(p3.rows.len(), 2, "12 行 / 每页 5 → 第三页 2 行");
        assert_eq!(p3.rows[0].id, 2);

        let p99 = ctx
            .select(
                testing::sandbox::Entity::find()
                    .order_by_desc(testing::sandbox::Column::Id),
            )
            .page(99, 5)
            .await
            .unwrap();
        assert_eq!(p99.rows.len(), 0, "越界页码返回空页,不报错");
        assert_eq!(p99.total, 12);
    }

    // ---- 写路径(审计盖章 + 行级范围进 WHERE + 0 行 NotFound)----

    #[tokio::test]
    async fn insert_stamps_audit() {
        let ds = testing::sqlite_memory().await;
        let ctx = ds.context(Principal {
            user_id: 42,
            dept_id: Some(10),
        });
        let m = ctx
            .insert(testing::sandbox_row(1, Some(10), None))
            .await
            .unwrap();
        assert_eq!(m.create_by, Some(42), "审计人 = principal.user_id");
        assert_eq!(m.update_by, Some(42), "RuoYi 习惯:插入也写 update_*");
        assert!(m.create_time.is_some());

        let sctx = ds.system_context();
        let m = sctx
            .insert(testing::sandbox_row(2, Some(10), None))
            .await
            .unwrap();
        assert_eq!(m.create_by, Some(0), "系统上下文审计人 = 0(系统)");
    }

    /// create_dept 盖的是**部门**不是人。sandbox 没有这列,拿真实实体 sys_user 测。
    #[tokio::test]
    async fn insert_stamps_create_dept_from_principal_dept() {
        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.context(Principal {
            user_id: 42,
            dept_id: Some(103),
        });
        let am = testing::sys_user::ActiveModel {
            user_id: Set(1),
            dept_id: Set(Some(103)),
            user_name: Set("u1".into()),
            nick_name: Set("n1".into()),
            password: Set("x".into()),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        };
        let m = ctx.insert(am).await.unwrap();
        assert_eq!(m.create_dept, Some(103), "create_dept = principal.dept_id");
        assert_eq!(m.create_by, Some(42), "create_by 才是 principal.user_id");
    }

    #[tokio::test]
    async fn update_denied_by_scope_is_not_found() {
        use testing::{FixedResolver, seed};

        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 99, 2, "0")]).await; // 别人部门的行
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let am = testing::sandbox::ActiveModel {
            id: Set(1),
            del_flag: Set("2".into()),
            ..Default::default()
        };
        let err = ctx.update(am).await.unwrap_err();
        assert!(
            matches!(err, DataError::NotFound),
            "写越权 → NotFound,绝不静默 Ok"
        );

        // 而且必须**真的没改**:换 system_context 读回来确认还是 '0'
        let row = ds
            .system_context()
            .find_by_id::<testing::sandbox::Entity>(1)
            .await
            .unwrap();
        assert_eq!(row.del_flag, "0", "越权的 update 不得落库");
    }

    #[tokio::test]
    async fn update_stamps_updater() {
        use testing::{FixedResolver, seed};

        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 10, 1, "0")]).await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 7,
            dept_id: Some(10),
        });
        let am = testing::sandbox::ActiveModel {
            id: Set(1),
            del_flag: Set("2".into()),
            ..Default::default()
        };
        let m = ctx.update(am).await.unwrap();
        assert_eq!(m.update_by, Some(7));
        assert_eq!(m.del_flag, "2", "exec() 直接返回更新后的 Model,不需要回读");
    }

    /// 空 SET 守卫:dict_entry 没有审计列,PK-only ActiveModel 盖章后 SET
    /// 仍为空。没有守卫时 vendored executor 的 is_noop 分支会跳过 WHERE
    /// (主键 + 范围)直接回读,任何存在的行都静默 Ok —— 这里必须 Unsupported,
    /// 且行不得被改动。有审计列的实体(如 sandbox)盖章后 SET 非空,不在此列。
    #[tokio::test]
    async fn update_with_empty_set_is_rejected_not_silent_ok() {
        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        ctx.insert(testing::dict_entry::ActiveModel {
            id: Set(1),
            label: Set("原值".into()),
            remark: Set("备注".into()),
        })
        .await
        .unwrap();

        let err = ctx
            .update(testing::dict_entry::ActiveModel {
                id: Set(1),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "空 SET 更新必须被拒,不得走 vendor 的无范围回读旁路返回 Ok"
        );

        // 行必须原样:被拒的 update 不得产生任何副作用
        let row = ctx
            .find_by_id::<testing::dict_entry::Entity>(1)
            .await
            .unwrap();
        assert_eq!(row.label, "原值");
        assert_eq!(row.remark, "备注");
    }

    // ---- 写侧显式豁免行级范围(update_without_row_scope,设计 7.4 修正)----

    /// ① SelfOnly 操作者改"归属人不是本人"的行:受辖 update 0 行 → NotFound;
    /// 豁免路径成功,且审计列照盖(= principal)。这就是 Java
    /// SysProfileController:72/:98 用 DataPermissionHelper.ignore 包住的
    /// 自服务写形状 —— "本人行"语义由业务 WHERE(此处为主键)表达,不经数据范围。
    #[tokio::test]
    async fn update_without_row_scope_crosses_owner_and_stamps_audit() {
        use testing::{FixedResolver, seed};

        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 10, 2, "0")]).await; // owner=2,不是本人
        let ds =
            ds.with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(1))));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });

        // 反面对照:受辖 update 仍被 owner 条件挡下(0 行 → NotFound)
        let err = ctx
            .update(testing::sandbox::ActiveModel {
                id: Set(1),
                del_flag: Set("2".into()),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::NotFound),
            "受辖 update 仍受范围约束,实际: {err:?}"
        );

        // 豁免路径:成功 + 审计盖章
        let m = ctx
            .update_without_row_scope(testing::sandbox::ActiveModel {
                id: Set(1),
                del_flag: Set("2".into()),
                ..Default::default()
            })
            .await
            .expect("豁免行级范围后应可改他人的行");
        assert_eq!(m.del_flag, "2", "更新已生效");
        assert_eq!(
            m.update_by,
            Some(1),
            "豁免的是行级范围,不是审计:update_by 仍盖 principal"
        );
        assert!(m.update_time.is_some(), "update_time 同样照盖");
    }

    /// ② 豁免只跳过行级范围一项:软删除存活过滤照旧 —— 仅 del_flag='2' 的
    /// 行,豁免路径同样 NotFound(Java 的 ignore 不影响 @TableLogic 的对应面),
    /// "恢复"尝试必须被挡下。刻意**不配 resolver**:豁免路径连 ScopeFilter
    /// 都不解析(同读侧 without_row_scope),若仍去解析会先拿到
    /// Unsupported(fail-closed), NotFound 与 Unsupported 的判别同时钉住
    /// 「软删仍在」与「范围未解析」两件事。
    #[tokio::test]
    async fn update_without_row_scope_still_respects_soft_delete() {
        use testing::seed;

        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await; // 刻意不 with_resolver
        seed(&ds, vec![(1, 10, 1, "2")]).await;
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let err = ctx
            .update_without_row_scope(testing::sandbox::ActiveModel {
                id: Set(1),
                del_flag: Set("0".into()), // "恢复"尝试:必须被存活过滤挡下
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::NotFound),
            "豁免行级范围 ≠ 豁免软删除,且不经 resolver(不得 Unsupported),实际: {err:?}"
        );
    }

    /// ③ 空 SET 守卫在豁免路径同样生效:dict_entry 无审计列,PK-only
    /// ActiveModel → Unsupported,不得走 vendor 的无范围回读旁路。
    #[tokio::test]
    async fn update_without_row_scope_empty_set_still_rejected() {
        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        ctx.insert(testing::dict_entry::ActiveModel {
            id: Set(1),
            label: Set("原值".into()),
            remark: Set("备注".into()),
        })
        .await
        .unwrap();
        let err = ctx
            .update_without_row_scope(testing::dict_entry::ActiveModel {
                id: Set(1),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "豁免路径的空 SET 同样必须被拒,实际: {err:?}"
        );
    }

    /// ④ SQL 断言(只 build 不执行,设计 9.1):豁免路径生成的 WHERE 只有
    /// 主键 + 软删存活,无任何 dept/owner 条件;受辖对照路径带 owner 条件。
    /// 经 update_query(构造半边)断言,保证测的就是执行路径的同一具身。
    #[tokio::test]
    async fn update_without_row_scope_sql_keeps_soft_delete_drops_scope() {
        use sea_orm::QueryTrait; // QueryTrait::build(只 build 不执行)
        use testing::FixedResolver;

        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory()
            .await
            .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))));
        let ctx = ds.context(Principal {
            user_id: 7,
            dept_id: Some(10),
        });
        let am = || testing::sandbox::ActiveModel {
            id: Set(1),
            del_flag: Set("2".into()),
            ..Default::default()
        };

        let exempt_sql = ctx
            .core
            .update_query(am(), true)
            .await
            .unwrap()
            .build(sea_orm::DbBackend::Sqlite)
            .to_string();
        let (_, exempt_where) = exempt_sql
            .split_once(" WHERE ")
            .expect("豁免路径必须带 WHERE(主键 + 软删)");
        assert!(
            exempt_where.contains("del_flag"),
            "软删存活条件必须在 WHERE:{exempt_sql}"
        );
        assert!(
            !exempt_where.contains("dept_id")
                && !exempt_where.contains("owner_id"),
            "豁免路径 WHERE 不得有范围条件:{exempt_sql}"
        );

        // 受辖对照:同一构造序列,skip=false 时 owner 条件必须在
        let scoped_sql = ctx
            .core
            .update_query(am(), false)
            .await
            .unwrap()
            .build(sea_orm::DbBackend::Sqlite)
            .to_string();
        assert!(
            scoped_sql.contains("owner_id"),
            "受辖对照必须带 owner 条件:{scoped_sql}"
        );
    }

    #[tokio::test]
    async fn delete_denied_by_scope_is_not_found() {
        use testing::{FixedResolver, seed};
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 99, 2, "0")]).await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });
        let err = ctx.delete::<testing::sandbox::Entity>(1).await.unwrap_err();
        assert!(matches!(err, DataError::NotFound));
        // 行还在
        assert!(
            ds.system_context()
                .find_by_id::<testing::sandbox::Entity>(1)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn unique_conflict_maps_to_conflict() {
        use testing::sandbox_row;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        ctx.insert(sandbox_row(1, Some(10), None)).await.unwrap();
        let err = ctx
            .insert(sandbox_row(1, Some(10), None))
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Conflict));
    }

    // ---- 按条件批量删(仅限豁免实体)----

    /// 关联表(复合主键、全 None 豁免)按条件删:返回受影响行数,0 行是
    /// 合法批量语义(Ok(0)),不是 NotFound。
    #[tokio::test]
    async fn delete_where_deletes_relation_rows_and_returns_count() {
        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        for (u, r) in [(1, 10), (1, 11), (2, 10), (3, 12)] {
            ctx.insert(testing::rel_link::ActiveModel {
                user_id: Set(u),
                role_id: Set(r),
            })
            .await
            .unwrap();
        }

        let n = ctx
            .delete_where::<testing::rel_link::Entity>(
                sea_orm::Condition::all()
                    .add(testing::rel_link::Column::UserId.eq(1)),
            )
            .await
            .unwrap();
        assert_eq!(n, 2, "user_id=1 的两行全删,返回计数");

        // 不匹配的条件 → Ok(0),不是 NotFound(批量语义无 NotFound 约定)
        let n = ctx
            .delete_where::<testing::rel_link::Entity>(
                sea_orm::Condition::all()
                    .add(testing::rel_link::Column::UserId.eq(99)),
            )
            .await
            .unwrap();
        assert_eq!(n, 0, "零行命中是合法批量结果");

        // 其余行不受影响
        let rest = ctx
            .select(
                testing::rel_link::Entity::find()
                    .order_by_asc(testing::rel_link::Column::UserId),
            )
            .without_row_scope()
            .all()
            .await
            .unwrap();
        assert_eq!(
            rest.iter()
                .map(|m| (m.user_id, m.role_id))
                .collect::<Vec<_>>(),
            vec![(2, 10), (3, 12)],
            "条件外的行必须原样保留"
        );
    }

    /// 受辖实体(sandbox:CONSTRAINED=true 且带软删列)必须 fail-closed
    /// 拒绝 —— 防止把行级范围绕成"条件里自己拼"。注意此处触发的是
    /// **第一个守卫**(CONSTRAINED 检查在前,软删守卫到不了);第二个
    /// 守卫的独立覆盖见 `delete_where_on_soft_delete_entity_fails_closed`。
    #[tokio::test]
    async fn delete_where_on_constrained_entity_fails_closed() {
        use testing::{FixedResolver, seed};
        let ds = testing::sqlite_memory().await;
        seed(&ds, vec![(1, 10, 1, "0"), (2, 99, 2, "0")]).await;
        let ds = ds.with_resolver(Arc::new(FixedResolver(
            ScopeFilter::DeptIn(vec![10]),
        )));
        let ctx = ds.context(Principal {
            user_id: 1,
            dept_id: Some(10),
        });

        let err = ctx
            .delete_where::<testing::sandbox::Entity>(
                sea_orm::Condition::all()
                    .add(testing::sandbox::Column::DeptId.eq(10)),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, DataError::Unsupported(_)),
            "受辖实体(CONSTRAINED=true)触发第一个守卫,不得走条件批量删,\
             实际: {err:?}"
        );
        // 行必须原样:拒绝不得产生副作用
        assert!(
            ds.system_context()
                .find_by_id::<testing::sandbox::Entity>(1)
                .await
                .is_ok()
        );
    }

    /// **第二个 fail-closed 守卫的独立覆盖**(fix round 1):CONSTRAINED
    /// = false 但带软删列的实体(soft_dict)—— sandbox 用例到不了这个
    /// 分支(CONSTRAINED 检查在前先拦),而 workspace 真实实体也没有
    /// 该形态,守卫此前零覆盖、与死代码无法区分。批量物理删会绕过
    /// @TableLogic 的存活语义,必须 Unsupported,且守卫在任何 SQL 之前
    /// 短路(种子行原样)。
    #[tokio::test]
    async fn delete_where_on_soft_delete_entity_fails_closed() {
        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        ctx.insert(testing::soft_dict::ActiveModel {
            id: Set(1),
            del_flag: Set("0".into()),
        })
        .await
        .unwrap();

        let err = ctx
            .delete_where::<testing::soft_dict::Entity>(
                sea_orm::Condition::all()
                    .add(testing::soft_dict::Column::DelFlag.eq("0")),
            )
            .await
            .unwrap_err();
        // 文案判别守卫归属:第一个守卫说"受辖实体",第二个说"软删除列" ——
        // 不看文案的话 CONSTRAINED 分支挂了也能蒙混过关(两者同是 Unsupported)。
        let DataError::Unsupported(msg) = err else {
            panic!("软删形态必须 Unsupported,实际: {err:?}")
        };
        assert!(
            msg.contains("软删除列"),
            "必须由第二个守卫(软删)拦截而非 CONSTRAINED 守卫,实际文案: {msg}"
        );
        // 行必须原样:守卫在执行前短路,不得删任何行
        let rows = ctx
            .select(testing::soft_dict::Entity::find())
            .without_row_scope()
            .all()
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "被拒的 delete_where 不得产生副作用");
    }

    /// 事务内的 delete-then-insert 模式(user_service 的关联表更新路径):
    /// TxnContext 同样暴露 delete_where;闭包 Err → 整体回滚,插入不落库。
    #[tokio::test]
    async fn delete_where_inside_transaction_rolls_back_with_inserts() {
        use crate::sea_orm::ActiveValue::Set;
        let ds = testing::sqlite_memory().await;
        let ctx = ds.system_context();
        for r in [10, 11] {
            ctx.insert(testing::rel_link::ActiveModel {
                user_id: Set(7),
                role_id: Set(r),
            })
            .await
            .unwrap();
        }

        // Err 分支:先删后插再报错 → 删与插都不落库
        let _ = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    tx.delete_where::<testing::rel_link::Entity>(
                        sea_orm::Condition::all()
                            .add(testing::rel_link::Column::UserId.eq(7)),
                    )
                    .await?;
                    tx.insert(testing::rel_link::ActiveModel {
                        user_id: Set(7),
                        role_id: Set(12),
                    })
                    .await?;
                    Err::<(), DataError>(DataError::NotFound)
                })
            })
            .await;

        let rest = ctx
            .select(testing::rel_link::Entity::find())
            .without_row_scope()
            .all()
            .await
            .unwrap();
        assert_eq!(
            rest.iter().map(|m| m.role_id).collect::<Vec<_>>(),
            vec![10, 11],
            "回滚必须同时撤销 delete_where 与随后 insert"
        );
    }
}
