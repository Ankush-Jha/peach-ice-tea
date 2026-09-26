use std::path::PathBuf;

use derive_setters::Setters;
use peach_api::{ConversationId, Environment};

//TODO: UIState and PeachPrompt seem like the same thing and can be merged
/// State information for the UI
#[derive(Debug, Default, Clone, Setters)]
#[setters(strip_option)]
pub struct UIState {
    pub cwd: PathBuf,
    pub conversation_id: Option<ConversationId>,
    // harness: R-PROTO-7 — set by `exec`. Suppresses the interactive
    // "continue anyway?" prompt on interrupt (there is nobody to answer it)
    // and records why the task stopped so the exit code can reflect it.
    pub non_interactive: bool,
    pub interruption: Option<peach_domain::InterruptionReason>,
}

impl UIState {
    pub fn new(env: Environment) -> Self {
        Self {
            cwd: env.cwd,
            conversation_id: Default::default(),
            non_interactive: false,
            interruption: None,
        }
    }
}
