# Channels: one logical stream per connection

A connection carries exactly one **channel** — a bidirectional byte stream
with its own backpressure and cancellation. Channels never share a socket;
each identifies itself in its first frame. Framing is unchanged per channel.

This record pins the channel model.

## 1. Decisions (locked)

1. **A channel is a bidirectional byte stream with its own backpressure and
   cancellation.** Three kinds today: `Control` (RPC request/reply), `Watch`
   (daemon-wide derived state), `Attach(session)` (one session's terminal
   bytes). Every channel identifies itself in its first frame.
2. **Local transport is one OS connection per channel** — a unix socket at
   `$BAO_HOME/daemon.sock` (mode 0600) by default, TCP behind an explicit
   `--port`. One socket per channel is cheap locally (tmux's control sockets
   work this way) and gives per-channel isolation in the kernel for free.
3. **Per-kind backpressure policy**:
   - `Control`: a bounded out queue (4096) behind a single writer task;
     every send is awaited. A stalled reader stalls only its own channel —
     replies queue up to the bound, never unbounded, and never block any
     other channel.
   - `Watch`: no per-channel buffer — writes go straight to the socket, so
     the transport's flow control is the backpressure. The bounded
     drop-oldest structure is the **state bus** (a 1024-slot broadcast):
     derived state is _replaceable_, so a lagging watcher just misses
     frames and re-syncs from the daemon's list — the newest picture, no
     backlog.
   - `Attach`: replay + live writes go straight to the socket; a stalled
     reader backs up only its own channel, and a dead one closes the
     channel — the client re-attaches. **Lossless because of snapshot +
     seq**: `attach_point()` returns a consistent `(seq, screen)` pair, so
     a re-attach replays the gap without loss or duplication. No new
     protocol messages.
4. **`Addr` is an enum** — `Tcp { host, port }` | `Unix(PathBuf)` — so
   addressing travels with the transport.
5. **QUIC/remote is deferred but anticipated.** The channel API is the seam a
   future quinn endpoint implements: locally `open_bi` dials a socket,
   remotely it opens a QUIC stream. The framed protocol does not change.

## 2. The wire shape

Framing stays exactly as-is: length-prefixed JSON, `FrameReader`/`FrameWriter`
over `AsyncRead`/`AsyncWrite`, typed `Request`/`Rpc` and `FromHost`. The only
addition is a **channel handshake**: the first frame on every connection names
its kind.

```
client                          daemon
  │  connect (socket)            │  accept
  │ ── ChannelHandshake ───────▶ │  dispatch:
  │                              │    Control  → RPC loop (request/reply)
  │                              │    Watch    → list + follow state bus
  │                              │    Attach   → snapshot+seq, then live
  │ ◀── frames for this kind ────│
  │  close socket                │  handler task ends (cancellation = close)
```

## 3. The daemon

`serve()` is an accept loop that reads the handshake and dispatches to a
dedicated handler task per channel. A channel's lifetime is its socket's —
closing it cancels the handler. The daemon keeps no per-client registry and
no shared outbound queue; each channel owns its writes. A control channel
tracks only the event-stream forwarders for sessions it launched or resumed
(aborted when the channel closes); a watch or attach channel owns no state
at all.

## 4. The client

`Conn` keeps its public surface (`connect`, `into_parts`, `info`, and every
RPC method) so the TUI and CLI do not change. Internally it owns a
`ConnWriter` (the control socket's write half, typed RPCs, and the channel
dialer) plus a merged event stream fed by a reader task per opened
`Watch`/`Attach` channel.

## 5. Adjacent boundaries (separate records)

- **Trust**: the daemon moves to a 0600 unix socket, verifies the peer (uid
  check), and reports its posture in `DaemonInfo`; clients refuse to `Launch`
  against an open daemon.
- **Durability**: the event log becomes a per-session writer task with a
  batched fsync policy, checksummed lines, and compaction.
- **Observability**: `eprintln!` gives way to structured `tracing` events at
  the channel/lifecycle boundaries.

## 6. Migration and testing

- `Addr::Tcp` keeps every existing test and CLI path working while the unix
  transport lands.
- New coverage per phase: in-process protocol E2E over `tokio::io::duplex`
  pairs (no daemon binary), a stalled-reader isolation test, a checksum
  salvage test, and a peer-refusal test.
- The real-binary E2E suite stays as the top smoke layer.
