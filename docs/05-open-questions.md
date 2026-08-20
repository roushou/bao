# 05 — Open questions

Deliberately undecided at the product level. Don't resolve these silently in a
spec — surface them, and where possible let real user feedback decide.

- **First paying customer: developer or non-developer?** The docs assume
  developer-first, but the answer shapes priorities and which views matter earliest.
  Decide with real users, not at the whiteboard.
- **How the overview is organized.** By _location_ (grouping sessions by which
  machine they live on — leans into what's unique about Bao) or by _task/goal_
  (grouping by what they're accomplishing — more legible to non-developers). This
  is quietly the same developer-vs-non-developer fork in another form.
- **How central "move it to another machine" is to the story.** It may be the
  dramatic demo, while everyday value comes from simply reconnecting from any
  device. Worth testing which one actually lands with users before leaning on it.
- **How many harnesses to support at launch.** pi is the first and deepest
  adapter; how many more ship with the first release is open.
- **Managed compute, ever?** The product is bring-your-own by principle. Whether a
  hosted-machine option is ever offered (as one more choice, never a requirement)
  is open and has real business-model consequences.
- **How much parallelism users actually want.** Whether "fork into two independent
  sessions" is a frequent need or a rare one affects how early it matters.
- **Should a stuck launch ever say "stuck"?** A session can sit in `starting`
  indefinitely if the harness never produces its first output — honest, but
  silent. Whether to add a bounded signal ("no output in 120s — still running")
  is a product call: it trades the never-misrepresent invariant against never
  leaving a dead-looking session hanging.
- **Is a failed launch worth keeping around to inspect?** Rollback removes the
  session and surfaces the reason as a transient toast (and, in the command
  center, the status line). A persistent `errored` row — readable and removable
  — preserves the "why" at the cost of session noise. User feedback settles which
  failure shape is right.
- **What should survive a daemon crash mid-launch?** A session caught in
  `preparing` folds to `interrupted` on restore and is cleaned on `rm`. Whether
  a half-built sandbox should instead be auto-compensated with a visible note is
  open.
