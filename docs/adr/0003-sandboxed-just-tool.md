---
status: superseded
superseded-by: 0004
date: 2026-08-03
---

# A Command Runner That Can't Redraw Its Own Cage

> Superseded by [ADR 0004](0004-internal-recipe-interpreter.md) and
> [ADR 0005](0005-per-recipe-mcp-tools.md): the `just_run` tool this
> decision protects has been removed. `recipe_run`'s internal
> interpreter and per-recipe tools now cover the same need, without an
> external `just` dependency. The threat model and write-protection
> reasoning described here were carried forward into ADR 0004 rather
> than lost — see that ADR's "Context and Problem Statement" — so this
> document is kept for history, not as a currently-enforced boundary.
> The workspace's own `justfile` still exists on disk but is no longer
> referenced by anything in this repository (`prek.toml`'s `cargo-test`
> hook now runs `cargo test` directly) or specially recognized, hidden,
> or write-refused by this MCP server.

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

### Addendum: reading was closed too

The "Reading stays open" reasoning above held only as long as a read
couldn't, by itself, hand anything back out — true for a human, not
guaranteed for an agent whose next step might be summarizing file
contents into a context an attacker-controlled prompt can steer. The
default `--ignore` list was extended to cover `justfile` itself as well,
making it invisible to `view` and `tree`, not merely write-refused.
`just_run`'s tool description (see the addendum below) remains the
agent's only window into what recipes exist, by design — that was
always meant to be sufficient, and closing the read path is what makes
it actually the *only* one, rather than one of two.

This is a default, not a hard rule the way write-refusal is: unlike
`into_write_buffer`, the ignore list is ordinary server configuration,
so an operator can still choose to make justfile-like files readable
again by passing a narrower `--ignore`. The write-refusal in
`into_write_buffer` stays unconditional either way, so removing the
default ignore entry only restores *reading*, never editing — see
"More Information" below for why the two checks are kept independent.

*(Superseded in part by the "conditional on `just_run` existing at all"
addendum further below: both checks moved out of `--ignore`'s default
value, and write-refusal stopped being unconditional, once `just_run`
itself became opt-in via `--enable-just-run`. This addendum is kept for
the reasoning that motivated closing the read path in the first place,
which still holds.)

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

### Addendum: a boundary the agent can't see isn't fully useful

The sandboxing above answers "can the agent misuse `just_run`" but not
"does the agent know `just_run` is worth using at all." A tool's
description is what an MCP client shows the agent before any call is
made, and that description is normally a string fixed at compile time —
it has to be, since it's attached to the tool's definition, not to a
particular server instance. But which recipes exist is only known once,
at startup, from that specific workspace's `justfile`. A compile-time
description can say "runs a `just` recipe" in the abstract; it cannot
say "runs `check`, `lint`, or `test`" for a `justfile` that didn't exist
when the server was built.

In practice this meant the tool went unused even when it was exactly
what the moment called for — an agent asked to run the project's linter
had no way to learn, short of guessing a recipe name and being told
"no such recipe," that `just_run` was the way to do it at all. The
sandbox held; the tool just sat unread.

The fix doesn't touch the sandbox — the discovered recipe set from
`RecipeName::discover` was always the source of truth for what's
callable, checked at call time regardless of what the description says.
It only changes what the client is told *before* calling anything: the
server builds its tool list itself rather than handing back a fixed one
per tool definition, and for `just_run` specifically, appends the
discovered recipe names to the description at that point — the same
list `just_run` already validates every call against, just surfaced a
step earlier. An agent can now see "runs `check`, `lint`, `test`" up
front, without a failed guess first. Nothing about which recipes are
*runnable* changes; only how early the agent learns what they are.

### Addendum: `import` reopens the file half of the problem

The original decision treated "the `justfile`" as one file, protected by
name. But `just` supports `import 'path'` and `import? 'path'` (and
`mod name` for submodules) directly inside a `justfile`, pulling recipes
in from another file at parse time — a file the agent, without a further
rule, could read and write freely, since only the literal name
`justfile` was ever refused. Editing that imported file is exactly as
effective a way to add or change a recipe as editing the `justfile`
itself would have been; the protected name became a facade with an
unprotected side door right next to it.

The fix widens the same two refusals ADR 0003 already established —
ignored (invisible) and write-refused — from the single literal name
`justfile` to two conventional patterns, matched at every depth rather
than just the workspace root: `justfile` and `*.just`. A file named
`recipes/build.just` is now exactly as untouchable as the top-level
`justfile` always was, for the same reason.

This is a deliberate narrowing, not full closure: `just`'s import target
is an arbitrary string, and a `justfile` can `import 'notes.md'` or
`import 'build-steps'` just as validly as `import 'ci.just'`. Actually
parsing every `justfile` for its import targets and protecting whatever
they resolve to would close that gap completely, but at the cost of a
recursive parser sitting in the trust path — more surface, for a
convention almost every real project already follows. The two patterns
here catch the conventional case; a `justfile` that imports something
named unconventionally is a risk this server does not currently guard
against, and that gap is intentional enough to be worth stating
plainly rather than leaving implicit.

### Addendum: the protection is conditional on `just_run` existing at all

`just_run` is off by default and only turns on via `--enable-just-run` —
requiring an explicit, positive choice rather than inferring it from a
`justfile`'s mere presence, because a `justfile` an operator hasn't
looked at is exactly the trust this ADR was written to avoid extending
automatically. Once that flag is set, the entire justification for
treating `justfile`/`*.just` specially applies: every refusal above,
from the original 2026-08-03 decision through both addenda, exists to
stop an agent from editing a recipe it could then invoke. Without the
flag, `just_run` is never offered at all — an agent that cannot invoke
*any* recipe has nothing to gain from editing one either, so the files
are just text at that point, no more dangerous than any other file in
the workspace.

So the two checks were made conditional on the same signal that already
decides whether `just_run` is offered — `!just_recipes.is_empty()`,
itself always `false` unless `--enable-just-run` was passed, since
recipe discovery doesn't run at all without it. `justfile`/`*.just` are
folded into the effective ignore list (and `into_write_buffer`'s refusal
enabled) only when that's `true`; by default, they're ordinary files —
readable, writable, listed in `tree` — same as anything else not
matched by `--ignore`. This also moved the two patterns out of
`--ignore`'s own default value: they were never meant to be ordinary,
user-editable ignore configuration, and living in the same list as
`.git`/`target`/etc. made that unclear.

`--enable-just-run` is meant to be the operator's confirmation that
they've reviewed the workspace's `justfile` — but a flag can't verify
that a review actually happened, only that someone typed it. To make
that confirmation checkable rather than merely assumed, startup walks
the workspace (skipping whatever `--ignore` already skips) and logs
every `justfile`/`*.just` it finds, by path, before the server starts
accepting connections. Anyone who set the flag can read that list back
against what they remember reviewing; anyone who didn't review carefully
gets a second, concrete chance to notice a file they missed, in a
directory they didn't expect a recipe file to be. The walk plays no part
in what actually gets protected — that's still `IgnorePattern`/
`WorkspacePath`, evaluated per request — it exists purely so the
assumption behind `--enable-just-run` has somewhere to be checked
instead of just made.
