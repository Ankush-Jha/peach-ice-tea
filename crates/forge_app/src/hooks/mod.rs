mod compaction;
mod doom_loop;
mod notes;
mod pending_todos;
mod runtime_verify_gate;
mod telemetry;
mod title_generation;
mod tracing;
mod verify_gate;

pub use compaction::CompactionHandler;
pub use doom_loop::DoomLoopDetector;
pub use notes::{NotesHandler, enabled as notes_enabled};
pub use pending_todos::PendingTodosHandler;
pub use runtime_verify_gate::RuntimeVerifyGateHandler;
pub use telemetry::{TelemetryHandler, record_model_retry};
pub use title_generation::TitleGenerationHandler;
pub use tracing::TracingHandler;
pub use verify_gate::{VerifyGateHandler, observe as observe_for_verification};
