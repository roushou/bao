# Event-sourced session lifecycle

A session's life is a finite state machine, and its history is an
append-only log of facts. State is never stored — it is folded from the log.
Launch is a backgrounded saga: it registers the session first (visible as
`preparing`), then walks two side-effecting steps with compensation, and the
the overview watches it advance `preparing → starting → running`. This record pins
the states, the events, the fold, the saga, and every surface they touch.

This is the technical contract for the "no dead waiting time at create" work.
It completes an architecture the code already half-has — the vt100 screen is
already a fold (`screen.process(bytes)`), `events.log` is already an
append-only log, and `signal()` is already a projection table. This record
makes the _write_ side as disciplined as the read side already is.

## 1. Decisions (locked)

1. **Backgrounded saga.** `Launch` registers the session and returns
   immediately; the sandbox+spawn work runs in a background task; success and
   failure arrive through the Watch stream, not the RPC reply.
2. **First-class states.** `Preparing` and `Starting` are real `Status`
   variants the daemon says — never guessed client-side from
   `running + no output`.
3. **Select + dock on create.** The TUI selects the new session and shows it
   booting in the docked terminal pane; it stays in the overview.
4. **Rollback + toast on failure.** A failed launch compensates and the
   session disappears (`absent`); the TUI shows a toast with the reason.
5. **Minimal event-store scope.** Lifecycle events join the existing log; no
   generic framework, no snapshots, no schema-versioning ceremony.

## 2. The lifecycle state machine

One machine, one source of truth, owned by `bao-core`. States (extending the
existing `Status` enum, which already has `Running`, `Exited(Option<i32>)`,
`Interrupted`, `Damaged`, `Moved`):

```
Absent ──Created──▶ Preparing ──Status(Starting)──▶ Starting ──first Output──▶ Running
                       │                            │                             │
                 sandbox fails                 spawn fails                  Status(Exited)
                       ▼                            ▼                             ▼
                   (compensate)               (compensate)                   Exited(code)
                       └───────────────▶ Absent ◀──────────────┘
```

`Absent` is the pre-registration state (never persisted). `Preparing` =
"registered, sandbox not yet built." `Starting` = "process spawned, awaiting
first output." `Running` = "producing output" (the steady state).

The transition table — the single `apply(state, event) -> state`, the only
place a `Status` may change:

