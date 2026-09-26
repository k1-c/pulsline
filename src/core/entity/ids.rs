//! Typed ids, so an event's id and an agent session's id cannot be swapped.

use std::fmt;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// An event's id: a ULID, so ids sort by the time they were made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub Ulid);

impl EventId {
    /// An id for an event made at `millis` since the Unix epoch, with
    /// `random` for the rest. The caller supplies both, so the core stays
    /// free of clocks and randomness.
    pub fn from_parts(millis: u64, random: u128) -> Self {
        Self(Ulid::from_parts(millis, random))
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A coding agent's session, as the agent names it (Claude Code's
/// `session_id`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentSessionId(pub String);

impl fmt::Display for AgentSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
