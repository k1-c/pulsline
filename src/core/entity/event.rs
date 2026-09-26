//! One line of the spool: something that happened, recorded as it happened.
//!
//! The spool is the source of truth, and its format is a contract with every
//! program that writes or reads it (docs/data-format.md). Fields are only
//! added; a change an older reader would misread bumps [`VERSION`].

use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::core::entity::ids::{AgentSessionId, EventId};

/// The spool format this build writes, and the newest it reads.
pub const VERSION: u32 = 1;

/// Something that happened, as one spool line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The spool format the line was written in.
    pub v: u32,
    pub id: EventId,
    /// When it happened.
    pub at: Timestamp,
    pub source: Source,
    pub kind: Kind,
    /// The agent session it happened in, when it came from an agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<AgentSessionId>,
    /// The directory the agent was working in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    /// Where the repository at `cwd` stood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitContext>,
    /// What the kind says about it: a tool's name and the file it touched,
    /// the first words of a prompt. Shaped by `kind`.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
}

/// Where an event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    ClaudeCode,
    Codex,
    Github,
    Linear,
    /// A source a newer Pulsline writes and this one does not know.
    #[serde(other)]
    Unknown,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    #[serde(rename = "agent.session.started")]
    AgentSessionStarted,
    #[serde(rename = "agent.prompt")]
    AgentPrompt,
    #[serde(rename = "agent.tool")]
    AgentTool,
    #[serde(rename = "agent.turn.ended")]
    AgentTurnEnded,
    #[serde(rename = "agent.compacted")]
    AgentCompacted,
    #[serde(rename = "agent.session.ended")]
    AgentSessionEnded,
    /// A kind a newer Pulsline writes and this one does not know.
    #[serde(other)]
    Unknown,
}

/// Where a repository stood when an event happened in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
}

impl Event {
    /// The event as one spool line, without the newline.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("an event always serializes")
    }
}
