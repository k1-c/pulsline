# Development

## Setup

The Rust toolchain is pinned in `mise.toml`, so [mise](https://mise.jdx.dev/)
is all you need to install:

```sh
git clone https://github.com/k1-c/pulsline.git
cd pulsline
mise install        # fetch the pinned toolchain
mise run verify     # format, lint, test, build
```

Use a throwaway data directory while trying changes, so your real spool is
left alone:

```sh
PULSLINE_DATA_DIR=$(mktemp -d) mise run dev -- index catch-up
```

Before changing code, read [AGENTS.md](../AGENTS.md): the project map and the
architecture invariants.

## The use case layer

`src/core/usecase/` is the specification of Pulsline: what a person or an
agent can do, and the rules each thing follows. It is written to be read as
such, by people and by agents working on the code.

**Structure.** One module per aggregate (`timeline`, and more as milestones
land). Everything done to an aggregate lives in its module. Never group use
cases by kind of operation. `usecase/mod.rs` opens with a table of the
modules.

**A use case** is a top-level `pub fn` taking explicit arguments and
returning its aggregate's `Request`, or a plain value for a rule. It never
performs I/O, and never reads a clock or randomness.

**Its doc comment is the specification.** It opens with the use case's name
in bold, as the user would say it; then its rules in plain sentences — what
happens, what is set aside or refused and why, what is left alone:

```rust
/// **Take a spool line into the timeline.**
///
/// A line written in this build's format, or an older one, is taken as its
/// event. A line from a newer Pulsline is set aside, not refused: …
pub fn admit(line: &str) -> Admission {
```

**The module's `//!` summary** is its table of contents: it links every use
case in the module.

**Its tests state the rules, one per test.** A `///` sentence above each test
says the rule; the test's name says it again as a sentence
(`a_newer_line_is_set_aside_not_refused`, never `test_admit`).

`tests/usecase_spec.rs` enforces the shape. Whether the words say the right
thing is for review.

## The desktop client

`desktop/` is its own package, outside the workspace's default members, so
`mise run verify` does not build it. Use:

```sh
mise run desktop          # run it
mise run verify:desktop   # lint and build it
```

GPUI needs system libraries on Linux: Vulkan, Wayland, X11 (xcb,
xkbcommon), fontconfig, and freetype. `desktop/with-deps.sh` runs the command
inside `desktop/shell.nix` on NixOS; elsewhere install them from the system
package manager (on Debian/Ubuntu: `libxkbcommon-x11-dev libwayland-dev
libx11-xcb-dev libfontconfig-dev libfreetype-dev libvulkan-dev`). macOS
needs nothing extra.

## Where a change is documented

| When you change | Update |
| --- | --- |
| what a person or an agent can do | the use case in `src/core/usecase/<aggregate>.rs`: its doc comment and its tests |
| a subcommand or its output | [cli.md](cli.md) |
| the spool format | [data-format.md](data-format.md), and `event::VERSION` for a breaking change |
| the index schema | a new `src/infra/index/migrations/NNNN_*.sql`; never edit a shipped one |
