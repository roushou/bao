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
bao-wire     transport — framing + the typed client (the tokio layer)
bao-daemon   supervisor — PTY/git/files + sandbox/harness adapters (modules)
bao-tui      frontend — the overview
bao          entrypoint — the binary (composition root)
```

```
bao-core     pure model: Status, Alert, LifecycleEvent + FSM, SessionMeta,
             EventKind/SessionEvent, Workspace/SandboxKind/SandboxSpec,
             protocol types (Rpc/Reply/FromHost/WireError), value types, error
bao-wire     framing (FrameReader/FrameWriter) + the client (Conn/HostMsg)
bao-daemon   session (live PTY/process/log/screen), manager (registry + saga),
             store (meta.json/events.log), sandbox (SandboxBackend trait + InPlace/
             GitWorktree), harness (Harness trait + Pi/Fallback), wire (server)
bao-tui      the overview renderer
bao          main, CLI dispatch, daemon-process management, Context
```

Dependency direction: everything points inward at `bao-core`; `bao-core`
points at nothing. `bao` is the only crate that imports every other crate, and
nothing imports it.

## 2. The rules

1. **Dependency rule.** Every `use` points toward `bao-core`. A crate may
   depend on anything _inside_ its layer, never on a layer outside it.
2. **Tokio rule.** `bao-core` has no `tokio` in its `Cargo.toml`. The domain is
   reusable by any client — including wasm — so its purity is a compile error,
   not a convention. This is _why_ `bao-wire` is a crate, not a `bao-core`
   module: framing needs tokio, and a module cannot enforce the boundary.
3. **Port-graduation rule.** The `SandboxBackend` and `Harness` traits live
   as modules inside `bao-daemon`. A trait moves into `bao-core` only when its first
   adapter becomes its own crate (because that adapter drags a heavy dependency
   or gains external consumers). No speculative ports in the core.
4. **`Transition` convention.** Every pure FSM follows `docs/design/
   state-machines.md`: a state enum + an event enum + one total `apply` + a
   lenient `fold` + legal/illegal cell tests.

## 3. The move map

| From                                                        | To                                                                                                                                                 |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bao-host`                                                  | `bao-daemon`                                                                                                                                       |
| `bao-agent` (Harness trait + Pi/Fallback)                   | `bao-daemon::harness`                                                                                                                              |
| `bao-client` (framing + Conn)                               | `bao-wire`                                                                                                                                         |
| `bao-cli`                                                   | `bao`                                                                                                                                              |
| `bao-core::session::Session` (live: PTY/process/log/screen) | `bao-daemon::session`                                                                                                                              |
| `bao-core::session::Manager` (registry + saga)              | `bao-daemon::manager`                                                                                                                              |
| `bao-core::session::SessionStore`/`StoredMeta`              | `bao-daemon::store`                                                                                                                                |
| `bao-core::screen` (vt100)                                  | `bao-daemon`                                                                                                                                       |
| `bao-core::sandbox` impls + `IsolationBackend`              | `bao-daemon::sandbox` (`SandboxBackend` trait + `InPlace`/`GitWorktree`)                                                                             |
| `bao-core::protocol` (types)                                | stays in `bao-core` (pure data)                                                                                                                    |
| `bao-core` keeps                                            | `Status`, `Alert`, `LifecycleEvent`+FSM, `SessionMeta`, `EventKind`, `Workspace`/`SandboxKind`/`SandboxSpec`, protocol types, value types, `error` |

The key split the rest of the table serves: **the live `Session` is not a
domain object; `SessionMeta` is.** Everything that produces `SessionMeta`
(process, PTY, log, restore fold) is the daemon's job; `SessionMeta` itself —
plus the state machine that decides its `Status` — is the domain's.

## 4. Naming decisions

- `bao-host → bao-daemon` — the crate is the process; name it for the process.
- `bao-agent → bao-daemon::harness` — harness adapters are used only by the
  daemon and there are two impls; a crate would be a boundary with nothing on
  the other side.
- `bao-client → bao-wire` — it was transport plumbing, not a product concept.
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
- `bao-wire` grows by verbs and transports, always versioned: new `Rpc`/`Reply`/
  `FromHost` variants. The transport seam itself is pinned in `channels.md` —
  an `Addr` enum (`Tcp | Unix`), socket-per-channel, and handlers generic over
  `AsyncRead`/`AsyncWrite`; remote (QUIC) is deferred but anticipated.
- `bao-daemon` is the growth home: new sagas (fork, move), new sandbox backends
  (`Bubblewrap`, `Landlock`, `Seatbelt`), new harnesses (`ClaudeCode`, `Codex`),
  later `machine/` and `peers/` modules. It grows by adapter modules, never by
  feature crates.
- Frontends are new crates (`bao-web`, `bao-mobile`), each thin, depending only
  on `bao-core` + `bao-wire`.

Move and fork are consequences of the event-sourced domain, not new
architecture: move = ship `meta.json` + `events.log`, re-fold on the target
daemon, resume via the harness's `pack`/`unpack`; fork = a new event, a new
stream prefix. The first release already paid for both.

**Resist:** per-feature crates (`bao-fork`, `bao-move`), I/O creeping back into
`bao-core`, speculative ports in the core, and a second god-crate (the daemon
grows only by adding adapter modules).

## 7. Implementation order

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
