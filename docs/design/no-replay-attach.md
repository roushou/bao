# No-replay attach: the daemon holds the screen

Attaching to a session never replays its byte history. The daemon keeps the
terminal's _state_ and hands the client a snapshot — exactly as tmux and
screen do. This is the decision most likely to be "simplified away" by a
future contributor, so here's the why.

## Why not replay

The tempting alternative is to replay the whole event log on attach
(`Attach { after: 0 }`). The cost: every attach — and every time the command
center's terminal re-selects a session — would scroll the session's entire
past across the screen. Replay is the symptom of storing _history_ instead of
_state_: a fresh emulator can only reconstruct the current screen by feeding
every byte through the parser from the start.

## The principle (borrowed from tmux)

tmux, screen, and wezterm never replay: the terminal emulator lives
permanently in the server, attached to the PTY, always holding the current
screen. On attach the server sends the **current screen**, not the bytes that
produced it. State is transmitted directly; history is paged only on demand.

## The shape

- **The daemon holds the screen.** Each `Session` owns a `Screen` — a
  `vt100::Parser` configured with no scrollback — fed on every output chunk
  in `Session::append`.
- **Attach sends a snapshot.** `Reply::Attach` carries `seq` (the event-log
  cursor the live stream follows from) and `screen` — the current screen
  rebuilt as a short byte stream via `Screen::repaint`, which delegates to
  `vt100`'s own `contents_formatted` (correct for attributes, wide
  characters, and cursor state by construction).
- **The client feeds the snapshot, then follows live.** A fresh emulator
  ingests the snapshot (arriving at the exact current screen) and then
  consumes `Output` frames from `seq` onward. No history, no scroll.

The snapshot is O(screen cells) — a few hundred bytes — not O(total output).

`launch`/`resume` are the one exception: a fresh or resumed session subscribes
from the log's start, so its short (or pre-crash) conversation replays once.
That's the resume story, not the cycling annoyance.

## Cost

- **Scrollback is per-session within a client's lifetime.** The `Terminal pane`
  component caches one emulator per session (one shared connection, output
  routed by `session` id), so cycling between sessions keeps each one's
  scrollback; a session attaches once and then just swaps in. History across
  restarts is a later, on-demand `Tail { session, before_seq }` RPC — the only
  thing replay would uniquely provide.
- The daemon pays one vt100 parser per session (cheap) and a ~Kb snapshot per
  attach; nothing else touches the wire.
