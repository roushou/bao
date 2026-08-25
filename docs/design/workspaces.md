# Workspaces: where sessions are aimed

Pins the vocabulary and the model for **workspaces** — the user-declared
groupings that sessions are targeted at. Decisions here unblock targeted
launch, the hierarchical overview, and per-workspace context later.

## The problem

A session today is created wherever `bao launch` happens to run; its location
is an accident of the shell, not an intent. Creating a session "just to create
it" is useless — creation must be _targeted_: "run an agent on myapp", from
anywhere, in a few keystrokes. And once there are agents on several efforts at
once, supervision needs to zoom: all workspaces at a glance, then one
workspace's agents, then one terminal.

## Vocabulary

A workspace is **not** a git repo and **not** a directory of sessions. It is a
named root the user declares, under which any number of directories/repos may
live (`backend/`, `frontend/`, shared libs) and on which any number of
sessions may run.

| Level     | What it is                                                | Example                 |
| --------- | --------------------------------------------------------- | ----------------------- |
| Host      | A machine running the daemon                              | laptop, build-box       |
| Workspace | A named path the user registered as a target for sessions | `myapp` → `~/dev/myapp` |
| Session   | One live agent in one working copy (unchanged)            | pi refactoring the API  |

Hierarchy to memorize: **host → workspace → session → terminal**. Every level
is a real noun; no invented ones.

### Rename: sandbox `Workspace` → `WorkingCopy`

The grouping concept takes **workspace**, so the old type renames:

- `bao_core::sandbox::Workspace` → `WorkingCopy`
- `WorkspaceStore` → `WorkingCopyStore`; `<home>/workspaces/` →
  `<home>/working-copies/` (one-shot migration: if the old dir exists and the
  new doesn't, it is renamed)
- `SessionMeta.workspace` → `SessionMeta.working_copy` on the wire

Why this name wins: VS Code's _multi-root workspaces_ already taught
developers exactly this concept — several folders, one working context — so
the word transfers the right mental model in zero seconds. Runners-up were
_space_ (clean but semantically empty) and _project_ (wrongly implies a single
repo). The Cargo-workspace collision is contributor-scoped and
self-disambiguating; the product name is read by users every day.

## Registration

- `bao ws add <path>` registers a workspace: alias + resolved root path,
  stored daemon-side in `<home>/workspaces.json` (alongside `profiles.json`).
  `bao ws rm/list` manage the registry.
- Declared by path, not inferred: no magic scanning of `$HOME`. Fragile
  inference was rejected.
- A path only means something on the host that can see it, so registration is
  **per-host**: `bao ws add` registers on the daemon whose machine resolves
  the path. Aliases are how clients target workspaces without knowing hosts;
  alias collision across hosts is an error surfaced at launch time, never
  silently resolved.

## Targeting

`bao launch [WORKSPACE]` aims a session: resolve alias → host + root, then
proceed exactly as today (profile/command/isolation unchanged). No argument
and no cwd fallback ambiguity: when omitted, Bao asks (picker/palette) rather
than guessing. The current directory loses its privileged role — you can be
anywhere.

Context-awareness beyond the path (per-workspace default harness/profile/
isolation) falls out of the registry record later and is deliberately out of
scope for the first slice.

## The overview

```
┌─────────────────────────────────────────────┐
│ header                                      │
├──────────┬──[tab][tab][tab]─────────────────┤
│ sidebar  │        main view                 │
│ (hosts → │   (terminal pane, unchanged —    │
│ workspaces│    raw passthrough + emulator)  │
│ → agents)│                                  │
├──────────┴──────────────────────────────────┤
│ footer                                      │
└─────────────────────────────────────────────┘
```

- **Sidebar** replaces the Rail's flat list: workspaces as groups, their
  agents nested beneath, severity ladder intact. It owns navigation focus.
- **Tab bar** is chrome, not a pane — the visual echo of the open agents of
  the selected workspace. Selection lives in the sidebar; tabs own no keys.
- **Main view** is today's Terminal pane untouched. Focus contract survives:
  exactly one focused pane, keys never overlap.

Aggregate state derives upward: a workspace "needs attention" iff one of its
sessions alerts. Views never compute understanding themselves.

### Attach without leaving

Attaching is a subscription, not navigation. `Watch` already streams every
session's state globally; `Attach` is one byte-stream per session id. The TUI
holds **one emulator per open tab**, subscribing lazily when a tab opens and
dropping the stream when it closes; switching tabs swaps which emulator paints
into the main view. Resource use is bounded by open tabs; everything else
stays summary-state from `Watch`.

## Slices

1. `bao-core`: `WorkingCopy` rename (mechanical); `Workspace` registry record
   type (alias, root path) — pure domain, no I/O.
2. Daemon: `workspaces.json` store + resolution (alias → host + root) +
   `working-copies/` dir migration.
3. CLI: `bao ws add/rm/list` + targeted `bao launch [WORKSPACE]`.
4. TUI: sidebar hierarchy, then tabs over the existing terminal pane.

Each slice stands alone; targeted creation works from any terminal before the
TUI catches up.

## Decisions

- **workspace** for the grouping; sandbox type renamed to **working copy**
  (dir: `working-copies/`) so the word stays unambiguous.
- Registration by explicit path, per host; aliases are the client-facing
  handle.
- Launch targets a workspace; cwd loses its privileged role.
- Sidebar/tab-bar/main layout; tabs are chrome, selection lives in the
  sidebar.
- Attach is a per-tab subscription — emulators are bounded by open tabs.

## Open questions

- Do aliases live in one global namespace or per-host? (First cut: global
  uniqueness enforced across the registry files Bao sees; revisit with real
  multi-host usage.)
- Should a workspace record eventually carry defaults (harness, isolation)?
  (Deferred — let real use say.)
