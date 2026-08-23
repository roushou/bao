# Crate topology: dependency boundaries, not feature areas

The workspace was cut along feature areas (sessions, protocol, tui, harness)
instead of dependency direction. The symptoms: `bao-core` is a dump of
business logic, `bao-agent` is really a harness, `bao-client` isn't a client,
`bao-host` is really a daemon, and "ports & adapters" exists only in prose.
The root cause: crates were modules promoted to directories.

This record pins the fix. A crate is a _dependency/ownership boundary_, and Bao
has exactly three real seams — the pure model, the supervisor that touches the
OS, and the transport + frontends. Everything below follows from that.

## 1. The target topology

```
bao-core     pure domain — no tokio, no I/O
bao-protocol wire contract — versioned message vocabulary (pure serde)
bao-transport framing + addressing (FrameReader/FrameWriter, Addr)
bao-client   typed client (Conn/ConnWriter + typed events)
bao-daemon   supervisor — PTY/git/files + sandbox/harness adapters (modules)
bao-tui      frontend — the overview
bao          entrypoint — the binary (composition root)
```

```
bao-core     pure model: Status, Alert, LifecycleEvent + FSM, SessionMeta,
             EventKind/SessionEvent, Workspace/SandboxKind/SandboxSpec,
             value types, error (domain rules only)
bao-protocol Rpc/Reply/FromHost/WireError/ChannelKind/PROTOCOL_VERSION,
             LaunchRequest/DaemonInfo/WireBytes (depends on core only)
bao-transport framing (FrameReader/FrameWriter) + Addr/DEFAULT_PORT (tokio)
bao-client   the client (Conn/ConnWriter + HostEvent); wire envelopes stay private
bao-daemon   session (live PTY/process/log/screen), manager (registry + saga),
             store (meta.json/events.log), sandbox (SandboxFactory seam +
             Sandbox + SandboxBackend + WorkspaceStore; one file per backend:
             InPlace/Worktree/Bubblewrap/Seatbelt), harness (Harness trait +
             Pi/Fallback), server
bao-tui      the overview renderer
bao          main, CLI dispatch, daemon-process management, Context
```

Dependency direction: everything points inward at `bao-core`; `bao-core`
points at nothing. `bao` is the only crate that imports every other crate, and
nothing imports it. The contract (`bao-protocol`) is shared by both ends but
owned by neither; the daemon depends on the transport, never on the client.

## 2. The rules

1. **Dependency rule.** Every `use` points toward `bao-core`. A crate may
   depend on anything _inside_ its layer, never on a layer outside it.
2. **Tokio rule.** `bao-core` has no `tokio` in its `Cargo.toml`. The domain is
   reusable by any client — including wasm — so its purity is a compile error,
   not a convention. This is _why_ framing lives in `bao-transport` and the
   client in `bao-client`, never in `bao-core`: they need tokio, and a module
   cannot enforce the boundary.
3. **Port-graduation rule.** The `SandboxBackend` and `Harness` traits live
   as modules inside `bao-daemon`. A trait moves into `bao-core` only when its first
   adapter becomes its own crate (because that adapter drags a heavy dependency
   or gains external consumers). No speculative ports in the core.
4. **`Transition` convention.** Every pure FSM follows `docs/design/
   state-machines.md`: a state enum + an event enum + one total `apply` + a
   lenient `fold` + legal/illegal cell tests.

## 3. The move map

