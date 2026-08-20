# 02 — Capabilities

What the product does, described as behavior. How to build any of it is open.

## Bring your own machines

A user registers machines they control (laptop, VPS, others over time) as places
sessions can run. Bao runs sessions _on those machines_; it never requires renting
compute from Bao.

## Run an session in a sandbox

A user launches a coding-session harness (pi, Claude Code, Codex CLI, …) into a
sandbox on a location of their choosing. Each session gets an isolated
working copy of the code so multiple sessions don't step on each other, and the
user can choose the isolation level — from running in place to an isolated
git worktree — with the daemon never silently delivering a weaker level than
requested. Launching can attach the user to the live session, or deploy the
session in the background without a terminal.

## Work with an session from any device

A user can connect to any of their running sessions from any device — laptop, phone,
desktop. More than one device can be connected to the same live session at once; all
see the same activity and any can interact. Connecting is viewing/participating; it
never spins up a duplicate.

## One live session per unit of work

A given session runs in exactly one place at a time. Opening it "again" somewhere
else connects to the same running session rather than creating a second copy. This
keeps a unit of work single and continuous — its code and its conversation never
silently split into two diverging realities. If a user genuinely wants two, they
**fork** it into a new, independent session that is clearly its own thing.

## Move a unit of work between machines

A user can relocate a running unit of work to a different machine — e.g. off a
laptop onto a stronger box. It comes back with its working state and its
conversation intact, so the user doesn't re-explain context or lose progress. (The
everyday version of this value is simply reconnecting from another device; moving
to a different machine is the more dramatic case of the same idea.)

## See all your agents at a glance

A user can see all their sessions at once: what each is doing, which need
alert, and where each one lives. Alert signals derive only from facts
(status, exit code, idle time) — and when a harness can honestly report that
it is waiting for the human, the overview says so directly. The point is to
supervise many sessions without reading many walls of text — surface the ones
that need a human, stay quiet about the rest.

## Drill into one session

From the overview, a user can open the detailed view of any single session. For
coding agents today that's the full terminal view. The product is built so other
kinds of detailed views can exist later without reworking everything.

## Overview — later, richer

The at-a-glance overview grows into a genuinely visual command view. The intended
"beloved" expression is a spatial map of the user's own machines, with sessions shown
living and acting on them — where an session stands and what it appears to be doing
both carry real meaning, and moving a unit of work to another machine is something
you can watch happen. This is a later terminal pane, not the first release.

## Non-developer accessibility — later

Over time, richer views (including live previews of what sessions are building) make
Bao usable by people who don't live in a terminal. Later terminal pane; it informs what
stays flexible now.
