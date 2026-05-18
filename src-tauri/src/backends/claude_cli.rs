use async_trait::async_trait;
use base64::Engine;
use tokio::process::Command;

use super::{ChatContext, ChatMessage, LLMBackend};

pub struct ClaudeCliBackend;

#[async_trait]
impl LLMBackend for ClaudeCliBackend {
    fn id(&self) -> &'static str {
        "claude-cli"
    }

    fn label(&self) -> &'static str {
        "Claude Code CLI (uses your subscription)"
    }

    async fn is_available(&self) -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn send(
        &self,
        messages: Vec<ChatMessage>,
        context: Option<ChatContext>,
    ) -> Result<String, String> {
        let mut prompt = String::new();

        // Personality voice (and any other system messages) frame the prompt.
        for m in &messages {
            if m.role == "system" {
                prompt.push_str(&m.content);
                prompt.push_str("\n\n");
            }
        }

        // Attached images (e.g. a screen capture) — the claude CLI can't take
        // image bytes inline, so write each to a temp PNG and point Claude at
        // the path; it reads local image files referenced in the prompt.
        let mut img_idx = 0;
        for m in &messages {
            for img in &m.images {
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(img) {
                    let path = std::env::temp_dir()
                        .join(format!("claudio-shot-{}-{}.png", std::process::id(), img_idx));
                    img_idx += 1;
                    if std::fs::write(&path, &bytes).is_ok() {
                        prompt.push_str(&format!(
                            "The user shared a screenshot — read this image file to see it: {}\n\n",
                            path.display()
                        ));
                    }
                }
            }
        }

        // Context block — what the user is looking at.
        if let Some(ctx) = context.as_ref() {
            let before = prompt.len();
            if let Some(app) = &ctx.focused_app {
                prompt.push_str(&format!("(I'm looking at: {})\n", app));
            }
            if let Some(sel) = &ctx.selection {
                prompt.push_str(&format!("Selected code:\n```\n{}\n```\n", sel));
            } else if let Some(text) = &ctx.focused_text {
                prompt.push_str(&format!("Nearby code:\n```\n{}\n```\n", text));
            }
            if prompt.len() > before {
                prompt.push('\n');
            }
        }

        for m in &messages {
            if m.role == "user" {
                prompt.push_str(&m.content);
                prompt.push('\n');
            }
        }

        let output = Command::new("claude")
            .arg("-p")
            .arg(&prompt)
            .output()
            .await
            .map_err(|e| format!("Couldn't invoke `claude` CLI: {e}. Is it on PATH?"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("claude CLI failed: {}", stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