| From                                                        | To                                                                                                                                                                    |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bao-host`                                                  | `bao-daemon`                                                                                                                                                          |
| `bao-agent` (Harness trait + Pi/Fallback)                   | `bao-daemon::harness`                                                                                                                                                 |
| `bao-client` (framing + Conn)                               | `bao-transport` (framing) + `bao-client` (Conn)                                                                                                                       |
| `bao-cli`                                                   | `bao`                                                                                                                                                                 |
| `bao-core::session::Session` (live: PTY/process/log/screen) | `bao-daemon::session`                                                                                                                                                 |
| `bao-core::session::Manager` (registry + saga)              | `bao-daemon::manager`                                                                                                                                                 |
| `bao-core::session::SessionStore`/`StoredMeta`              | `bao-daemon::store`                                                                                                                                                   |
| `bao-core::screen` (vt100)                                  | `bao-daemon`                                                                                                                                                          |
| `bao-core::sandbox` impls + `IsolationBackend`              | `bao-daemon::sandbox` (`SandboxBackend` trait + `InPlace`/`Worktree`/`Bubblewrap`)                                                                                    |
| `bao-core::protocol` (types)                                | `bao-protocol` (own crate — the wire contract)                                                                                                                        |
| `bao-core::types::{LaunchRequest, DaemonInfo, WireBytes}`   | `bao-protocol`                                                                                                                                                        |
| `bao-core::types::{Addr, DEFAULT_PORT}`                     | `bao-transport`                                                                                                                                                       |
| `bao-wire::frame`                                           | `bao-transport`                                                                                                                                                       |
| `bao-wire::client`                                          | `bao-client` (wire envelopes made private; typed `HostEvent` stream)                                                                                                  |
| `bao-core::types::Hostname::local()` (I/O)                  | `bao-daemon::hostname`                                                                                                                                                |
| `bao-core::types::SessionSpec`                              | `bao-daemon::session`                                                                                                                                                 |
| `bao-core::error` (Pty/Spawn/Worktree/…)                    | split → `bao-core::error` (domain rules), `bao-daemon::error`, `bao-transport::error`, `bao-client::error`                                                            |
| `bao-core` keeps                                            | `Status`, `Alert`, `LifecycleEvent`+FSM, `SessionMeta`, `EventKind`/`SessionEvent`, `Workspace`/`SandboxKind`/`SandboxSpec`, value types, `error` (domain rules only) |

The key split the rest of the table serves: **the live `Session` is not a
domain object; `SessionMeta` is.** Everything that produces `SessionMeta`
(process, PTY, log, restore fold) is the daemon's job; `SessionMeta` itself —
plus the state machine that decides its `Status` — is the domain's.

## 4. Naming decisions

- `bao-host → bao-daemon` — the crate is the process; name it for the process.
- `bao-agent → bao-daemon::harness` — harness adapters are used only by the
  daemon and there are two impls; a crate would be a boundary with nothing on
  the other side.
- `bao-wire` split into `bao-protocol` (the contract), `bao-transport`
  (framing + addressing), and `bao-client` (the typed client) — the contract
  is shared by both ends, the transport is shared plumbing, and the client is
  the only surface frontends see.
- `IsolationBackend → SandboxBackend` (trait); the data struct `Sandbox →
  Workspace`
  (the isolated working copy). Impls: `InPlace`, `GitWorktree`. `SandboxKind`
  and `SandboxSpec` keep their names.
- `bao-cli → bao` — the binary is `bao`, so the crate that produces it is `bao`.

## 5. The entrypoint (`bao`)

`bao` is the composition root: it imports every crate, nothing imports it, and
it contains no domain logic. It owns `main.rs`, CLI parsing and subcommand
dispatch (`bao`, `bao daemon`, `bao launch`, `bao attach`, `bao resume`,
`bao list`, `bao info`, …), daemon-process management (start-if-not-running,
wait, connect), and a small `Context`. If logic accumulates here, that is a
smell — the same way `tokio` in `bao-core` is.

## 6. How this grows

The skeleton is fixed; all growth is additive and points inward.

- `bao-core` grows by data only: new `Status` variants, new `LifecycleEvent`s
  (`Forked`, `Moved`), new alert kinds, new types. It stays pure forever.
- `bao-protocol` grows by verbs, always versioned: new `Rpc`/`Reply`/
  `FromHost` variants and payloads. Bump `PROTOCOL_VERSION` on a breaking
  change.
- `bao-transport` grows by transports: the `Addr` enum (`Tcp | Unix`) and the
  framing stay, and remote (QUIC) plugs in as a new dial/bind adapter — the
  seam is pinned in `channels.md`.
- `bao-client` grows by typed methods and richer events; its wire envelopes
  stay private.
- `bao-daemon` is the growth home: new sagas (fork, move), new sandbox backends
  (`Landlock`, and later containers/MicroVMs), new harnesses (`ClaudeCode`,
  `Codex`), later `machine/` and `peers/` modules. It grows by adapter modules,
  never by feature crates.
- Frontends are new crates (`bao-web`, `bao-mobile`), each thin, depending only
  on `bao-core` + `bao-client`.

Move and fork are consequences of the event-sourced domain, not new
architecture: move = ship `meta.json` + `events.log`, re-fold on the target
daemon, resume via the harness's `pack`/`unpack`; fork = a new event, a new
stream prefix. The first release already paid for both.

**Resist:** per-feature crates (`bao-fork`, `bao-move`), I/O creeping back into
`bao-core`, speculative ports in the core, and a second god-crate (the daemon
grows only by adding adapter modules).

## 7. Implementation order

Done. The steps below are retained as the historical record of how the
current topology was reached.

Each step ends with `cargo test --workspace` + clippy green.

1. Rename crates: `bao-host → bao-daemon`, `bao-cli → bao` (mechanical).
2. Extract `bao-wire`: move framing + the client out of `bao-core`/`bao-client`;
   delete `bao-client`.
3. Slim `bao-core`: move the live `Session`/`Manager`/`SessionStore`/`screen`
   into `bao-daemon`; keep only the pure domain. `bao-core` becomes tokio-free.
4. Fold harness + sandbox into `bao-daemon`: `bao-agent`'s `Harness` trait +
   impls into `bao-daemon::harness`; rename `IsolationBackend →
   SandboxBackend`,
   `Sandbox` (data) → `Workspace`; impls into `bao-daemon::sandbox`.
5. Re-point frontends: `bao-tui` and `bao` depend only on `bao-core` +
   `bao-wire` (never `bao-daemon`).
6. Update docs: `architecture.md`, the crate map, and `terminology.md` where the
   host/daemon and agent/harness vocabulary shifted.

## 8. Still open

The three product questions in `05-open-questions.md` remain undecided and are
not part of this restructure: a stuck `starting` (timeout vs. age-only),
failed-launch durability (toast vs. an inspectable `errored` row), and
daemon-crash-mid-launch cleanup.
