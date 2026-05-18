use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod claude_cli;
pub mod mock;
pub mod ollama;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    /// Base64-encoded images attached to this message (e.g. a screen capture).
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatContext {
    pub focused_app: Option<String>,
    pub focused_text: Option<String>,
    pub selection: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackendInfo {
    pub id: String,
    pub label: String,
    pub available: bool,
}

#[async_trait]
pub trait LLMBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    async fn is_available(&self) -> bool;
    async fn send(
        &self,
        messages: Vec<ChatMessage>,
        context: Option<ChatContext>,
    ) -> Result<String, String>;
}

pub struct BackendRegistry {
    backends: HashMap<String, Arc<dyn LLMBackend>>,
    order: Vec<String>,
    current: RwLock<String>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        let backends_list: Vec<Arc<dyn LLMBackend>> = vec![
            Arc::new(mock::MockBackend),
            Arc::new(ollama::OllamaBackend::default()),
            Arc::new(claude_cli::ClaudeCliBackend),
        ];
        let mut backends = HashMap::new();
        let mut order = Vec::new();
        for b in backends_list {
            order.push(b.id().to_string());
            backends.insert(b.id().to_string(), b);
        }
        Self {
            backends,
            order,
            current: RwLock::new("mock".to_string()),
        }
    }

    pub async fn current(&self) -> Arc<dyn LLMBackend> {
        let id = self.current.read().await.clone();
        self.backends
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.backends.get("mock").expect("mock backend").clone())
    }

    pub async fn current_id(&self) -> String {
        self.current.read().await.clone()
    }

    pub async fn set_current(&self, id: &str) -> Result<(), String> {
        if !self.backends.contains_key(id) {
            return Err(format!("unknown backend: {id}"));
        }
        *self.current.write().await = id.to_string();
        Ok(())
    }

    pub async fn list(&self) -> Vec<BackendInfo> {
        let mut out = Vec::new();
        for id in &self.order {
            if let Some(b) = self.backends.get(id) {
                out.push(BackendInfo {
                    id: b.id().to_string(),
                    label: b.label().to_string(),
                    available: b.is_available().await,
                });
            }
        }
        out
    }
}
