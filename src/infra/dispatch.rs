//! Carries out one use case [`Request`] against the spool and the index, and
//! answers with a [`Message`].

use std::path::PathBuf;

use crate::core::message::Message;
use crate::core::usecase::Request;
use crate::core::usecase::timeline::{self, Admission, Tally};
use crate::infra::index::Index;
use crate::infra::spool::Spool;

/// What dispatch works against.
#[derive(Debug, Clone)]
pub struct Context {
    pub spool: Spool,
    pub index_path: PathBuf,
}

/// Carries out `request`.
pub fn execute(request: Request, cx: &Context) -> Message {
    match request {
        Request::Timeline(timeline::Request::CatchUp) => match catch_up(cx, false) {
            Ok(tally) => Message::CaughtUp(tally),
            Err(error) => failed("catch the timeline up", error),
        },
        Request::Timeline(timeline::Request::Rebuild) => match catch_up(cx, true) {
            Ok(tally) => Message::Rebuilt(tally),
            Err(error) => failed("rebuild the timeline", error),
        },
    }
}

fn failed(what: &'static str, error: anyhow::Error) -> Message {
    Message::Failed {
        what,
        error: format!("{error:#}"),
    }
}

fn catch_up(cx: &Context, from_scratch: bool) -> anyhow::Result<Tally> {
    use anyhow::Context as _;

    let mut index = Index::open(&cx.index_path)
        .with_context(|| format!("opening the index at {}", cx.index_path.display()))?;
    if from_scratch {
        index.clear()?;
    }
    let mut tally = Tally::default();
    for file in cx.spool.files().context("listing the spool")? {
        let mut offset = index.offset(&file)?;
        if offset > cx.spool.len(&file)? {
            // The file is shorter than what was read of it: it was replaced.
            // Read it again from the start; events already taken stay one row.
            tracing::warn!(file, offset, "spool file shrank; reading it again");
            offset = 0;
        }
        let chunk = cx
            .spool
            .read_from(&file, offset)
            .with_context(|| format!("reading {file}"))?;
        let mut events = Vec::new();
        for line in &chunk.lines {
            let admission = timeline::admit(line);
            tally.count(&admission);
            match admission {
                Admission::Taken(event) => events.push(*event),
                Admission::Malformed { reason } => {
                    tracing::warn!(file, reason, "malformed spool line set aside")
                }
                Admission::Newer { version } => {
                    tracing::debug!(file, version, "spool line from a newer Pulsline set aside")
                }
                Admission::Blank => {}
            }
        }
        index.take(&file, &events, chunk.end)?;
    }
    Ok(tally)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::core::entity::{Event, EventId, Kind, Source};

    fn event(at: &str, n: u128) -> Event {
        Event {
            v: 1,
            id: EventId::from_parts(0, n),
            at: at.parse().unwrap(),
            source: Source::ClaudeCode,
            kind: Kind::AgentPrompt,
            session: None,
            cwd: None,
            git: None,
            data: serde_json::json!({ "text": "hello" }),
        }
    }

    fn context(dir: &std::path::Path) -> Context {
        Context {
            spool: Spool::new(dir.join("spool")),
            index_path: dir.join("index.db"),
        }
    }

    fn count(cx: &Context) -> u64 {
        Index::open(&cx.index_path).unwrap().event_count().unwrap()
    }

    #[test]
    fn catching_up_takes_only_what_is_new() {
        let dir = tempfile::tempdir().unwrap();
        let cx = context(dir.path());
        cx.spool.append(&event("2026-09-25T10:00:00Z", 1)).unwrap();
        cx.spool.append(&event("2026-09-26T10:00:00Z", 2)).unwrap();
        let first = execute(timeline::catch_up().into(), &cx);
        assert_eq!(
            first,
            Message::CaughtUp(Tally {
                taken: 2,
                ..Tally::default()
            })
        );

        cx.spool.append(&event("2026-09-26T11:00:00Z", 3)).unwrap();
        let second = execute(timeline::catch_up().into(), &cx);
        assert_eq!(
            second,
            Message::CaughtUp(Tally {
                taken: 1,
                ..Tally::default()
            })
        );
        assert_eq!(count(&cx), 3);
    }

    #[test]
    fn bad_lines_are_set_aside_without_stopping_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let cx = context(dir.path());
        cx.spool.append(&event("2026-09-26T10:00:00Z", 1)).unwrap();
        let path = cx.spool.dir().join("2026-09-26.jsonl");
        let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(b"garbage\n{\"v\":9}\n").unwrap();
        cx.spool.append(&event("2026-09-26T10:05:00Z", 2)).unwrap();

        let message = execute(timeline::catch_up().into(), &cx);
        assert_eq!(
            message,
            Message::CaughtUp(Tally {
                taken: 2,
                newer: 1,
                malformed: 1
            })
        );
    }

    #[test]
    fn rebuilding_takes_every_line_again() {
        let dir = tempfile::tempdir().unwrap();
        let cx = context(dir.path());
        cx.spool.append(&event("2026-09-26T10:00:00Z", 1)).unwrap();
        execute(timeline::catch_up().into(), &cx);
        let message = execute(timeline::rebuild().into(), &cx);
        assert_eq!(
            message,
            Message::Rebuilt(Tally {
                taken: 1,
                ..Tally::default()
            })
        );
        assert_eq!(count(&cx), 1);
    }

    #[test]
    fn an_empty_spool_catches_up_to_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let cx = context(dir.path());
        assert_eq!(
            execute(timeline::catch_up().into(), &cx),
            Message::CaughtUp(Tally::default())
        );
    }
}
