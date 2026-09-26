# Pulsline

Read [AGENTS.md](AGENTS.md) first — it holds the build commands, project
structure, architecture invariants, and the commit/release flow. This file
only adds what is specific to Claude Code.

## Rules

- To change what a person or an agent can do, start in `src/core/usecase/<aggregate>.rs`: write the rule into the use case's doc comment and a test per rule, as `docs/development.md` ("The use case layer") describes. `tests/usecase_spec.rs` and `tests/architecture.rs` must pass.
- After completing any implementation task, ALWAYS run `mise run verify` before marking it done. When `desktop/` changed, run `mise run verify:desktop` too.
- Use mise for every development task (`mise run <task>`); add a task to `mise.toml` rather than documenting a bare command.
