use std::{collections::HashMap, sync::Arc};

#[async_trait::async_trait]
pub trait JobHandler: Send + Sync {
    async fn execute(&self, params: &str) -> Result<String, String>;
}

pub struct JobRegistry {
    handlers: HashMap<String, Arc<dyn JobHandler>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: Arc<dyn JobHandler>,
    ) {
        self.handlers.insert(name.into(), handler);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn JobHandler>> {
        self.handlers.get(name).cloned()
    }

    pub fn names(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoHandler;

    #[async_trait::async_trait]
    impl JobHandler for EchoHandler {
        async fn execute(&self, params: &str) -> Result<String, String> {
            Ok(format!("echo:{}", params))
        }
    }

    #[tokio::test]
    async fn test_registry_get_and_execute() {
        let mut registry = JobRegistry::new();
        registry.register("echo", std::sync::Arc::new(EchoHandler));

        let handler = registry.get("echo").expect("handler should exist");
        let result = handler.execute("hello").await.expect("should succeed");
        assert_eq!(result, "echo:hello");

        assert!(registry.get("missing").is_none());
        assert!(registry.names().contains(&"echo"));
    }
}
