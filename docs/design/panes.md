# Panes: focus-driven composition with per-pane keybindings

The overview is a
composition of focusable **panes**; exactly one pane owns the keyboard at a
time, and each pane defines its own keybindings so they can never overlap.
This is the tmux/neovim model — and it is what lets each pane grow more
powerful without the others losing keys.

The bindings themselves live in one declarative table (`bao-tui/src/keys.rs`):
key → action → label → help group. Routing resolves against that table; help
and footer hints render from it, and its tests assert no two bindings in a
scope share a key — this section's "never overlap" is an executed assertion,
not prose. Text entry (filter input, prompts) and raw terminal passthrough
deliberately stay outside the table.

## 1. The model

```
App
├── header / footer            (chrome — never focused)
├── Rail  pane                 keymap: navigate, act, open overlays
├── Terminal pane              keymap: raw byte passthrough + step-out
└── overlays (palette/help)    modal panes (future: same treatment)
```

- **One focused pane.** Keys route to the focused pane only. The others keep
  rendering (the terminal still shows the harness's live screen while the
  rail has focus — you watch it, you're not inside it).
- **Per-pane keybindings.** Each pane owns its keys; nothing is shared. `⌃q`
  means _quit_ in the rail and _step out_ in the terminal. The same key,
  different verbs, no ambiguity — because focus decides.
- **Panes return actions.** A pane never reaches into another pane. It
  returns a small `Action` (focus me, fullscreen the terminal, quit, toast)
  and the `App` performs it.

## 2. The two panes

### Rail (browse)

Keymap — navigation and acting on the _selected_ session:

`j/k/↑/↓` move · `g/G` first/last · `Enter` fullscreen the terminal ·
`Tab` step into the terminal · `c/r/s/n/d` create/resume/stop/rename/remove ·
`/` filter · `⌃p` palette · `?` help · `⌃q` quit.

The rail renders the severity ladder (left edge), the header status strip,
and owns its modal overlays (palette, help, filter, prompts). Its `handle_key`
returns an `Action` for anything that crosses a pane boundary.

### Terminal (the harness, natively)

The terminal pane **is** the running harness instance — a raw PTY attach:

- **Output** — the daemon's byte stream feeds a real vt100 emulator; the
  harness draws its own screen, echoes your typing, edits its own lines.
- **Input** — raw: each keystroke is encoded to the bytes the terminal would
  send (`\r`, `\x7f`, `\x1b[A`, Ctrl-codes) and forwarded to the PTY
  _immediately_. Paste is forwarded verbatim. There is no line buffer, no
  "send on Enter", no Bao-side echo — the harness's input machinery is the
  truth.
- **Step out** — `⌃q` returns focus to the rail; it is the one key never
  forwarded. `PgUp/PgDn` scroll Bao's scrollback (the emulator's history),
  so the arrows stay with the harness where they belong.
- **Always attached.** The terminal follows the rail's selection: selecting
  an session re-attaches the pane to that session's PTY (the daemon's single
  live instance — never a second process).

### Sizes

One terminal, three sizes, same pane: **docked** (beside the rail, the
default), **fullscreen** (`Enter`), and the harness's own size is whatever
the PTY says. `⌃q` unwinds: fullscreen → docked → rail focus.

## 3. Key routing contract

```
on key:
  1. overlays (palette/help/filter/prompt/confirm) handle their own keys
  2. else the keymap resolves for the focused scope:
       Rail     -> keys::Keymap::resolve(Rail, key)    -> Action (applied by the Overview)
       Terminal -> Terminal::press(key) -> Keypress;   (pure decision —
                   StepOut crosses a boundary as Action::StepOut, Send bytes
                   are delivered by the caller at the shell)
  3. the app performs the resulting Action
```

The bindings live in one declarative table (`bao-tui/src/keys.rs`); help and
footer hints render from it. Text entry (filter input, prompts) and raw
passthrough stay outside the table by design.

Global keys are deliberately absent (except resize, which is not a key):
every key is owned by exactly one pane. A later pane (inspect, log, map)
drops in with its own keymap and zero collisions.

## 4. Decisions

- **Raw input = re-encoded crossterm events**, encoded by `term/encode.rs`
  to the exact byte sequence a terminal would send — honoring the modes the
  harness set on its output (application cursor keys, bracketed paste).
  Paste is wrapped iff the harness asked for it. (Reading the raw stdin fd
  directly is the faithful refinement for exotic keys — a documented
  follow-up.)
- **`PgUp/PgDn` scroll Bao's scrollback**, a deliberate tradeoff: harnesses
  rarely need them; the arrows stay with the harness.
