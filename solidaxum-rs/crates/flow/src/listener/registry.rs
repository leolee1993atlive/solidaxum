use std::{collections::HashMap, sync::Arc};

use app_core::error::AppResult;
use async_trait::async_trait;

use super::types::ListenerVariable;

/// 监听器 trait：后端通过注册名绑定具体实现
#[async_trait]
pub trait Listener: Send + Sync {
    /// listener_type: "start" | "assignment" | "finish" | "create"
    /// params: listener_path 里 `name({json})` 的声明式参数(无参为 None)
    async fn notify(
        &self,
        listener_type: &str,
        params: Option<&serde_json::Value>,
        var: &mut ListenerVariable,
    ) -> AppResult<()>;
}

/// 监听器注册表（名称 → 实现）
#[derive(Clone, Default)]
pub struct ListenerRegistry {
    listeners: HashMap<String, Arc<dyn Listener>>,
}

impl ListenerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        listener: Arc<dyn Listener>,
    ) {
        self.listeners.insert(name.into(), listener);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Listener>> {
        self.listeners.get(name).cloned()
    }

    /// 返回所有注册的监听器名称（供设计器下拉框显示）
    pub fn names(&self) -> Vec<&str> {
        self.listeners.keys().map(String::as_str).collect()
    }
}
