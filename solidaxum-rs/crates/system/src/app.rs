use std::sync::Arc;

use app_core::{
    config::CodegenConfig, infra::Infrastructure,
    serde::encrypt_field::FieldEncryptor,
};

use crate::{
    client::service::client_service::ClientService,
    codegen::service::{
        code_gen::CodeGenEngine, gen_table_service::GenTableService,
    },
    config::service::config_service::ConfigService,
    dept::{
        repository::dept_repo::DeptRepository,
        service::dept_service::DeptService,
    },
    dict::service::{
        dict_data_service::DictDataService, dict_type_service::DictTypeService,
    },
    menu::{
        repository::menu_repo::MenuRepository,
        service::menu_service::MenuService,
    },
    message::service::message_service::MessageService,
    notice::service::notice_service::NoticeService,
    oss::service::oss_service::OssService,
    permission::{
        scope_resolver::{
            CachedScopeResolver, ScopeCacheSlot, empty_scope_cache_slot,
        },
        service::PermissionService,
    },
    post::service::post_service::PostService,
    profile::service::profile_service::ProfileService,
    push::sse::PushHub,
    role::service::role_service::RoleService,
    social::service::social_service::SocialService,
    token::token_service::TokenService,
    translation::service::translation_service::TranslationService,
    user::service::user_service::UserService,
};

#[derive(Clone)]
pub struct AppState {
    pub infra: Infrastructure,
    pub services: ServiceContainer,
}

#[derive(Clone)]
pub struct ServiceContainer {
    pub user_service: UserService,
    pub role_service: RoleService,
    pub client_service: ClientService,
    pub menu_repo: MenuRepository,
    pub menu_service: MenuService,
    pub dept_repo: DeptRepository,
    pub dept_service: DeptService,
    pub post_service: PostService,
    pub notice_service: NoticeService,
    pub permission_service: PermissionService,
    pub profile_service: ProfileService,
    pub token_service: TokenService,
    pub dict_type_service: DictTypeService,
    pub dict_data_service: DictDataService,
    pub sys_config_service: ConfigService,
    pub translation_service: TranslationService,
    pub push_hub: PushHub,
    pub oss_service: OssService,
    pub gen_table_service: GenTableService,
    pub code_gen_engine: CodeGenEngine,
    pub social_service: SocialService,
    pub message_service: MessageService,
    /// 数据范围缓存(60s TTL,user_id 键)的失效槽。私有字段 + 后补注入:
    /// resolver 构造依赖本容器里的 PermissionService(构造环,同
    /// `Infrastructure::ds` 的理由,见 core/src/infra/mod.rs),容器构造时
    /// 它还不存在 —— 先发空槽给 role/user 服务(失效 no-op),装配层
    /// (admin/app.rs)建好 resolver 后经 [`Self::set_scope_cache`] 一次
    /// 性后补,所有持有者即刻可见。`Clone` 携带同一格槽。
    scope_cache: ScopeCacheSlot,
}

impl ServiceContainer {
    pub fn new(
        infra: &Infrastructure,
        aes_key: Option<String>,
        codegen: &CodegenConfig,
    ) -> anyhow::Result<Self> {
        // 先造空槽发给 role/user/dept 服务(它们是失效调用点),
        // 装配层再后补真句柄 —— 服务与容器必须共享同一格槽,set 一次全体可见。
        let scope_cache = empty_scope_cache_slot();
        Ok(Self {
            user_service: UserService::new(infra)
                .with_scope_cache(scope_cache.clone()),
            role_service: RoleService::new(infra)
                .with_scope_cache(scope_cache.clone()),
            client_service: ClientService::new(infra),
            menu_repo: MenuRepository::new(),
            menu_service: MenuService::new(infra),
            dept_repo: DeptRepository::new(),
            // dept 写路径也接失效槽 —— 部门树变更改变
            // DeptAndBelow 范围条目内嵌的子树展开。
            dept_service: DeptService::new(infra)
                .with_scope_cache(scope_cache.clone()),
            post_service: PostService::new(infra),
            notice_service: NoticeService::new(infra),
            permission_service: PermissionService::new(infra),
            profile_service: ProfileService::new(infra),
            token_service: TokenService::new(infra),
            dict_type_service: DictTypeService::new(infra),
            dict_data_service: DictDataService::new(infra),
            sys_config_service: ConfigService::new(infra),
            translation_service: TranslationService::new(infra),
            push_hub: PushHub::new(),
            oss_service: {
                let encryptor = match aes_key {
                    Some(ref key) if !key.is_empty() => {
                        use base64::{
                            Engine, engine::general_purpose::STANDARD,
                        };
                        match STANDARD.decode(key) {
                            Ok(bytes) if bytes.len() == 32 => {
                                let mut arr = [0u8; 32];
                                arr.copy_from_slice(&bytes);
                                Some(
                                    Arc::new(
                                        app_core::serde::encrypt_field::AesFieldEncryptor::new(
                                            arr, "ENC:",
                                        ),
                                    ) as Arc<dyn FieldEncryptor>,
                                )
                            }
                            Ok(bytes) => {
                                tracing::error!(
                                    "security.aes_key 解码后长度 {} 不是 32 字节，OSS 字段加密已禁用",
                                    bytes.len()
                                );
                                None
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "security.aes_key base64 解码失败，OSS 字段加密已禁用"
                                );
                                None
                            }
                        }
                    }
                    _ => None,
                };
                OssService::with_encryptor(infra, encryptor)
            },
            gen_table_service: GenTableService::new(
                // 元数据读取经 app-data 漏斗(设计 6.3 meta 行)。此处恒为
                // 装配后捕获:装配层(admin/app.rs)必先
                // set_data_source(带 resolver 的派生 ds)再构造容器,未装配
                // 即 panic,已无"降级快照"分支。该服务全豁免读(gen_table/
                // gen_column 落 CONSTRAINED=false,漏斗不解析 resolver),
                // 见 GenTableService.ds 的注释。
                infra.data_source(),
                codegen.package_name.clone(),
                codegen.frontend_type.clone(),
                codegen.author.clone(),
            ),
            code_gen_engine: CodeGenEngine::new().map_err(|e| {
                anyhow::anyhow!("Failed to init code gen engine: {:?}", e)
            })?,
            social_service: SocialService::new(infra),
            message_service: MessageService::new(infra),
            scope_cache,
        })
    }

    /// 装配期一次性注入数据范围缓存句柄(OnceLock:首次成功,之后恒
    /// false,形状对齐 `Infrastructure::set_data_source`)。返回 bool 而
    /// 非 panic:被拒只说明重复装配(编程错误),首次句柄仍在生效,炸
    /// 进程不如让启动日志可见。生产装配见 admin/app.rs —— 同一份
    /// resolver **双投**:DataSource(行级范围解析)+ 本槽(写路径失效),
    /// 两本缓存必须是同一个,否则失效只清其一。
    pub fn set_scope_cache(&self, resolver: Arc<CachedScopeResolver>) -> bool {
        self.scope_cache.set(resolver).is_ok()
    }
}
