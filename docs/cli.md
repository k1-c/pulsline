# CLI

`pulsline <command>`. Diagnostics go to stderr, filtered by `$PULSLINE_LOG`
(for example `PULSLINE_LOG=debug`); stdout is only the command's output.
`$PULSLINE_DATA_DIR` moves the data directory (see
[data-format.md](data-format.md)).

## `pulsline paths`

Prints where Pulsline keeps its data:

```
data   /home/me/.local/share/pulsline
spool  /home/me/.local/share/pulsline/spool
index  /home/me/.local/share/pulsline/index.db
```

## `pulsline index catch-up`

Takes into the index what the spool gained since the last catch-up: only the
bytes past what was read of each file, and only complete lines. Prints a
tally:

```
caught up: 42 events taken, 1 malformed lines set aside
```

Lines from a newer Pulsline, and lines that are not events, are set aside and
counted; they never stop the rest. Exits non-zero only when the index or the
spool cannot be read or written.

## `pulsline index rebuild`

Drops everything in the index and takes in the whole spool again. The spool
is left alone. Use it after an upgrade that reads lines differently.
