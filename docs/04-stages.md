# 04 — Roadmap

## Scope

The first release (v0/v1) is a **local application**: the TUI client and the
daemon on the user's own machine(s). "Work from any device" (web/mobile
clients) is a later terminal pane; the architecture keeps it possible without rework,
but it is not part of the first release.

Each capability is built as the smallest thing that proves the next sentence
of the product, and must pass the same test: _would a developer who never
heard of the broader vision want this?_ If not, it does not belong in the
first release.

## Current state

The single-machine story is built and working:

- **One live session per unit of work.** Launch a coding-session harness (pi
  first) into an isolated working copy; any number of terminals can attach
  to the same live session, see its current screen, and send input. Detach
  (`^Q`) and reattach from anywhere — nothing is lost.
- **Survives restarts.** Sessions persist their record and event log; after a
  crash or reboot they come back with full history, honestly marked
  `interrupted` — or `damaged` when the record can't be read (salvaged,
  never dropped), and can be reviewed and removed.
- **Many sessions, one machine.** Each session runs in an isolated git worktree
  on its own branch, with a name and a place in the session list — sessions
  never step on each other.
- **Resume.** `bao resume` relaunches an interrupted session with its
  conversation intact (pi resumes via `pi --session <file>`).
- **Overview.** bare `bao` shows every session at a glance — alert
  signals derived only from facts (errored, interrupted, idle, done),
  last-output snippets, and drill-in to attach, resume, stop, or remove from
  the same screen.
- **One-action creation.** Create an session from the overview, or deploy one in
  the background without a terminal (`bao launch --detach`). Sessions can be
  renamed (`bao rename`, or `n` in the overview).
- **Honest alert.** The daemon polls each harness's status hook; the
  the overview shows "waiting for you" only when the harness can prove it — it
  never guesses what an session is doing.
- **Isolation choice.** `bao launch --isolation inplace|worktree` requests an
  isolation level; the daemon never silently delivers a weaker one.
- **Machine identity and capability negotiation.** `bao info` reports the
  host, protocol version, and available isolation backends; clients
  handshake before speaking.

## Remaining (single machine)

- **Fork.** A second, independent session created from an existing one —
  explicit, and clearly its own thing. (The one remaining item on the
  single-machine menu.)

## Later

- **Move a unit of work between machines** — relocate a running session to a
  stronger machine with code and conversation intact (same identity, source
  honestly marked `moved`). The sandbox and the harness pack/unpack hooks are
  its building blocks. Parked until the single-machine story matures.
- **Sessions across machines** — multi-machine list and overview.
- **Any-device clients** — web/mobile views. They reuse the core and the
  client library; they never drag in TUI dependencies.
- **Richer, more visual views** — the spatial map; non-developer
  accessibility. The same product growing.
- **Managed compute, if ever** — one more machine the user can point at,
  never a requirement.

## Build order

- One capability at a time; each ends runnable and testable on its own.
- The load-bearing risk of a capability is tested _first_ (a spike), before
  the surrounding machinery is built around it.
- Deferred items are deferred deliberately — "any device" and "move" are both
  in the product vision, but neither is in the first release's single-machine
  scope.
