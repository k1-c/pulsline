//! The index: a SQLite database built from the spool, for reading a day back
//! quickly. Everything in it can be dropped and built again from the spool
//! (`pulsline index rebuild`), so a migration never has to carry data.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::core::entity::Event;

/// Schema migrations, applied in order. `PRAGMA user_version` holds how many
/// have been applied. Append only; never edit one that has shipped.
const MIGRATIONS: &[&str] = &[include_str!("migrations/0001_events.sql")];

/// An open index.
pub struct Index {
    conn: Connection,
}

impl Index {
    /// Opens the index at `path`, creating it and applying any migration it
    /// has not seen.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            // SQLite reports a missing directory as "unable to open", which
            // hides the cause; creating it here keeps the error meaningful.
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        // WAL lets the clients read while a catch-up writes.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::with(conn)
    }

    /// An index in memory, for tests.
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::with(Connection::open_in_memory()?)
    }

    fn with(mut conn: Connection) -> rusqlite::Result<Self> {
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// How far into the spool file `file` the index has read.
    pub fn offset(&self, file: &str) -> rusqlite::Result<u64> {
        let offset: Option<i64> = self
            .conn
            .query_row(
                "SELECT offset FROM ingest_offsets WHERE file = ?1",
                [file],
                |row| row.get(0),
            )
            .optional()?;
        Ok(offset.unwrap_or(0) as u64)
    }

    /// Takes in `events` read from `file`, and records that the file has
    /// been read up to `end` — both or neither.
    pub fn take(&mut self, file: &str, events: &[Event], end: u64) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT OR IGNORE INTO events
                     (id, v, at, source, kind, session, cwd, branch, head, data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for event in events {
                let git = event.git.as_ref();
                insert.execute(params![
                    event.id.to_string(),
                    event.v,
                    event.at.to_string(),
                    name(&event.source),
                    name(&event.kind),
                    event.session.as_ref().map(|s| s.0.as_str()),
                    event.cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    git.and_then(|g| g.branch.as_deref()),
                    git.and_then(|g| g.head.as_deref()),
                    (!event.data.is_null()).then(|| event.data.to_string()),
                ])?;
            }
        }
        tx.execute(
            "INSERT INTO ingest_offsets (file, offset) VALUES (?1, ?2)
             ON CONFLICT (file) DO UPDATE SET offset = excluded.offset",
            params![file, end as i64],
        )?;
        tx.commit()
    }

    /// Drops everything taken in, so the next catch-up reads the spool from
    /// the start.
    pub fn clear(&mut self) -> rusqlite::Result<()> {
        self.conn
            .execute_batch("DELETE FROM events; DELETE FROM ingest_offsets;")
    }

    /// How many events the index holds.
    pub fn event_count(&self) -> rusqlite::Result<u64> {
        self.conn
            .query_row("SELECT count(*) FROM events", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|n| n as u64)
    }

    /// The schema version: how many migrations have been applied.
    pub fn schema_version(&self) -> rusqlite::Result<usize> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .map(|v| v as usize)
    }
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let applied: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (n, sql) in MIGRATIONS.iter().enumerate().skip(applied as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (n + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// The spool's name for a source or a kind.
fn name(value: &impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::{EventId, Kind, Source};

    fn event(n: u128) -> Event {
        Event {
            v: 1,
            id: EventId::from_parts(0, n),
            at: "2026-09-26T00:00:00Z".parse().unwrap(),
            source: Source::ClaudeCode,
            kind: Kind::AgentSessionStarted,
            session: None,
            cwd: None,
            git: None,
            data: serde_json::Value::Null,
        }
    }

    #[test]
    fn a_new_index_has_every_migration_applied() {
        let index = Index::in_memory().unwrap();
        assert_eq!(index.schema_version().unwrap(), MIGRATIONS.len());
    }

    #[test]
    fn reopening_an_index_applies_nothing_twice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/index.db");
        Index::open(&path).unwrap();
        let index = Index::open(&path).unwrap();
        assert_eq!(index.schema_version().unwrap(), MIGRATIONS.len());
    }

    #[test]
    fn taking_an_event_twice_keeps_one_row() {
        let mut index = Index::in_memory().unwrap();
        index.take("a.jsonl", &[event(1), event(2)], 10).unwrap();
        index.take("a.jsonl", &[event(2)], 20).unwrap();
        assert_eq!(index.event_count().unwrap(), 2);
        assert_eq!(index.offset("a.jsonl").unwrap(), 20);
    }

    #[test]
    fn kinds_and_sources_are_stored_by_their_spool_names() {
        let mut index = Index::in_memory().unwrap();
        index.take("a.jsonl", &[event(1)], 1).unwrap();
        let (source, kind): (String, String) = index
            .conn
            .query_row("SELECT source, kind FROM events", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            (source.as_str(), kind.as_str()),
            ("claude_code", "agent.session.started")
        );
    }

    #[test]
    fn clearing_forgets_events_and_offsets() {
        let mut index = Index::in_memory().unwrap();
        index.take("a.jsonl", &[event(1)], 10).unwrap();
        index.clear().unwrap();
        assert_eq!(index.event_count().unwrap(), 0);
        assert_eq!(index.offset("a.jsonl").unwrap(), 0);
    }
}
