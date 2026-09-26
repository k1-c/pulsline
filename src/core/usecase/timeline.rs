//! The timeline: what the spool holds, indexed so a day can be read back.
//!
//! [`admit`] decides what one spool line becomes; [`catch_up`] brings the
//! timeline up to date with the spool, and [`rebuild`] builds it again from
//! the start.

use crate::core::entity::event::{Event, VERSION};

/// What the timeline use cases ask of the spool and the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Take in the lines written since the last catch-up.
    CatchUp,
    /// Drop what was taken in and take in every line again.
    Rebuild,
}

/// What became of one spool line.
#[derive(Debug, Clone, PartialEq)]
pub enum Admission {
    /// The line is an event, and the timeline takes it.
    Taken(Box<Event>),
    /// The line is an event in a format newer than this build reads.
    Newer { version: u32 },
    /// The line is not an event.
    Malformed { reason: String },
    /// The line holds nothing.
    Blank,
}

/// How many lines a catch-up or rebuild took in, and how many it set aside.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub taken: usize,
    pub newer: usize,
    pub malformed: usize,
}

impl Tally {
    /// Counts one line's admission.
    pub fn count(&mut self, admission: &Admission) {
        match admission {
            Admission::Taken(_) => self.taken += 1,
            Admission::Newer { .. } => self.newer += 1,
            Admission::Malformed { .. } => self.malformed += 1,
            Admission::Blank => {}
        }
    }
}

/// **Take a spool line into the timeline.**
///
/// A line written in this build's format, or an older one, is taken as its
/// event. A line from a newer Pulsline is set aside, not refused: another
/// install may share the spool, and a later upgrade reads it on rebuild. A
/// line that is not an event is set aside with the reason, so one bad line
/// never stops the rest. An empty line is passed over.
pub fn admit(line: &str) -> Admission {
    let line = line.trim();
    if line.is_empty() {
        return Admission::Blank;
    }
    // The version first, so a newer line is recognised even when the rest
    // of it does not parse as this build's event.
    #[derive(serde::Deserialize)]
    struct Versioned {
        v: u32,
    }
    match serde_json::from_str::<Versioned>(line) {
        Ok(Versioned { v }) if v > VERSION => return Admission::Newer { version: v },
        Ok(_) => {}
        Err(e) => {
            return Admission::Malformed {
                reason: e.to_string(),
            };
        }
    }
    match serde_json::from_str::<Event>(line) {
        Ok(event) => Admission::Taken(Box::new(event)),
        Err(e) => Admission::Malformed {
            reason: e.to_string(),
        },
    }
}

/// **Catch the timeline up** with the spool.
///
/// Only lines written since the last catch-up are read, file by file. A line
/// still being written — one with no newline yet — is left for the next
/// catch-up, never taken half-written. An event already taken is not taken
/// twice.
pub fn catch_up() -> Request {
    Request::CatchUp
}

/// **Rebuild the timeline** from the spool.
///
/// Everything taken in is dropped and every line is taken again, so a change
/// in how lines are read reaches the past too. The spool itself is left
/// alone: it is the source of truth.
pub fn rebuild() -> Request {
    Request::Rebuild
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::{Kind, Source};

    /// A line in this build's format, for an agent's tool call.
    const TOOL_LINE: &str = r#"{"v":1,"id":"01JABCDEFGHJKMNPQRSTVWXYZ0","at":"2026-09-26T00:02:11Z","source":"claude_code","kind":"agent.tool","session":"b3f","cwd":"/home/me/dev/integral","git":{"branch":"feat/INT-132","head":"a1b2c3"},"data":{"tool":"Edit","path":"src/auth.rs"}}"#;

    /// A line in the current format is taken as its event.
    #[test]
    fn a_current_line_is_taken_as_its_event() {
        let Admission::Taken(event) = admit(TOOL_LINE) else {
            panic!("not taken: {:?}", admit(TOOL_LINE));
        };
        assert_eq!(event.source, Source::ClaudeCode);
        assert_eq!(event.kind, Kind::AgentTool);
        assert_eq!(event.data["path"], "src/auth.rs");
    }

    /// An event read back from its own line is the same event.
    #[test]
    fn an_event_survives_its_own_line() {
        let Admission::Taken(event) = admit(TOOL_LINE) else {
            panic!("not taken");
        };
        assert_eq!(admit(&event.to_line()), Admission::Taken(event));
    }

    /// A line from a newer format is set aside, even when this build could
    /// not read the rest of it.
    #[test]
    fn a_newer_line_is_set_aside_not_refused() {
        let line = r#"{"v":2,"something":"else"}"#;
        assert_eq!(admit(line), Admission::Newer { version: 2 });
    }

    /// A kind or source a newer Pulsline added within the same format is
    /// still taken, as unknown.
    #[test]
    fn an_unknown_kind_in_the_same_format_is_taken_as_unknown() {
        let line = TOOL_LINE
            .replace("agent.tool", "agent.dreamed")
            .replace("claude_code", "gemini");
        let Admission::Taken(event) = admit(&line) else {
            panic!("not taken");
        };
        assert_eq!(event.kind, Kind::Unknown);
        assert_eq!(event.source, Source::Unknown);
    }

    /// A line that is not an event is set aside with the reason.
    #[test]
    fn a_line_that_is_not_an_event_is_set_aside_with_a_reason() {
        for line in ["not json", r#"{"v":1}"#, r#"{"id":"x"}"#] {
            assert!(
                matches!(admit(line), Admission::Malformed { ref reason } if !reason.is_empty()),
                "{line}"
            );
        }
    }

    /// An empty line is passed over and counted nowhere.
    #[test]
    fn an_empty_line_is_passed_over() {
        let mut tally = Tally::default();
        tally.count(&admit("   "));
        assert_eq!(admit(""), Admission::Blank);
        assert_eq!(tally, Tally::default());
    }

    /// Catching up asks for the lines written since the last time.
    #[test]
    fn catching_up_asks_for_the_new_lines() {
        assert_eq!(catch_up(), Request::CatchUp);
    }

    /// Rebuilding asks for every line again.
    #[test]
    fn rebuilding_asks_for_every_line_again() {
        assert_eq!(rebuild(), Request::Rebuild);
    }
}
