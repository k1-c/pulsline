# Pulsline

Journal your coding-agent work and see your day as a timeline — locally.

Pulsline records what your coding agents do (through a Claude Code plugin),
lets the agent itself journal what it finished and decided, and shows your
day as work sessions alongside your GitHub and Linear activity, in a TUI and
a desktop app. There is no server: your data stays on your machine.

> **Status:** early development (milestone M0: the foundation). See
> [docs/implementation-plan.md](docs/implementation-plan.md).

## Try it from source

```sh
mise install
mise run dev -- paths             # where Pulsline keeps its data
mise run dev -- index catch-up    # index what the spool holds
mise run desktop                  # the desktop client (GPUI)
```

## Documentation

See [docs/README.md](docs/README.md).

## License

MIT
