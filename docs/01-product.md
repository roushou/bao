# 01 — Product

## The bet

Coding is shifting from one person in one checkout to one person orchestrating many
AI agents at once, each working on its own task. The moment you have a team of
sessions instead of a single helper, two problems appear that nothing solves well
today: _where does each session run_, and _how do you keep track of them all_. Bao
is the tool for that world.

## The problem today

- An session started on your laptop is stuck on your laptop. Want it on a stronger
  machine and the experience falls apart (SSH in, different tools, lost state).
- Running several sessions means several scrolling walls of text — impossible to
  supervise at a glance.
- Your work is trapped where it started. Close the laptop and the session is gone
  or frozen; you can't pick it up elsewhere.

## What Bao is

An overview that lets a developer:

- put each session on whichever of _their own_ machines fits the task,
- work with any session from any device, without losing its state,
- see all your agents at a glance and know which sessions need them.

## Who it's for

- **First: developers** running multiple sessions. Everything in the first release
  serves them. Bao must succeed as a standalone developer tool on its own merits.
- **Later: non-developers**, reached through richer, more visual views. This is a
  direction that shapes what stays flexible — not a separate product.

## What makes it different

1. **Bring-your-own compute.** Bao commands machines the user already owns. It is
   not a cloud reseller — no lock-in, no middleman charging for compute.
2. **Harness-agnostic.** It works with whatever coding-session harnesses the user
   already uses, and with new ones as they appear. Bao is the neutral layer they
   run inside; it never ships its own harness.
3. **Your work follows you.** Not the machine, not the window — the living session,
   across devices and locations. This is the capability competitors don't nail.

## What Bao is not

- Not a terminal (it uses one as a detail view, not as the main interface).
- Not an AI session (it runs other people's).
- Not a cloud/compute provider (it commands the user's own machines).
- Not a generic session dashboard. Its edge is making _location_ and _coordination_
  legible — which it can do because BYO compute and neutrality are core to it.
