# The registry: one store of named things launches consume

Supersedes the registration model described in
[`workspaces.md`](workspaces.md) (that doc's layout/attach sections stand).

## The decision

There is **one registry**, not one per kind. It holds every named thing a
launch consumes, under **one alias namespace**:

```json
// <home>/registry.json
[
  { "alias": "myapp",  "kind": "workspace", "root": "/home/u/dev/myapp" },
  { "alias": "review", "kind": "profile",   "argv": ["pi", "--model", "fast"] }
]
```

- A **workspace** names *where* a session runs (a root directory).
- A **profile** names *what* runs (an argv preset).

Why one store: the two shared everything except their payload — persistence,
load lifecycle, listing, alias rules, RPC choreography, CLI shape. Two
registries would duplicate all of that per kind, and every future launch
parameter (default harness, isolation presets) would be a third copy. Now it's
a new [`EntryKind`] variant with zero new plumbing. A generic `Registry<T>`
was rejected: the enum-entry form needs no type-parameter machinery.

## Semantics

- **One namespace, enforced**: an alias is either a workspace or a profile,
  never both.
- **Put is upsert**: re-registering an alias replaces its entry — registries
  people edit behave like maps, not append-only logs.
- **Per host**: entries live in the daemon's home; a path or command only
  means something on the machine that runs it.
- **Validation at put time, by kind**: workspace roots must exist on this
  host (canonicalized); no two workspaces may claim the same root; profile
  argv must be non-empty.
- **Launch precedence**: explicit `--cmd` > `--profile` alias > default (`pi`).
  The most explicit thing the user said wins. This reversed an older behavior
  where `--profile` silently beat `--cmd`.
- **Typed refusals**: unknown aliases at launch time are `UnknownWorkspace` /
  `UnknownProfile`, never guesses.

## Migration

First load folds in the legacy files (`workspaces.json`, `profiles.json`),
writes `registry.json`, and removes them. Unparseable legacy profiles are
skipped honestly, not guessed.

## Mapping

| Concept        | Wire / code                                        |
| -------------- | -------------------------------------------------- |
| The store      | `bao-daemon::registry::Registry`, `<home>/registry.json` |
| An entry       | `bao-core::registry::RegistryEntry` / `EntryKind`  |
| Mutate         | `Rpc::RegistryPut` / `Rpc::RegistryRemove`         |
| List           | `Rpc::RegistryList` → `Reply::Entries`             |
| Product nouns  | `bao workspace …`, `bao profile …` (thin fronts)   |

[`EntryKind`]: ../crates/bao-core/src/registry.rs
