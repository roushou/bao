# AGENTS.md

> **For AI coding agents.** This file sets the working contract for AI
> agents contributing to this repo. Humans should start at
> [`README.md`](README.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md) instead.

## You own the technical design

These docs define the product, not how to build it. Architecture, data model,
interfaces, protocols, language, and libraries are all yours to decide and spec.
Don't look here for those answers — there aren't any on purpose.

## What to hold to

- Deliver the behaviors in `02-capabilities.md`.
- Keep every invariant in `03-principles.md` true. These are outcomes, not
  implementations — meet them however you judge best.
- Scope the first build to `04-stages.md`. If a task doesn't serve the first-release
  wedge, question it.
- Treat `05-open-questions.md` as genuinely open. Don't quietly pick an answer;
  surface the decision and, where it matters, prefer letting real user feedback
  settle it.

## How to work

- Produce your own technical spec before building, and state the significant
  tradeoffs behind big choices so they can be reviewed.
- Prefer the smallest thing that proves the value. The first release exists to
  validate that "your work isn't trapped where it started" is real — not to be
  complete.
- Flag anything in these docs that's ambiguous or that you think is wrong, rather
  than working around it silently.
