---
status: accepted
date: 2026-08-03
---

# An Ignore List That Means "Doesn't Exist," Not "Don't Show It"

## Context and Problem Statement

A real project isn't just the files someone wrote. It's also `.git`,
`target`, `node_modules`, and a handful of other directories that
tooling leaves behind — often larger, in file count, than the project
itself. None of that is what an agent working through this server
actually needs to see. Listing it anyway doesn't just clutter an
overview; it buries the files that matter under thousands that don't,
and it invites a tool call down into a `.git` object store or a
dependency tree that was never meant to be read directly. How should a
server built around one clear boundary — the workspace root — treat a
second, softer boundary around names nobody wants to look at?

## Decision Drivers

- Hiding something from a listing is worthless if a tool will still
  happily open it when asked by name — the two need to agree, or the
  hiding is theater.
- What counts as noise differs by project and by ecosystem; a fixed,
  unchangeable list would inevitably hide too little for some projects
  and too much for others.
- Whatever mechanism does the hiding shouldn't become a second place
  where "is this path allowed" gets decided, alongside the workspace
  boundary from ADR 0001 — two separate gates are two separate places to
  get the logic almost right.

## Considered Options

- A fixed list, filtered only in `tree`'s own output
- A configurable list, filtered only in `tree`'s own output
- A configurable list, enforced everywhere a path is resolved

## Decision Outcome

Chosen option: "A configurable list, enforced everywhere a path is
resolved", because a list that only changes what `tree` prints leaves
every other tool free to read, edit, or overwrite exactly what was just
hidden — which answers the wrong question. The list isn't there to make
a listing shorter. It's there to say that certain names aren't part of
the workspace an agent is meant to be working in, at all.

Concretely, this rides on the same mechanism ADR 0001 already
introduced: the moment a path is checked against the workspace root, it
is also checked against the ignore list, and a match is reported the
same way a path that never existed would be. There is exactly one gate,
not two, and every tool that resolves a path — not only `tree` — passes
through it.

### Consequences

- Good, because there is only one place "is this path visible" gets
  decided, shared by every tool, rather than a rule duplicated (and
  possibly drifting) across each one.
- Good, because the default list still covers the usual suspects out of
  the box, while remaining something a project can widen or narrow for
  its own layout.
- Good, because a name on the list and a name that was never there
  behave identically from every tool's point of view — nothing about the
  server's responses distinguishes "hidden" from "absent."
- Bad, because a project that genuinely needs to inspect something on
  the default list — a `.git` hook, a vendored dependency — has to
  explicitly reconfigure the server first; there's no per-call override.

### Confirmation

The workspace-path checking logic carries unit tests confirming that a
top-level ignored name is rejected, that a path nested underneath one
is rejected too, and that an ordinary path sharing no component with the
ignore list is unaffected. Because the check lives in the same function
ADR 0001 already covers, no tool can bypass it without also bypassing
the workspace-root check itself — the two are inseparable by
construction, not merely by convention.

## Pros and Cons of the Options

### A fixed list, filtered only in `tree`'s own output

The simplest possible version: `tree` hard-codes a handful of names and
skips them when building its listing. Every other tool is untouched.

- Good, because it needs almost no code and no configuration surface.
- Neutral, because it does genuinely make `tree`'s output easier to
  read, which was the original, narrower motivation.
- Bad, because "not shown" and "not accessible" are two different
  promises, and this option only ever makes the first one — an agent
  that spots `.git` mentioned anywhere else, or simply guesses at the
  name, can still read straight through it.

### A configurable list, filtered only in `tree`'s own output

The same cosmetic filter as above, but the list of names becomes a
server setting instead of a hard-coded constant.

- Good, because different projects can shape the listing to their own
  layout.
- Bad, because it inherits the same core problem unchanged: the list
  only ever governs what one tool prints, not what any tool can touch.

### A configurable list, enforced everywhere a path is resolved

Described above, under Decision Outcome.

- Good, because "hidden" and "inaccessible" become the same thing,
  which is the property that was actually wanted.
- Neutral, because it makes the ignore list dependent on the same
  central checking function as the workspace boundary — a dependency
  that's a feature here, not a coupling to be avoided.
- Bad, because it's slightly more code than a cosmetic filter would have
  been, for a payoff that only shows up once a tool actually tries to
  reach past the listing.

## More Information

This decision has no effect independent of ADR 0001 — the ignore list
is a second condition checked inside the same function that already
enforces the workspace boundary, not a separate mechanism running
alongside it.
