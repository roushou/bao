# Contributing

## Setup

Rust stable is all you need — `rust-toolchain.toml` pins the channel and
`rustup` installs it automatically. Everything else is plain `cargo`:

```bash
cargo build                       # build
cargo test --workspace            # run every crate's tests
cargo clippy --all-targets        # lint
cargo fmt --all                   # format
```

CI runs `fmt --check`, `clippy -D warnings`, `test --workspace`, `doc`, and
`cargo-deny` — keep those green locally and you're good.

## Try it

```bash
cargo run            # the overview (starts its daemon automatically)
```

## Project layout

| crate        | what it is                                                                     |
| ------------ | ------------------------------------------------------------------------------ |
| `bao-core`   | the pure domain — session model, lifecycle, alert, protocol types, value types |
| `bao-wire`   | the transport — framing + the typed client                                     |
| `bao-daemon` | the supervisor — PTY process, sandbox/harness adapters, wire server            |
| `bao-tui`    | the overview UI (ratatui components)                                           |
| `bao`        | the `bao` binary (composition root)                                            |

## Conventions

- `unsafe_code = "forbid"`; clippy `all = "warn"` (workspace-wide).
- Conventional Commits for commit messages.
- Libraries use `thiserror`; only `bao` uses `anyhow`.
- A dependency is promoted to `[workspace.dependencies]` only when a second
  crate uses it.

## Where things are

- [`docs/architecture.md`](docs/architecture.md) — how it fits together.
- [`docs/`](docs/) — product rationale and design records.

Questions? Open an issue.
