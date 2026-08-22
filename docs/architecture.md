# Bao — architecture

One page on how Bao is shaped today. The product contract lives in
`docs/01–05.md`; the detailed design records are in `docs/design/` (index at
the bottom).

## What it is

Bao supervises AI coding agents. A developer runs a daemon on
each machine they own; the daemon supervises coding-session harnesses (pi
first) in isolated working copies; clients attach to the live sessions from any
terminal. Compute is bring-your-own — Bao commands the user's machines, never
rents them.

The unit of work is a **session**: one isolated checkout + one live session
process in a PTY + its conversation + its event log. A session lives in
exactly one place at a time; attaching elsewhere joins the same live process,
never a copy.

## Crates

```
bao (the binary) ──┬── bao-daemon ── bao-transport ── bao-protocol ── bao-core
                  ├── bao-tui    ── bao-client  ── bao-transport, bao-protocol, bao-core
                  ├── bao-client ── bao-transport, bao-protocol, bao-core
                  └── bao-transport ── bao-core
```

- **`bao-core`** — the pure domain: the session model, the lifecycle state
  machine, alert signals, and the value types. No I/O, no `tokio`, no wire
  types — reusable by any client (native or wasm).
- **`bao-protocol`** — the wire contract: the versioned message vocabulary
  (`Rpc`/`Reply`/`FromHost`/`WireError` and the request/reply payloads).
  Pure `serde`, owned by neither the client nor the daemon.
- **`bao-transport`** — the plumbing both ends share: length-prefixed
  framing and addressing (`Addr`).
- **`bao-client`** — the typed client: `Conn`/`ConnWriter` plus a typed
  event stream. Frontends speak only this and `bao-core`; the wire
  vocabulary never reaches them.
- **`bao-daemon`** — the supervisor: owns the live PTY process, the event
  log, and the sandbox/harness adapters (the only crate that touches the OS).
- **`bao-tui`** — the overview you see in the terminal: ratatui
  components (rail, terminal, footer, palette, help), the status language,
  theming.
- **`bao`** — the binary (composition root); the only crate allowed `anyhow`
  (libraries use typed `thiserror` errors).

Dependency direction: everything points inward at `bao-core`; `bao-core`
points at nothing.

Conventions: `unsafe_code = "forbid"` and clippy `all = "warn"` workspace-wide;
dependencies are promoted to `[workspace.dependencies]` only when a second
crate uses them.

## Trace a request

Follow one keystroke in the overview:

1. You press a key; `bao-tui`'s `Overview` maps it to an `Action` and updates the
   focused component's state.
2. Anything needing the daemon becomes a typed RPC (`bao-client` →
   `bao-daemon`).
3. The daemon's `Manager` acts on the `Session` (spawn, resize, input, …) and
   appends the session's PTY output to the event log.
4. The daemon derives the session's state — alert, idle, the current
   screen — and pushes it back: `Watch` streams every session's state, `Attach`
   streams one terminal's bytes.
5. The TUI renders that state as-is; it never computes understanding itself.

Attach is the same shape: `Attach` returns a screen snapshot (the current
screen, no history replay) plus the live byte stream from the current
sequence — the client feeds both into its emulator and renders.

## Core concepts

### The daemon is the single source of truth

Each machine's daemon owns the sessions on it. Clients are **stateless
renderers**: everything a view understands (`alert`, idle, status, the
screen) is derived by the daemon and pushed as typed state. Views never
compute understanding themselves.

### The event log — and the screen

Every session appends its terminal output to an on-disk event log
(`events.log`, JSONL) and a ring buffer. But **attach does not replay the
log**: the daemon keeps a live `vt100` parser per session (the terminal's
_state_) and, on attach, sends a **screen snapshot** — the current screen
rebuilt as a short byte stream — plus the live byte stream from the current
sequence. State is transmitted directly, never reconstructed from history
(see `docs/design/no-replay-attach.md`). The snapshot is generated at the
session's current terminal size: the PTY window, the daemon's snapshot
parser, and each client's emulator share one size, so the screen is laid out
at the width the client actually renders.

### The wire

Length-prefixed JSON frames, versioned. `Request`/`Rpc` client→host;
`FromHost` host→client (`Reply`/`Err`/`Output`/`Status`/`State`). Two verbs:
`Watch` streams the every session's derived state; `Attach` streams one
session's terminal. `WireError` is typed so clients branch on _kind_.

### Sandboxes

One isolated working copy per session, so sessions don't step on each other.
`SandboxSpec` (what the user asks) is materialized by `Sandbox::create` into a
`Sandbox` — a `Workspace` plus the `SandboxBackend` that built it, placed in
the `WorkspaceStore` — and the daemon **never silently delivers a weaker
isolation than requested**.
Worktrees (file isolation) are the default backend. `Bubblewrap` (Linux, behind
the `bubblewrap` feature) and `Seatbelt` (macOS) are the process-sandboxing
backends; Landlock on Linux is next, matching Claude Code and Codex, with
containers and MicroVMs later per-machine options.

### Harness adapters

The `Harness` trait is the only harness-specific surface: `launch`,
`resume`, `waiting_for_input`, `pack`/`unpack`. A new harness is one file +
one registry line (see `docs/design/terminology.md` for model vs harness
vs session).

### Honesty

Statuses carry provenance; nothing is guessed. Alert (`errored`,
`interrupted`, `idle`, `damaged`, `waiting`) derives only from facts (status,
exit code, last activity, a harness-proven "waiting for you"). The UI says
"unknown" rather than inventing.

## The TUI: focus-driven panes

The overview is a **composition of focusable panes** (a ratatui
Component architecture — one struct per surface, an `Action` bus, an `App`
that runs the loop), each with its own keybindings (see
`docs/design/panes.md`):

- **Rail** — the compact session browser (severity ladder, header status
  strip). Its keys navigate and act (`r/s/n/d`, palette, filter, help).
- **Terminal** — the running harness, natively: raw byte passthrough (no
  line buffer), a real emulator, the harness's own echo and editing. `Tab`
  steps in, `⌃q` steps out, `Enter` fullscreens.

Exactly one pane owns the keyboard at a time; keys never overlap. The
semantic status language — glyph + color + word, triple-encoded, with a
token map (`theme.rs`) for theming, `NO_COLOR`, and ASCII fallback — is
shared by every surface (see `docs/design/visual-language.md`).

## Invariants (the contract)

Bring-your-own compute · harness-agnostic · your work follows you · a unit
of work is single and continuous · never misrepresent · legibility over
spectacle · resilience of state. (Full text: `docs/03-principles.md`.)

## Where to read next

- Product: `docs/01-product.md`, `02-capabilities.md`, `03-principles.md`,
  `04-stages.md` (roadmap), `05-open-questions.md`.
- Design records: `docs/design/terminology.md`, `visual-language.md`,
  `panes.md`, `no-replay-attach.md`, `channels.md` (one logical channel per
  connection), `event-sourcing.md` (the session lifecycle FSM + backgrounded
  launch saga), `state-machines.md` (the FSM convention), `crate-topology.md`
  (the crate/dependency boundaries).
