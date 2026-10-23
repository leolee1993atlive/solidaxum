//! dict 仓储集成测试(全豁免)。
//!
//! 覆盖三组语义:
//! 1. 基础读/写路径(经 `system_context` 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    所有读方法照见全部行 —— SysDictTypeMapper/SysDictDataMapper 无
//!    @DataPermission,字典是全局定义(漏判的反向钉死);
//! 3. **级联与守卫**:rename 级联
//!    `update_dict_type_in_txn`(逐行换轨,dept reparent 同款)与
//!    删除守卫 `exists_by_type`(Java deleteDictTypeByIds 的 exists)。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::{
    middleware::extractors::CurrentUser, model::login_user::LoginUser,
    vo::page::PageQuery,
};
use app_data::{
    DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::setup_db;
use system::dict::{
    model::{
        dict_data::{DictData, DictDataCreateBo, DictDataPageQuery},
        dict_type::{DictType, DictTypeCreateBo, DictTypePageQuery},
    },
    repository::{
        dict_data_repo::DictDataRepository,
        dict_type_repo::{self as dict_type_entity, DictTypeRepository},
    },
};

fn test_user() -> CurrentUser {
    CurrentUser(LoginUser {
        user_id: 1,
        dept_id: Some(100),
        username: "test".to_string(),
        nick_name: Some("测试".to_string()),
        user_type: Some("sys_user".to_string()),
        client_id: "client".to_string(),
        client_key: None,
        device_type: None,
        permissions: HashSet::new(),
        roles: HashSet::new(),
        login_ip: None,
        login_date: None,
    })
}

/// 仓库 + 无 resolver 的 DataSource(system_context 语义,ScopeFilter 恒 All)。
fn repo_system(
    ds: DataSource,
) -> (Arc<DataSource>, DictTypeRepository, DictDataRepository) {
    let ds = Arc::new(ds);
    (
        ds.clone(),
        DictTypeRepository::new(ds.clone()),
        DictDataRepository::new(ds),
    )
}

/// 仓库 + 固定 ScopeFilter 的 DataSource(范围用例,免搭业务角色语义)。
fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, DictTypeRepository, DictDataRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (
        ds.clone(),
        DictTypeRepository::new(ds.clone()),
        DictDataRepository::new(ds),
    )
}

fn empty_type_query() -> DictTypePageQuery {
    DictTypePageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        dict_name: None,
        dict_type: None,
    }
}

fn empty_data_query() -> DictDataPageQuery {
    DictDataPageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        dict_type: None,
        dict_label: None,
    }
}

// ====================== DictType ======================

#[tokio::test]
async fn test_dict_type_select_by_type() {
    let (ds, repo, _) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let dt = repo
        .select_by_type(&ctx, "sys_normal_disable")
        .await
        .unwrap()
        .expect("字典类型不存在");
    assert_eq!(dt.dict_name, "系统开关");
}

#[tokio::test]
async fn test_dict_type_select_page() {
    let (ds, repo, _) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let page = repo
        .select_page(&ctx, &empty_type_query(), 1, 10, "dict_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 3);
}

#[tokio::test]
async fn test_dict_type_insert_and_delete() {
    let (ds, repo, _) = repo_system(setup_db().await);
    let ctx = ds.system_context();
    let user = test_user();

    let bo = DictTypeCreateBo {
        dict_name: "测试字典".to_string(),
        dict_type: "test_dict".to_string(),
        remark: Some("测试".to_string()),
    };
    let dt = DictType::from_create_bo(bo, &user);
    let id = dt.dict_id.unwrap();

    repo.insert(&ctx, &dt).await.unwrap();
    assert!(repo.select_by_id(&ctx, &id).await.unwrap().is_some());

    repo.delete_by_id(&ctx, &id).await.unwrap();
    assert!(repo.select_by_id(&ctx, &id).await.unwrap().is_none());
}

// ====================== DictData ======================

#[tokio::test]
async fn test_dict_data_select_by_type() {
    let (ds, _, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let list = repo
        .select_by_type(&ctx, "sys_normal_disable")
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].dict_label, "正常");
    assert_eq!(list[0].dict_sort, 1);
    assert_eq!(list[1].dict_label, "停用");
    assert_eq!(list[1].dict_sort, 2);
}

#[tokio::test]
async fn test_dict_data_select_page() {
    let (ds, _, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let query = DictDataPageQuery {
        dict_type: Some("sys_user_gender".to_string()),
        ..empty_data_query()
    };

    let page = repo
        .select_page(&ctx, &query, 1, 10, "dict_sort asc")
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.rows.len(), 2);
}

