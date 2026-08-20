# 03 — Principles (invariants that must hold)

These bound the product, not the implementation. Any technical design is fine as
long as it keeps these true. They are acceptance criteria, not architecture.

## Product invariants

- **Bring-your-own compute.** Bao commands machines the user owns. It never
  requires the user to buy compute from Bao. (If managed compute ever exists, it
  must be one more option among the user's machines, never a requirement.)
- **Harness-agnostic.** Bao runs the user's choice of coding-session harnesses
  and adapts to new ones. It never ships or locks users to its own harness.
- **Your work follows you, not the machine.** A user can leave and resume a unit of
  work from a different device or machine with its state and conversation intact.
- **A unit of work is single and continuous.** It runs live in exactly one place at
  a time. Connecting from elsewhere joins the same one; it never silently forks into
  two diverging copies. Running two is possible only through an explicit fork that
  produces a clearly separate session.
- **Never misrepresent what an session is doing.** Status and activity shown to the
  user must reflect what is actually known. If the product can't truly tell what an
  session is doing, it says so rather than guessing. Trust dies the first time the
  overview lies.
- **Legibility over spectacle.** The the overview exists to direct alert to
  what needs a human and stay quiet otherwise — not to be busy or decorative. Every
  visual element should encode a real fact; if it encodes nothing, it's noise.
- **Resilience of state.** A user's work must survive the messy real world — a
  laptop sleeping, a network dropping, a machine restarting — without ending up
  corrupted or lost. Exactly how is your call; that it holds is not optional.

## Product-shape invariants

- **One product, in stages.** The first release is a focused developer tool that
  must stand on its own. Later stages (richer visual view, non-dev accessibility)
  are the same product growing — not a separate product and not a platform with
  something built on top. The later vision only tells you what to keep flexible.
- **First release must stand alone.** Test every proposed feature: _would a
  developer who never heard of the broader vision want this?_ If not, it's scope
  creep for the first release — defer it.
