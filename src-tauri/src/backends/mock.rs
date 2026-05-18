use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;

use super::{ChatContext, ChatMessage, LLMBackend};

pub struct MockBackend;

#[async_trait]
impl LLMBackend for MockBackend {
    fn id(&self) -> &'static str {
        "mock"
    }

    fn label(&self) -> &'static str {
        "Mock (offline, canned replies)"
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn send(
        &self,
        messages: Vec<ChatMessage>,
        context: Option<ChatContext>,
    ) -> Result<String, String> {
        sleep(Duration::from_millis(450)).await;

        let user_msg = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");

        let snippet = context
            .as_ref()
            .and_then(|c| c.selection.as_deref().or(c.focused_text.as_deref()))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let shared_screen = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| !m.images.is_empty())
            .unwrap_or(false);

        let reply = if shared_screen {
            "I can see you shared your screen — but on the Mock backend I can't actually look at it. Switch to Ollama with a vision model (llama3.2-vision) or Claude, and I'll really see it.".to_string()
        } else if let Some(snip) = snippet {
            let preview: String = snip.chars().take(80).collect();
            format!(
                "Looking at this with you. About \"{}\"{}\n\n(Mock reply: pick Ollama in settings for a real local model, or wire Claude CLI for the production path.)",
                preview,
                if snip.len() > 80 { "…" } else { "" }
            )
        } else if user_msg.trim().is_empty() {
            "Hey — I'm here. Drop a line of code on me or just say what you're poking at.".to_string()
        } else {
            format!(
                "Heard you: \"{}\". I'm running on the Mock backend so I can't actually think — switch to Ollama in settings for real replies.",
                user_msg.trim()
            )
        };

        Ok(reply)
    }
}
