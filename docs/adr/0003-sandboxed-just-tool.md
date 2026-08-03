---
status: accepted
date: 2026-08-03
---

# A Command Runner That Can't Redraw Its Own Cage

## Context and Problem Statement

Every language a workspace might contain comes with its own build tool,
its own test runner, its own dialect of "how do I run this." Giving the
agent a native tool for each one doesn't scale, and it never will —
there is always another ecosystem the server hasn't heard of yet. `just`
already exists to solve exactly this for humans: one command runner, one
file of recipes, any toolchain underneath. Handing that same door to the
agent solves the scaling problem in one stroke.

But the `justfile` a project already trusts `just` to read is, from the
server's point of view, a file like any other — one the agent can open,
and, without a rule saying otherwise, one it can edit. A recipe is just
text until something runs it, and the same tool being asked to run a
recipe is perfectly willing to be the one that also wrote it. An agent
that can add a line to a `justfile` and then invoke that line has, in
effect, been handed a second, unmonitored way past every boundary ADR
0001 and ADR 0002 already built — not by breaking those boundaries, but
by convincing the workspace's own trusted files to step through the
door on its behalf. And the file's contents are only half of it: even a
`justfile` left untouched can be pointed at from somewhere else entirely,
if the tool invoking it also lets the agent choose which file, and which
directory, that invocation runs against.

## Decision Drivers

- What a `justfile` contains is a project's own business — a reckless
  recipe someone wrote by hand is a risk the project chose. What the
  server must never allow is the agent choosing that risk on the
  project's behalf, quietly, from inside a session.
- A safeguard that only holds for files the agent edits through this
  server, while leaving `just` itself free to be pointed anywhere, isn't
  a boundary — it just moves the gap somewhere less obvious.
- Discoverability shouldn't become its own hazard: an agent guessing at
  recipe names, or inventing ones that sound plausible, is a different
  and unnecessary risk on top of the one already being guarded against.
- The fewer parameters a tool exposes, the fewer of them can ever be
  turned against the boundary it's meant to respect.

## Considered Options

- Trust the agent not to add anything harmful to the `justfile`
- Let every filesystem tool refuse the `justfile` by name, at the code
  level, and give `just` itself no room to point elsewhere
- Keep the `justfile` editable, but review every recipe run before it
  executes

## Decision Outcome

Chosen option: "Let every filesystem tool refuse the `justfile` by name,
at the code level, and give `just` itself no room to point elsewhere",
because it is the only option that closes both halves of the problem —
what the file says, and where the invocation looks — without asking
anyone, human or agent, to be trustworthy in the moment.

The `justfile` becomes readable but not writable through this server:
every tool capable of changing file contents rejects that name outright,
before the ignore list from ADR 0002 is even consulted, and nothing in
the server's configuration can lift that rejection. Reading stays open,
because the agent has to know what recipes exist to use them at all, and
a read cannot, by itself, hand anything back out. Running a recipe, in
turn, exposes only the recipe's name and its arguments — never a choice
of which `justfile` to read or which directory to run inside. Both of
those are fixed to the workspace itself, wired in at the point the tool
is built, with no parameter through which an agent could ask for
somewhere else. And before the tool is ever offered, the server asks
`just` itself what recipes the file defines, so the agent is choosing
from what was already written, never guessing at a name that isn't
there. A workspace without a `justfile` is simply never offered the tool
at all — there is nothing to sandbox and nothing to promise.

### Consequences

- Good, because a recipe the agent runs today is always a recipe a human
  wrote before the session began — never one the agent added for the
  occasion.
- Good, because the invocation path itself can't be redirected outside
  the workspace, even by an agent that fully understands how `just`'s
  own flags work.
- Good, because the same instinct already established for filesystem
  tools — refuse at the boundary, in code, unconditionally — extends
  cleanly to a tool that isn't a filesystem tool at all.
- Neutral, because the agent can still read the `justfile` in full,
  which is necessary for it to be useful but does mean the recipes'
  exact wording is always visible to it.
- Bad, because a project that wants the agent to propose new recipes has
  no path to that through this server; the human has to make that change
  themselves, outside the session.

### Confirmation

The write-refusal is covered by tests confirming that `create`,
`insert`, and `str_replace` all reject the workspace's `justfile` under
every name variant it might appear as, while `view` and `tree` continue
to reach it without issue. The recipe runner is covered by tests
confirming that neither the justfile path nor the working directory can
be influenced by anything in its input, and that only recipe names
already present in the discovered list are ever accepted.

## Pros and Cons of the Options

### Trust the agent not to add anything harmful to the `justfile`

No new rule at all — the `justfile` is treated like any other file, and
the recipe runner like any other command.

- Good, because it costs nothing to build.
- Bad, because it isn't a boundary, it's a hope. The whole reason this
  server exists is to make workspace safety independent of an agent's
  intentions in a given moment, and this option abandons that on the one
  file most able to undo it.

### Let every filesystem tool refuse the `justfile` by name, at the code level, and give `just` itself no room to point elsewhere

Described above, under Decision Outcome.

- Good, because it closes both the content half and the path half of the
  problem at once, each with the same kind of unconditional, code-level
  refusal already trusted elsewhere in the server.
- Neutral, because it means one specific filename is treated differently
  from every other file in the workspace — a small, deliberate exception
  to an otherwise uniform rule.
- Bad, because it removes one legitimate use case along with the
  illegitimate ones: an agent can never propose a new recipe, even a
  harmless one, without a human stepping outside the session to add it.

### Keep the `justfile` editable, but review every recipe run before it executes

A middle path: let the agent edit the file freely, but insert a
confirmation step before any recipe actually runs, so a human sees what
they're about to approve.

- Good, because it preserves the agent's ability to propose new recipes.
- Bad, because it turns a structural guarantee into a procedural one —
  the safety of the workspace now depends on a human reading and
  understanding an arbitrary recipe correctly, every single time, rather
  than on something the server itself enforces.
- Bad, because it does nothing about the path half of the problem; a
  reviewed, approved recipe can still be run against a `justfile` and
  directory the agent chose, unless that's separately closed off too —
  at which point this option has quietly become the chosen one plus an
  extra step.

## More Information

This decision leans on the same central path-checking mechanism ADR
0001 introduced and the same ignore-list reasoning from ADR 0002: the
`justfile` exception is enforced at that one shared gate, not
re-implemented separately for this tool.
