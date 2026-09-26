//! What Pulsline is about, one file per aggregate.
//!
//! - [`ids`] — the typed ids.
//! - [`event`] — one line of the spool: something that happened, recorded as
//!   it happened. Its format is a contract with the hook, the MCP server, and
//!   any program reading the spool.

pub mod event;
pub mod ids;

pub use event::{Event, GitContext, Kind, Source};
pub use ids::{AgentSessionId, EventId};
