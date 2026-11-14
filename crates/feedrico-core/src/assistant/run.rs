//! Run one Assistant [`Task`] (#113).
//!
//! Contract: `run` spawns the found Assistant's CLI in a fresh empty temporary directory with
//! its hardened flags, writes the prompt to stdin, streams [`Event`]s to `on_event`, and returns
//! the final text. It stops on `cancel`, killing the process tree, or after `task.timeout`, and
//! maps failures to [`AssistantError`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{AssistantError, Event, FoundAssistant, Task};

/// Cancels a running Task. Clone it and keep it, then call [`Cancel::cancel`] from anywhere.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Run `task` on `assistant`. On success it returns the [`Event::Done`] payload, which has also
/// been sent to `on_event`.
pub async fn run(
    _assistant: &FoundAssistant,
    _task: &Task,
    _cancel: Cancel,
    _on_event: impl FnMut(Event) + Send,
) -> Result<(String, Option<serde_json::Value>), AssistantError> {
    Err(AssistantError::NotInstalled) // #113 implements this.
}
