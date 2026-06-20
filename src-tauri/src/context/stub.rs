use crate::backends::ChatContext;
use super::AccessibilityStatus;

pub fn read_focused_context() -> ChatContext {
    ChatContext::default()
}

pub fn read_context_for_pid(_pid: i32) -> ChatContext {
    ChatContext::default()
}

pub fn check_accessibility(_prompt: bool) -> AccessibilityStatus {
    AccessibilityStatus::NotApplicable
}
