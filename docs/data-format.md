# Data format

Everything lives under one data directory: `$PULSLINE_DATA_DIR` when set,
otherwise the platform's data directory (`~/.local/share/pulsline` on Linux).

```
<data>/
  spool/2026-09-26.jsonl   the source of truth: append only, one file per UTC day
  index.db                 SQLite, built from the spool; safe to delete
```

## The spool

The spool is a contract with every program that writes it (the hook, the MCP
server) or reads it (Pulsline's clients, your scripts):

- One event per line, as a JSON object, ending in `\n`.
- A writer appends a whole line with a single write, to the file named by
  the event's UTC date. Files are created readable by the user alone.
- A reader takes only lines that end in `\n`; a last line without one is
  still being written.
- Fields are only added. A change an older reader would misread bumps the
  version `v`; a reader sets aside lines with a newer `v` than it knows.
- A `source` or `kind` a reader does not know is read as unknown, not refused.

### Fields (`v` = 1)

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `v` | integer | yes | Format version, `1` |
| `id` | string | yes | ULID; unique, sorts by creation time |
| `at` | string | yes | When it happened, RFC 3339 in UTC |
| `source` | string | yes | `claude_code`, `codex`, `github`, `linear` |
| `kind` | string | yes | What happened (below) |
| `session` | string | no | The agent's session id |
| `cwd` | string | no | The directory the agent worked in |
| `git` | object | no | `branch`, `head` of the repository at `cwd` |
| `data` | object | no | What the kind says about it |

### Kinds

| Kind | Recorded when |
| --- | --- |
| `agent.session.started` | An agent session starts or resumes |
| `agent.prompt` | The user sends a prompt |
| `agent.tool` | The agent used a tool |
| `agent.turn.ended` | The agent finished answering |
| `agent.compacted` | The conversation was compacted |
| `agent.session.ended` | The session ended |

### Example

```json
{"v":1,"id":"01JABCDEFGHJKMNPQRSTVWXYZ0","at":"2026-09-26T00:02:11Z","source":"claude_code","kind":"agent.tool","session":"b3f…","cwd":"/home/me/dev/integral","git":{"branch":"feat/INT-132","head":"a1b2c3"},"data":{"tool":"Edit","path":"src/auth.rs"}}
```
