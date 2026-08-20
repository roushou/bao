# State machines: one shape, written down

Any finite state machine in Bao follows one convention. It is a _pattern_, not
a framework: the language already provides the machinery (enums, exhaustive
`match`, `Result`, `Iterator::fold`), so the convention just makes every
machine look identical and keeps each one obvious in its own module.

The reference implementation is the session lifecycle:
`bao-core/src/lifecycle.rs`.

## 1. The shape

Every FSM is five things, always in this form:

1. **A `State` enum** — serializable if it crosses the wire or disk, `Copy`
   when it's cheap.
2. **An `Event` enum** — the machine's _input alphabet_, one variant per real
   fact. Not the storage/wire type: the storage layer maps onto this alphabet
   (see `lifecycle_event` in `session.rs`).
3. **One total `apply(&self, event) -> Result<Self>`** — the whole machine in
   one function. Every legal edge is a named `(state, event)` pair; the last
   arm is `Err(IllegalTransition)`. No guards, no I/O, no context.
4. **A `fold`** — replay a stream of events over a starting state. Lenient: an
   illegal event keeps the previous state, so restore never fails on a corrupt
   log.
5. **Tests** — every legal cell and every illegal cell, plus a lenient-fold
   test.

The `Transition` trait (in `bao-core/src/lifecycle.rs`) is the anchor: it declares
`apply`/`fold`/`Event` so "what's a state machine here?" is answerable by
grepping for `impl Transition`. It carries no logic beyond the default lenient
`fold` — it is a naming contract, not a framework.

## 2. Two kinds of machine, one boundary

There are two kinds, and they must not be forced into the same trait:

- **Pure fold** (`Transition`): context-free `(state, event) → state`, event-
  sourced, replayable. The session lifecycle is one.
- **Guarded Mealy machine**: `(state, input, context) → (action, next state)`,
  where inputs carry guards ("Enter does X only when not filtering"). The
  overview's input/focus machine is one. It is _not_ a `Transition` —
  it has context and produces actions — and that is correct, not a gap.

The boundary: if a machine needs context, guards, or produces side-effecting
actions, it is a Mealy machine; give it its own module and a `route`/`step`
function. Only pure, context-free machines implement `Transition`.

## 3. The rule of three

Do not extract a generic state-machine framework. Each machine is hand-written
in its own module (an obvious, dedicated file). Generic code is extracted only
when a _third_ machine of the same kind appears — the first two rarely agree on
the real axis of reuse, and a framework built from one example fits nothing.

## 4. Where each machine lives

- Session lifecycle → `bao-core/src/lifecycle.rs` (the machine:
  `LifecycleEvent` + `impl Transition for Status`), with the generic
  `Transition` trait in `bao-core/src/lifecycle.rs`.
- Overview input/focus → its own `bao-tui` module (future; a Mealy
  machine, see §2).
