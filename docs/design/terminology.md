# Terminology: model, harness, session

Pins the vocabulary so the
wire, the invariants, and future views speak the same language.

## The three levels

| Level       | What it is                                                                                        | Examples                                             |
| ----------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| **Model**   | The LLM — weights, no behavior of its own                                                         | Claude Sonnet, GPT-5.x, pi's backend                 |
| **Harness** | The program that runs the agentic loop: tools, terminal access, file editing, session persistence | pi, Claude Code, Codex CLI, Cursor agent mode, Aider |
| **Agent**   | The _running instance_: harness + model + working copy + conversation, doing one task             | one live pi process working on a repo                |

pi, Claude Code, and Codex are **harnesses**. The **agent** is the live unit
Bao supervises — exactly one per Bao session.

## Why this line

1. It resolves the built-in ambiguity of "claude" (model vs Claude Code vs the
   running instance) and "codex" (model family vs Codex CLI).
2. It matches the product's own shape: Bao is harness-agnostic (never ships a
   harness), model-agnostic by construction (the model lives inside the
   harness), and what Bao supervises is a set of _agents_ (instances).
3. "Supervise your AI agents" survives — the phrase refers to the _agents_
   (instances) Bao supervises, not the tools or models.

## Mapping

| Concept                      | Wire / code                                                   | Product / docs           |
| ---------------------------- | ------------------------------------------------------------- | ------------------------ |
| The running unit of work     | session (`SessionId`, `SessionMeta`)                          | session                  |
| The tool inside it           | harness (`SessionMeta.harness`, `Harness` trait, `--harness`) | harness                  |
| The model inside the harness | —                                                             | model (invisible to Bao) |

## Host vs daemon

A different axis, pinned to stop the drift between the docs and the wire:

| Term       | Meaning                                                                         | Where it appears                                                                                    |
| ---------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| **Host**   | The server end of the wire, and the machine that runs it — the client/host pair | `bao-daemon` crate, `FromHost`, `HostMsg`, `Addr.host`, `Hostname`, `SessionMeta.host`              |
| **Daemon** | The _process behavior_ of that host (background, always-on)                     | the `bao daemon` command (`bao daemon` = "run the daemon", the Unix `X daemon` naming like `httpd`) |

The docs' "daemon" is the process; the crate and wire say "host" (the role).
`bao daemon` is the command that runs the daemon — a VPS deploy is just `bao daemon` on
that machine.

## Discipline

Precise in Bao's own docs, wire, and invariants; loose in speech (industry
still says "coding sessions" for the tools, and that's fine). When in doubt:
harness = the tool you install, session = the thing Bao supervises.