#[tokio::test]
async fn test_dict_data_insert_and_delete() {
    let (ds, _, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();
    let user = test_user();

    let bo = DictDataCreateBo {
        dict_sort: Some(0),
        dict_label: "测试".to_string(),
        dict_value: "test_val".to_string(),
        dict_type: "test_type".to_string(),
        css_class: None,
        list_class: None,
        is_default: None,
        remark: None,
    };
    let dd = DictData::from_create_bo(bo, &user);
    let id = dd.dict_code.unwrap();

    repo.insert(&ctx, &dd).await.unwrap();
    assert!(repo.select_by_id(&ctx, &id).await.unwrap().is_some());

    repo.delete_by_id(&ctx, &id).await.unwrap();
    assert!(repo.select_by_id(&ctx, &id).await.unwrap().is_none());
}

// ─── 全豁免判例 + 级联/守卫 ───

/// 范围用例的补充种子:4(create_dept=101)与 5(create_dept=102)——
/// 若字典被误施加行级范围,DeptIn([101]) 下 5 号行与 NULL 种子行会消失。
/// 免盖章种子:显式审计值原样落库(insert_unstamped)。
async fn seed_scope_dicts(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, name, dict_type, create_dept, create_by) in [
        (4, "本部门字典", "scoped_same_dept", 101, 7),
        (5, "越权部门字典", "scoped_other_dept", 102, 8),
    ] {
        ctx.insert_unstamped(dict_type_entity::ActiveModel {
            dict_id: Set(id),
            dict_name: Set(name.into()),
            dict_type: Set(dict_type.into()),
            create_dept: Set(Some(create_dept)),
            create_by: Set(Some(create_by)),
            ..Default::default()
        })
        .await
        .expect("种范围字典行失败");
    }
}

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,字典读必须照见全部行
/// (含 create_dept=102 的越权行与 create_dept=NULL 的种子行)——
/// SysDictTypeMapper/SysDictDataMapper 无 @DataPermission,字典数据
/// 非行级;若任何方法漏标 `without_row_scope`,CONSTRAINED=false 的驱动
/// 会 fail-closed 报 Unsupported,或(误加范围列时)静默砍行,两者都会
/// 在此变红。
#[tokio::test]
async fn exempt_dict_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_dicts(&ds).await;
    let (ds, type_repo, data_repo) =
        repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // select_all:种子 3 行 + 补充 2 行(4 同部门 / 5 越权部门)全见
    let types = type_repo.select_all(&ctx).await.unwrap();
    assert_eq!(types.len(), 5, "字典类型全量读不得被部门范围收窄(全豁免)");

    // select_by_type / select_by_id:越权行照见
    assert!(
        type_repo
            .select_by_type(&ctx, "scoped_other_dept")
            .await
            .unwrap()
            .is_some(),
        "越权部门的字典类型对唯一性读照常可见"
    );
    assert!(
        type_repo.select_by_id(&ctx, &5).await.unwrap().is_some(),
        "越权部门的字典类型对详情读照常可见"
    );

    // select_page:total 不被范围砍小
    let page = type_repo
        .select_page(&ctx, &empty_type_query(), 1, 10, "dict_id asc")
        .await
        .unwrap();
    assert_eq!(page.total, 5, "分页 total 须是全量计数(全豁免)");

    // dict_data 同口径:select_by_type / select_page / exists 全豁免
    let datas = data_repo
        .select_by_type(&ctx, "sys_normal_disable")
        .await
        .unwrap();
    assert_eq!(datas.len(), 2, "字典数据读不得被部门范围收窄");
    let data_page = data_repo
        .select_page(&ctx, &empty_data_query(), 1, 10, "dict_sort asc")
        .await
        .unwrap();
    assert_eq!(data_page.total, 6, "字典数据分页 total 须是全量计数");
    assert!(
        data_repo
            .exists_by_type_and_value(&ctx, "sys_normal_disable", "0", None)
            .await
            .unwrap(),
        "唯一性校验不得被范围砍小"
    );
}

/// **rename 级联**:dict_type 改名时 sys_dict_data.dict_type 逐行跟随
/// (Java updateDictType 的 lambda update,ServiceImpl:192-196;@Transactional
/// 平价 —— 事务闭包内 cascade + 自身 update,dept reparent 同款)。
#[tokio::test]
async fn cascade_update_dict_type_in_txn_renames_data_rows() {
    let (ds, type_repo, data_repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // sys_normal_disable 下有 2 行 data(种子 1/2)
    assert!(
        data_repo
            .exists_by_type(&ctx, "sys_normal_disable")
            .await
            .unwrap(),
        "改名前应存在引用"
    );

    // 事务闭包:cascade + 自身 update 同事务(读在闭包外完成,
    // 闭包内只做 tx 面)
    let mut renamed = type_repo
        .select_by_id(&ctx, &1)
        .await
        .unwrap()
        .expect("种子 1 应存在");
    renamed.dict_type = "sys_normal_disabled_v2".to_string();
    ctx.transaction(|tx| {
        Box::pin(async move {
            DictDataRepository::update_dict_type_in_txn(
                tx,
                "sys_normal_disable",
                "sys_normal_disabled_v2",
            )
            .await?;
            DictTypeRepository::update_by_id_in_txn(tx, &renamed).await
        })
    })
    .await
    .expect("级联改名失败");

    // 旧类型零引用、新类型接管 2 行、update_by 被 principal 盖章
    assert!(
        !data_repo
            .exists_by_type(&ctx, "sys_normal_disable")
            .await
            .unwrap(),
        "改名后旧 dict_type 应零引用"
    );
    let datas = data_repo
        .select_by_type(&ctx, "sys_normal_disabled_v2")
        .await
        .unwrap();
    assert_eq!(datas.len(), 2, "2 行字典数据应跟随改名");
    assert_eq!(
        datas[0].base_entity.update_by,
        Some(7),
        "级联行的 update_by = principal(漏斗盖章)"
    );
    let renamed = type_repo
        .select_by_id(&ctx, &1)
        .await
        .unwrap()
        .expect("改名后应可读回");
    assert_eq!(renamed.dict_type, "sys_normal_disabled_v2");
    assert_eq!(
        renamed.base_entity.update_by,
        Some(7),
        "自身 update 的 update_by = principal"
    );
}

/// **删除守卫**:exists_by_type(Java deleteDictTypeByIds 的
/// `dictDataMapper.lambda().eq(...).exists()`,ServiceImpl:147)。
#[tokio::test]
async fn exists_by_type_guards_delete() {
    let (ds, _, data_repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    assert!(
        data_repo
            .exists_by_type(&ctx, "sys_normal_disable")
            .await
            .unwrap()
    );
    assert!(
        !data_repo
            .exists_by_type(&ctx, "no_such_type")
            .await
            .unwrap()
    );
}
