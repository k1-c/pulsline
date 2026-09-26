# Pulsline

Journal your coding-agent work and see your day as a timeline, locally. A
Claude Code plugin records what agents do into an append-only spool; a TUI and
a desktop client (GPUI) read it back as a day of work sessions, alongside
GitHub and Linear. No server: everything stays on the machine.

The plan and the milestones are in
[docs/implementation-plan.md](docs/implementation-plan.md) (Japanese).

## Build & Check Commands

The toolchain is pinned in `mise.toml`, so a checkout needs nothing but
[mise](https://mise.jdx.dev/):

```bash
mise install            # fetch the pinned Rust toolchain
mise run verify         # fmt, lint, test, build — run after any implementation task
mise run dev -- paths   # run the CLI from source
mise run desktop        # run the desktop client
mise run verify:desktop # lint and build the desktop client
```

`verify` is the gate before every commit. Underneath it is:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

The root package needs nothing outside the toolchain: SQLite is compiled in
(`rusqlite`'s `bundled`). Keep it that way — no dependency that needs a system
library or `pkg-config`. The desktop client is the exception, and is kept out
of the default members for that reason: GPUI needs system libraries on Linux,
which `desktop/with-deps.sh` supplies (`desktop/shell.nix` on NixOS).

Every `uses:` in `.github/workflows/` is pinned to a full commit SHA with the
version in a trailing comment (`@<sha> # v4.4.0`); Dependabot keeps the pins
current, so never add one by tag or branch.

## Project Structure

A workspace of two packages. The root package `pulsline` is the CLI (and, in
later milestones, the hook, the MCP server, and the TUI) plus a library the
desktop client uses. `desktop/` is `pulsline-desktop`, the GPUI client.

`src/` is three directories, one per layer (see [Architecture](#architecture)):

- `src/main.rs` — the binary: hands over to `interface::cli`;
  `src/lib.rs` — the library: every module, for the binary and `desktop/`;
  `src/config.rs` — where the data lives (`$PULSLINE_DATA_DIR`, or
  `~/.local/share/pulsline`); `src/logging.rs` — `$PULSLINE_LOG` to stderr
- `src/core/` — no files, no network, no terminal, no environment:
  - `entity/` — what Pulsline is about, one file per aggregate: `event.rs`
    (a spool line, its format a contract: `event::VERSION`), `ids.rs`
  - `usecase/` — what can be done, one module per aggregate (`timeline`),
    each returning its `Request`; `mod.rs` indexes them and combines the
    requests into `usecase::Request`. **This layer is the specification** —
    see `docs/development.md` ("The use case layer")
  - `message.rs` — `Message`, the answers `dispatch` brings back
- `src/interface/` — the ways in: `cli/` (the subcommands, `docs/cli.md`)
- `src/infra/` — the systems Pulsline calls on:
  - `spool/` — the append-only JSONL files, one per UTC day
  - `index/` — the SQLite index built from the spool, and its migrations
    (`migrations/NNNN_*.sql`, append only)
  - `dispatch.rs` — `execute`: carries out one `usecase::Request`
- `desktop/` — the GPUI client; `shell.nix` and `with-deps.sh` for its
  system libraries
- `tests/` — `architecture.rs` (the layers depend inwards, the core does no
  I/O, the desktop client uses no interface), `usecase_spec.rs` (the use case
  layer reads as the specification)
- `docs/` — user and contributor documentation, indexed by `docs/README.md`

## Architecture

**The layers only depend inwards** (Clean Architecture), and
`tests/architecture.rs` fails when one names a layer it may not, when the
core uses a crate for storage, the terminal, drawing, the network, or
logging, or when the core names `std::fs`, `std::io`, `std::net`,
`std::process`, or `std::env`:

```
interface/   cli (tui · mcp later)          the ways in       ─┐ both depend on core,
infra/       spool · index · dispatch       what it calls on  ─┘ not on each other
core/        usecase → entity; message      what Pulsline is: no I/O
main.rs, lib.rs, config.rs, logging.rs      beside the layers
desktop/     a way in of its own: core and infra, never interface
```

- `interface/` and `infra/` do not know each other, except that each
  subcommand in `interface/cli` assembles the infra it needs.
- The core is always named `crate::core::…`; a bare `core::` is Rust's own
  crate.
- A use case never performs I/O and never reads a clock or randomness: it
  takes explicit arguments and returns its aggregate's `Request`, which
  `infra::dispatch` carries out. Ids and timestamps come in as arguments.
- Use cases are grouped by the aggregate they act on, never by kind of
  operation.

Other invariants:

- **The spool is the source of truth; the index is derived.** Anything in
  the index can be dropped and rebuilt from the spool (`pulsline index
  rebuild`), so an index migration never has to carry data.
- **The spool format is a contract** with the hook, the MCP server, and any
  program reading it (`docs/data-format.md`). Fields are only added; a change
  an older reader would misread bumps `event::VERSION`.
- **Writers append whole lines with one write; readers take only complete
  lines.** A hook must never slow an agent down or fail it.
- Spool files are created readable by the user alone.

## Commits, PRs, and releases

Conventional Commits (`feat:`, `fix:`, `docs:`, …), checked on PR titles by
`.github/workflows/pr-title.yml`. `feat` bumps the minor version, `fix` the
patch. release-plz opens the release PR and publishes the root package; the
desktop client is not released to crates.io.
