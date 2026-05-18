use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{ChatContext, ChatMessage, LLMBackend};

pub struct OllamaBackend {
    endpoint: String,
    model: String,
}

impl Default for OllamaBackend {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:11434".to_string(),
            model: "llama3.2".to_string(),
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
    /// Base64 images for vision models. Omitted entirely when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: Option<OllamaResponseMessage>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

#[async_trait]
impl LLMBackend for OllamaBackend {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn label(&self) -> &'static str {
        "Ollama (local, free)"
    }

    async fn is_available(&self) -> bool {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_millis(400))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        client
            .get(format!("{}/api/tags", self.endpoint))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn send(
        &self,
        messages: Vec<ChatMessage>,
        context: Option<ChatContext>,
    ) -> Result<String, String> {
        let mut prepared: Vec<ChatMessage> = Vec::new();

        // The frontend sends Claudio's active personality as the leading
        // system message — keep it (and any other leading system messages)
        // at the very front so it frames the whole exchange.
        let mut convo = messages.into_iter().peekable();
        while convo.peek().map(|m| m.role == "system").unwrap_or(false) {
            prepared.push(convo.next().unwrap());
        }

        // Context block — what the user is looking at — as its own system message.
        if let Some(ctx) = context.as_ref() {
            let mut ctx_block = String::new();
            if let Some(app) = &ctx.focused_app {
                ctx_block.push_str(&format!("App: {}\n", app));
            }
            if let Some(sel) = &ctx.selection {
                ctx_block.push_str(&format!("Selected:\n```\n{}\n```\n", sel));
            } else if let Some(text) = &ctx.focused_text {
                ctx_block.push_str(&format!("Nearby code:\n```\n{}\n```\n", text));
            }
            if !ctx_block.is_empty() {
                prepared.push(ChatMessage {
                    role: "system".into(),
                    content: format!("Context the user is looking at right now:\n{}", ctx_block),
                    ..Default::default()
                });
            }
        }

        // The rest of the conversation (user/assistant turns).
        prepared.extend(convo);

        let req_messages: Vec<OllamaMessage> = prepared
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.as_str(),
                content: m.content.as_str(),
                images: m.images.clone(),
            })
            .collect();

        let body = OllamaRequest {
            model: &self.model,
            messages: req_messages,
            stream: false,
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| format!("client build failed: {e}"))?;

        let resp = client
            .post(format!("{}/api/chat", self.endpoint))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama unreachable at {}. Is `ollama serve` running? ({e})", self.endpoint))?;

        if !resp.status().is_success() {
            return Err(format!("Ollama returned {}", resp.status()));
        }

        let parsed: OllamaResponse = resp
            .json()
            .await
            .map_err(|e| format!("bad Ollama response: {e}"))?;

        Ok(parsed
            .message
            .map(|m| m.content)
            .unwrap_or_else(|| "(empty response)".to_string()))
    }
}
