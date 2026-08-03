---
status: accepted
date: 2026-08-03
---

# Making Path Safety a Type, Not a Habit

## Context and Problem Statement

Every tool in this server touches the filesystem, and every one of them
starts from the same place: a path the client sent, unchecked, in a
message that could have come from anywhere. Somewhere between that
message arriving and a file actually being read or written, one question
has to get answered correctly, every single time: does this path
actually belong inside the workspace, or does it reach somewhere it
shouldn't? How do we make sure that question is never, even once, left
unanswered?

## Decision Drivers

- A forgotten check should be caught by the compiler, not discovered
  later by whoever the missing check was supposed to stop.
- The guarantee has to hold for every tool touching the filesystem today
  and every tool that gets added later, without relying on whoever adds
  it to remember a rule that lives only in a comment or a code review.
- Whatever mechanism enforces this shouldn't demand more from the
  framework than it can actually give — a design that only works if some
  unverified internal behaves a certain way is not a foundation to build
  a security boundary on.
- The ceremony this costs at each call site should stay small enough
  that using it correctly is also the path of least resistance.

## Considered Options

- Convention: a shared function everyone remembers to call
- A wrapper type that's the only way to get a filesystem-safe path
- Validating paths the instant they're deserialized from the request

## Decision Outcome

Chosen option: "A wrapper type that's the only way to get a
filesystem-safe path", because it's the only one of the three that turns
a forgotten check into something the compiler refuses to build, without
depending on framework internals nobody could verify without a build
environment on hand.

A path arriving from a client is wrapped immediately in a type that
offers exactly one thing: a way to check itself against the workspace
and the ignore list, and, if that check succeeds, turn into a second,
different type — one that carries no memory of ever having been
unchecked. That second type is the only thing any tool may hand to the
filesystem. There is no other way to produce one: no shortcut, no back
door, no field left exposed for a moment of convenience.

### Consequences

- Good, because a tool that tries to skip the check doesn't compile —
  not "might misbehave," not "would fail a test if someone wrote one,"
  but does not build.
- Good, because the guarantee lives in one place. A new tool inherits it
  automatically, simply by using the type the way every other tool
  already does, with no separate step to remember.
- Good, because it needed nothing from the framework beyond what its
  public, documented extraction mechanism already offers — nothing here
  depends on internal behavior we couldn't verify.
- Neutral, because a small amount of ceremony remains at the boundary: a
  tool receives the unchecked form and converts it, one line, before
  doing anything else.
- Bad, because that one line can still be skipped on purpose, by reaching
  past the type and pulling a path out of it directly. Nothing in the
  type system stops a determined rewrite from doing that.

### Confirmation

The wrapper type's checking logic carries its own unit tests, covering
directory traversal through `..` (including forms that stay lexically
inside the workspace on paper while still stepping outside it), absolute
paths reinterpreted as workspace-relative rather than reaching the real
filesystem root, and entries matching the configurable ignore list. Every
tool that touches the filesystem is reviewed to confirm it only ever
holds the checked form of a path at the point it calls into `std::fs` —
a review that the compiler itself already performs on every build,
before any human needs to.

## Pros and Cons of the Options

### Convention: a shared function everyone remembers to call

A single function, called at the top of every tool, that checks a path
before anything else happens to it. This is roughly where the project
started.

- Good, because it's the simplest thing that could possibly work, and it
  did work, for as long as everyone remembered to call it.
- Neutral, because it puts all the checking logic in one place, which is
  necessary but not sufficient — the logic being centralized doesn't
  guarantee every call site actually reaches it.
- Bad, because nothing stops a new tool, or a hurried edit to an old one,
  from touching the filesystem without calling it first. The mistake
  doesn't announce itself; it waits for the one path that was built to
  actually cross the line.

### A wrapper type that's the only way to get a filesystem-safe path

Described above, under Decision Outcome.

- Good, because forgetting becomes a compile error instead of a bug
  waiting to be found.
- Good, because it costs only one predictable line per tool, at the
  point where the path is first used.
- Bad, because it doesn't stop someone from deliberately working around
  it — only from doing so by accident.

### Validating paths the instant they're deserialized from the request

The most ambitious version of the idea: check every path before any of
our own code even runs, as part of turning the incoming message into
Rust values in the first place.

- Good, because it would close the one gap the chosen option leaves
  open — there would be no unchecked form to ever hold, not even for one
  line.
- Bad, because checking a path needs to know which workspace it's being
  checked against, and that information isn't available yet at the
  moment a message is being decoded — the framework's public
  deserialization mechanism has no path to that context.
- Bad, because the only way found to work around that gap involved
  overriding internal framework behavior in a way that couldn't be
  verified without a build environment — an unacceptable foundation for
  a security boundary in a server already running in production.

## More Information

This decision governs every tool that reads or writes files, including
`tree`, `view`, `str_replace`, `create`, and `insert`.
