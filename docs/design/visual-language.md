# The visual language: reading becomes seeing

The overview's severity and
alert signals are _shapes, color, and position_ — not _words_ —
the eye answers "what needs me" without parsing a sentence.

## 1. The idea

`1 needs you · 2 working · 1 done` is a sentence you have to read. The same
facts, rendered as `bao █▏▏·`, answer the question in one glance. The rule:
**every alert fact gets a visual shape at a fixed place; words are
annotation, never the carrier.**

## 2. The three fixed places

Color and shape appear at exactly three locations — nowhere else — so every
colored pixel means "look here":

1. **The header status strip** — one severity cell per session, ordered by
   urgency: `█` urgent (red errored, magenta damaged, cyan waiting, amber
   interrupted), `▌` idle (amber), `▏` running (green), `·` done (dim).
   The shape _is_ the sessions' health.
2. **The rail's left edge** — the same severity ladder, one bar per row. A
   vertical scan reads as a ranked urgency list.
3. **The terminal pane's headline edge** — the selected session's severity bar, so the
   rail and the terminal pane speak the same visual dialect.

Everything else — the session's words, metadata, verbs — is monochrome: bright
content, dim context.

## 3. The devices

- **Severity ladder (shape × color).** Full block = urgent, half = idle,
  thin = running, dot = done; the color names the kind. Two channels, one
  meaning.
- **Idle as cooling.** A running session's rail text dims as it goes quiet
  (bright → gray → dim), so "this one has gone cold" is felt, not read. The
  exact seconds stay in the terminal pane.
- **The flash.** When an session _enters_ the alert state, its header cell
  blinks for a beat and a toast announces it — a fact that just happened,
  never an idle animation. Nothing else moves.
- **Done recedes.** Finished sessions are a faint dot in the strip and a dim
  row in the rail — quiet, but still selectable.

## 4. Wording

The header carries no words at all; the rail title
reads `sessions (n) · k alert`. "Alert" matches the code's `Alert`
model and is severity-agnostic; the _precise_ verb lives in the terminal pane's
action hint (`→ waiting for you`, `→ exited with code 1`, `→ needs resume`).
The word is `running`, matching `Status::Running`.

## 5. The token map and glyph fallback

- **`theme::palette()`** — one color per _meaning_ (`waiting`, `errored`,
  `idle`, `healthy`, `dim`, …), resolved once. `NO_COLOR` maps every token to
  the terminal default, degrading the UI to monochrome in one line; a future
  light theme or user theme is a second table. `signal.rs` asks the
  palette instead of writing literal colors.
- **`theme::glyphs()`** — one glyph per _shape_ (`full`, `half`, `thin`,
  `dot`, `rule`, `vline`, `arrow`). On a terminal that cannot do Unicode
  (`BAO_ASCII=1`, `TERM=dumb`) it returns ASCII stand-ins (`#`, `-`, `|`, `.`,
  `>`) so the shapes survive. The language is still triple-encoded, so color
  and words carry the meaning even when the glyphs are plain.

## 6. Out of scope

The needs-you transition detection and flash are wired; the reconnect state
and rail overflow scrolling remain. Scrollback is the live window; a `Tail`
pager is the future on-demand history (see `no-replay-attach.md`).
