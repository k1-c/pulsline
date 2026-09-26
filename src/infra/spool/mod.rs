//! The spool: append-only JSONL files, one per UTC day, that hold every event
//! as it was recorded. It is the source of truth; the index is built from it
//! and can always be built again (docs/data-format.md).
//!
//! Writers — the hook, the MCP server — only ever append a whole line with a
//! single write, so concurrent agent sessions do not interleave within a
//! line. Readers only take lines that end in a newline, so a line still being
//! written is left for next time.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::core::entity::Event;

/// The spool directory.
#[derive(Debug, Clone)]
pub struct Spool {
    dir: PathBuf,
}

/// Lines read from a spool file, and the byte offset just past the last one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub lines: Vec<String>,
    pub end: u64,
}

impl Spool {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Appends `event` as one line to the file for its UTC day.
    pub fn append(&self, event: &Event) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let name = format!("{}.jsonl", event.at.strftime("%Y-%m-%d"));
        let mut line = event.to_line();
        line.push('\n');
        let mut file = open_for_append(&self.dir.join(name))?;
        // One write for the whole line: with O_APPEND, a concurrent writer's
        // line lands before or after it, never inside it.
        file.write_all(line.as_bytes())
    }

    /// The spool's files by name, oldest day first. A spool not yet written
    /// to has none.
    pub fn files(&self) -> io::Result<Vec<String>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".jsonl"))
            .collect();
        names.sort();
        Ok(names)
    }

    /// The size of the file `name`, in bytes.
    pub fn len(&self, name: &str) -> io::Result<u64> {
        Ok(fs::metadata(self.dir.join(name))?.len())
    }

    /// The whole lines of the file `name` from byte `offset` on. A last line
    /// with no newline yet is left out, and `end` stops before it.
    pub fn read_from(&self, name: &str, offset: u64) -> io::Result<Chunk> {
        let mut file = File::open(self.dir.join(name))?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let complete = bytes
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |at| at + 1);
        let lines = String::from_utf8_lossy(&bytes[..complete])
            .lines()
            .map(str::to_owned)
            .collect();
        Ok(Chunk {
            lines,
            end: offset + complete as u64,
        })
    }
}

/// Opens `path` for appending, creating it readable by the user alone: the
/// spool holds what the user asked their agents.
fn open_for_append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::{EventId, Kind, Source};

    fn event(at: &str, n: u128) -> Event {
        Event {
            v: 1,
            id: EventId::from_parts(0, n),
            at: at.parse().unwrap(),
            source: Source::ClaudeCode,
            kind: Kind::AgentTool,
            session: None,
            cwd: None,
            git: None,
            data: serde_json::Value::Null,
        }
    }

    #[test]
    fn events_land_in_the_file_for_their_utc_day() {
        let dir = tempfile::tempdir().unwrap();
        let spool = Spool::new(dir.path());
        spool.append(&event("2026-09-25T23:59:00Z", 1)).unwrap();
        spool.append(&event("2026-09-26T00:01:00Z", 2)).unwrap();
        spool.append(&event("2026-09-26T09:00:00Z", 3)).unwrap();
        assert_eq!(
            spool.files().unwrap(),
            ["2026-09-25.jsonl", "2026-09-26.jsonl"]
        );
        let chunk = spool.read_from("2026-09-26.jsonl", 0).unwrap();
        assert_eq!(chunk.lines.len(), 2);
        assert_eq!(chunk.end, spool.len("2026-09-26.jsonl").unwrap());
    }

    #[test]
    fn reading_resumes_from_an_offset() {
        let dir = tempfile::tempdir().unwrap();
        let spool = Spool::new(dir.path());
        spool.append(&event("2026-09-26T00:00:00Z", 1)).unwrap();
        let first = spool.read_from("2026-09-26.jsonl", 0).unwrap();
        spool.append(&event("2026-09-26T00:01:00Z", 2)).unwrap();
        let second = spool.read_from("2026-09-26.jsonl", first.end).unwrap();
        assert_eq!(second.lines.len(), 1);
        assert!(second.lines[0].contains(&EventId::from_parts(0, 2).to_string()));
    }

    #[test]
    fn a_half_written_line_is_left_for_next_time() {
        let dir = tempfile::tempdir().unwrap();
        let spool = Spool::new(dir.path());
        spool.append(&event("2026-09-26T00:00:00Z", 1)).unwrap();
        let path = dir.path().join("2026-09-26.jsonl");
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(br#"{"v":1,"id":"#).unwrap();
        let chunk = spool.read_from("2026-09-26.jsonl", 0).unwrap();
        assert_eq!(chunk.lines.len(), 1);
        assert!(chunk.end < spool.len("2026-09-26.jsonl").unwrap());
    }

    #[test]
    fn a_spool_never_written_has_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let spool = Spool::new(dir.path().join("missing"));
        assert!(spool.files().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn spool_files_are_readable_by_the_user_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let spool = Spool::new(dir.path());
        spool.append(&event("2026-09-26T00:00:00Z", 1)).unwrap();
        let mode = fs::metadata(dir.path().join("2026-09-26.jsonl"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