| From                 | Event                  | To             | Note                                    |
| -------------------- | ---------------------- | -------------- | --------------------------------------- |
| Absent               | `Created`              | `Preparing`    | registration (the fold's seed)          |
| `Preparing`          | `Status(Starting)`     | `Starting`     | saga step 2 succeeded                   |
| `Starting`           | first `Output`         | `Running`      | boot complete                           |
| `Starting`           | `Status(Running)`      | `Running`      | defensive (the logged transition)       |
| `Running`            | `Output`               | `Running`      | steady state                            |
| `Starting`/`Running` | `Status(Exited(code))` | `Exited(code)` | process exit (also before first output) |
| `Interrupted`        | `Status(Starting)`     | `Starting`     | resume                                  |
| any live state       | `Status(Interrupted)`  | `Interrupted`  | restore injection (process gone)        |
| any                  | `Status(Damaged)`      | `Damaged`      | unreadable meta                         |
| any                  | `Status(Moved)`        | `Moved`        | future (relocation)                     |

Illegal transitions (rejected by `apply`, replacing today's scattered guards):
`Status(Starting)` from any live state (`AlreadyRunning`), `Output` from
`Preparing`/`Interrupted`/terminal states (`NotRunning`), resume from anything
but `Interrupted`. The `child.is_some()` guard in `start_process` and the
`status != Interrupted` guard in `resume` collapse into this table.

Consequences:

- `Preparing` and `Starting` are **persistent**, not timed. `Starting` sits
  until first output, process exit, or an explicit stop/rm. It never guesses
  "stuck": the UI shows age (`starting · 45s`), which is honest about slowness
  without claiming failure.
- `idle` and `waiting` stay **orthogonal projections of `Running`**, not
  lifecycle states (they are qualities, not stages).

## 3. Events as the source of truth

The log is authoritative; `meta.json` keeps identity; state is derived.

**Event vocabulary** (extends the existing `EventKind`):

```
EventKind::Output(Vec<u8>)      // unchanged; the first one fires Starting → Running
EventKind::Status(Status)       // existing; now also persisted (was broadcast-only)
```

`Created` is _not_ a persisted event — it is the fold's seed (a readable
`meta.json` for an id means "registered", so the initial state is
`Preparing`).

**`events.log` format** carries a `kind` discriminator and a checksum:

```jsonl
{"seq": 1, "ts": …, "kind": "status", "status": "starting", "crc": …}
{"seq": 2, "ts": …, "kind": "output", "data": "…", "crc": …}
```

`load_log` skips unparseable lines, lines whose checksum does not hold, and
lines with an unknown `kind`.

**`meta.json` split.** Identity facts stay in `StoredMeta` (name, args, cwd,
created, sandbox); `status` is never persisted — the log is the sole source
of truth for lifecycle state. Derived facts (alert, snippets, idle) were
never persisted and remain so.

**Restore = fold + honesty rule.** On daemon restart:

1. Fold `events.log` from the `Preparing` seed to derive the last lifecycle
   state.
2. Apply the honesty rule: a fold that ends in a _live_ state
   (`Preparing`/`Starting`/`Running`) on a fresh restore is `Interrupted` —
   the process is gone.
3. Unreadable `meta.json` → `Damaged` (unchanged; the log is kept).

## 4. The launch saga (backgrounded)

Launch is a two-step saga with compensation. It is the impure process; the FSM
is the pure state. The saga never sets state directly — it performs side
effects and emits events, and the FSM advances.

```
Rpc::Launch
  ├─ validate command + cwd                 (synchronous; fail → Err, no session)
  ├─ Manager::begin_launch(id, …)           (register: meta.json, status=Preparing,
  │                                          publish State, insert into map)
  ├─ Reply::Launch { session: Preparing }   (immediate)
  └─ spawn saga task:
       step 1  prepare sandbox              (spawn_blocking: git worktree add)
       step 2  spawn harness                (PTY + process; emit Status(Starting))
       await first output                   (no side effect; a marker)
       on step N failure → compensate 1..N-1 in reverse → emit Gone { reason }
```

| Step              | Forward                                              | Compensation                                                               |
| ----------------- | ---------------------------------------------------- | -------------------------------------------------------------------------- |
| 1. SandboxBackend | `SandboxBackend::prepare` (`git worktree add` today) | `SandboxBackend::teardown` (`worktree remove` + `branch -D` + delete tree) |
| 2. Spawn          | open PTY, spawn harness                              | kill child, close PTY                                                      |

This is the `SandboxBackend` trait — a port with `prepare`/`teardown`, so
inplace/worktree/bubblewrap/seatbelt are adapters the saga is generic over.
The current
`Manager::create` (sandbox-then-spawn, synchronous) is replaced by
`begin_launch` + the saga task; the sandbox's `teardown` is a first-class undo
on the retained `Sandbox`, not just an `rm` path.

The daemon's PTY pump, `append`, `attach_point`, and `input`/`resize` are
unchanged — the saga only changes _when_ `start_process` runs (backgrounded)
and _what_ state the session is in while waiting.

## 5. Wire & types

- **`Status`**: add `Preparing`, `Starting`. Serde is automatic
  (`rename_all = "snake_case"`); `Display` gains both.
  `AlertInput::alert()` returns `None` for both.
- **`Reply::Launch`**: shape unchanged (`{ session: SessionMeta }`), but now
  returns the `Preparing` meta immediately.
- **`FromHost::Gone`**: new variant `Gone { session: SessionId, reason:
  Option<String> }`, pushed to `Watch` subscribers when a session is rolled
  back or removed — so every client sees removals, not just the one that
  issued `Rm`. (This also closes the pre-existing gap where a second client
  never learns a session was removed.)
- **State bus**: `Manager`'s `state_bus` carries a `StateEvent` enum instead
  of bare `SessionMeta`:

  ```
  StateEvent::Snapshot(SessionMeta)           // existing path
  StateEvent::Gone { session, reason }        // new
  ```

  `Watch` maps `State → FromHost::State` and `Gone → FromHost::Gone`.
  Future facts (`Forked`, `Moved`) are new variants, not new channels.
- **Attach on a not-yet-spawned session** must succeed: `Attach` to a
  `Preparing`/`Starting` session returns an empty screen snapshot and follows
  the live stream; bytes arrive when the harness spawns. (`attach_point`/
  `snapshot_and_last` already tolerate an empty log; registration must also
  create the screen parser, which today only `spawn`/`build_restored` do.)

## 6. The TUI

- **Non-blocking create.** `App::create` sends `Rpc::Launch` and receives the
  `Preparing` meta back quickly (registration only). It then selects the new
  session, attaches it docked (`ensure_terminal`), and returns to the render
  loop — no frozen frame. `preparing → starting → running` and any `Gone`
  arrive via the existing `handle_host` path.
- **The `preparing`/`starting` signal** — triple-encoded, in the three fixed
  places (header strip, rail left edge, terminal pane headline), sharing the existing
  one-table discipline (`signal.rs`, `theme.rs`):

  | State       | glyph                   | color    | word        |
  | ----------- | ----------------------- | -------- | ----------- |
  | `Preparing` | `○` (hollow; ASCII `o`) | `dim`    | `preparing` |
  | `Starting`  | `○` (hollow; ASCII `o`) | `accent` | `starting`  |

  Rationales: `"launching — building the sandbox"` / `"launched — waiting for
  first output"`. `Glyphs` gains one `hollow` entry; no new palette token is
  required (`dim` + `accent` exist). `Group::of` puts both in `Working`;
  `edge_glyph()` and `action_hint()` gain arms (`→ preparing…`, `→ starting —
  waiting for first output`). `NO_COLOR` and `BAO_ASCII=1` degrade as today.
- **First output = the flash.** `Starting → Running` is a fact that just
  happened: the header cell blinks for a beat and a `started <name>` toast
  fires — the existing flash, never an idle animation.
- **Rollback feedback.** On `FromHost::Gone { session, reason }`, the TUI
  removes the row (idempotently — an already-removed row is a no-op) and, when
  it was this client's launch, shows a toast `launch failed: <reason>` and
  sets the status line to the same reason (so it outlasts the toast).
- The footer prompt clears in the same frame the submit is accepted, so the
  user sees `preparing…` immediately, not a stale cursor.

## 7. Restore

- A `Preparing` session caught by a daemon restart folds to `Interrupted`
  (honesty rule). Its sandbox may be half-built; `rm` already runs the
  worktree compensation, so it is cleaned on removal.
- `PROTOCOL_VERSION` bumps (new `Status` values and `FromHost::Gone` change
  the wire). A mismatched client refuses to talk rather than misparse
  (existing handshake behavior).

## 8. Test strategy

- **`bao-core` — the transition table.** A test per legal cell and per illegal
  cell (e.g. `Status(Starting)` from `Running` is rejected, `Output` from
  `Preparing` is rejected, resume from `Running` is rejected). Pure and total:
  no I/O.
- **`bao-core` — fold & restore.** Fold a synthetic log to `Preparing`,
  `Starting`, `Running`, `Exited(code)`; verify live-states → `Interrupted`
  on restore; verify unreadable meta → `Damaged`.
- **`bao-core` — saga rollback.** Spawn fails after worktree created ⇒ worktree
  removed, session gone; sandbox fails ⇒ nothing leaked; success ⇒
  `Preparing → Starting → Running` in order.
- **`bao-daemon` — stream.** `Watch` emits `State(Preparing)` then
  `State(Starting)` then `State(Running)`; a failed launch emits `Gone`.
- **`bao-tui` — projection.** `signal()` for `Preparing`/`Starting` returns
  the right glyph/color/word, ASCII fallback, and `NO_COLOR`; the `Gone`
  handler is idempotent; create selects + docks the new session.
- **Contract.** A test that a new harness (fake) and a new sandbox backend
  (fake) can be dropped in without changing the FSM or events (the "swap
  everything" check).

## 9. Build order

Each slice ends runnable and testable:

1. **`bao-core` — states + fold.** Add `Preparing`/`Starting` to `Status`
   (serde, `Display`, alert), write the `apply` transition table, route
   every mutation through it, persist `Status` events to the log, fold on
   restore with the honesty rule. Existing tests updated.
2. **`bao-core` + `bao-daemon` — backgrounded saga.** `begin_launch` +
   compensation + `StateEvent::Gone` + `FromHost::Gone` + `Watch` mapping.
   `SandboxBackend` port extracted into `sandbox/`.
3. **`bao-tui` — feedback.** Non-blocking create with select+dock, the
   `preparing`/`starting` signal + `hollow` glyph, first-output flash, and
   `Gone` rollback toast.

## 10. Tradeoffs

- **State is derived, not stored.** Single source of truth and honest recovery,
  at the cost of a re-read-and-fold on restore. Bounded today (`LOG_CAP`
  in-memory ring; restore already slurps the file), worth revisiting only when
  logs grow.
- **Launch is asynchronous.** The user is never blocked, and failure timing
  becomes stream-driven — the cost is that `bao launch` (CLI) and any future
  caller must tolerate attaching to a `Preparing` session and learning failure
  asynchronously.
- **No timeout on `Starting`.** Honest (we can't distinguish "slow" from
  "stuck") but it can linger; mitigated by showing age and by `s`/`d` still
  working. An explicit supervision timeout is the escape hatch if it ever
  matters (see §11).

## 11. Open questions

1. **Supervision timeout for `Starting`?** Recommendation: none for now —
   show age. If added, it must be worded as a fact ("no output in 120s —
   still running"), never "stuck".
2. **`bao launch` (CLI) behavior.** Attach immediately to the `Preparing`
   session and watch boot (consistent with the TUI), or wait for `Running`
   before dropping into the terminal? Recommendation: attach immediately.
3. **How durable should the rollback reason be?** Toast fades; status line
   persists until the next action. If inspectable failures matter, a future
   `errored` row is a one-variant change (the FSM already has `Exited`), not
   an architecture change.
4. **Persisted `Preparing` on crash.** Folded to `Interrupted` today, cleaned
   on `rm`; acceptable, but worth a user-visible note if it happens often.
