---
name: code-review-request-writing-guide
version: 2.0
description: >
  Writing guidelines for code review descriptions (MRs, PRs). Teaches storytelling
  structure, formatting rules, and reviewer-focused prose. Designed to be
  consumed by storytelling agents or used standalone.
---

# Writing Guide for Code Reviews

These rules govern how code review descriptions are written. They are
tool-agnostic and can be adopted by any agent or human author.

## Audience

The reader is a reviewer with **very limited time** — assume 30 seconds for the
TL;DR and at most 2 minutes for the full description. They have a trust
threshold to cross. They read the description _instead of_ commits, not
alongside them.

## Brevity is king

Every sentence must earn its place. If a sentence does not help the reviewer say
yes or flag a concern, delete it. A shorter description is always better than a
longer one — _provided it still tells the story_. Target: a reviewer who skims
should grasp motivation, approach, and risk in under 90 seconds. If the full
description takes more than 2 minutes to read, it is too long.

Surface non-obvious decisions, flag risks, make the reviewer's job effortless.
If the reviewer has to open the diff to understand the motivation, the
description has failed.

## TL;DR section

Every code review description **must** open with a `## TL;DR` section — 2–4 sentences
that a reviewer can absorb in 30 seconds. It answers three questions in
compressed form: what changed, why, and what the impact is. No code identifiers,
no implementation details, no bullet-list-of-files. If the reviewer reads
nothing else, this section alone must give them enough context to decide whether
the review is worth their attention.

## The one rule: tell a story

A code review description is not a changelog. It is a short story with a beginning
(Why), a turning point (What), and a resolution (How it was tested). Every
formatting decision — prose, bullets, subheadings — serves this arc. If the
description reads like a list of facts, the structure is wrong regardless of
whether it uses bullets or paragraphs.

**"Why" — normal world, then the break.** Start with how the system works when
everything is fine. The happy path, the ritual, the expectation. Then show where
it breaks and what the consequences are. The reader should _feel_ the problem
before being told the solution. Don't open with "X was broken" — let the reader
arrive there.

When the change is one piece of a multi-part effort, weave in a brief orientation:
name the larger goal, where this piece sits, and what comes before or after —
just enough so the reviewer can judge scope without opening sibling MRs. One or
two sentences, embedded in the narrative flow. Don't enumerate the full plan or
label it mechanically ("step 2 of 4"); let context emerge naturally.

**"What" — insight before mechanics.** Open with the core idea — the mental
shift that makes the solution feel inevitable. The reader should understand _why
this approach_ before seeing _what was changed_. Then walk through the mechanics
in whatever format serves them best.

**"How was it tested" — close the arc.** Connect back to the "Why": the
scenarios that were broken are now proven fixed. The reader should feel the
circle closing.

Each section should feel like the natural continuation of the previous one, not
an isolated block.

## Formatting follows content

Formatting is a tool, not a goal. Choose the format that serves the content:

- **Prose** when building context, motivation, or narrative flow.
- **Bullets** when enumerating concrete changes, test scenarios, or mechanical
  steps where scannability helps the reviewer.
- **Mixed** — prose paragraph followed by a bullet list — when a section needs
  both context and enumeration. This is often the best fit.

There is no prohibition on bullets, and no requirement for them. The test is
simple: _does this format help the reviewer understand faster?_ A section of
pure prose, pure bullets, or a mix are all valid if they pass that test.

## Section headings

The three story sections — _Why is this needed?_, _What does this change do?_, and
_How was it tested?_ — use `##` (H2). They are top-level peers of the TL;DR.

When a section covers multiple distinct concerns (e.g. runtime problem vs. test
acknowledgment, or guard model vs. refactorings), separate them with `###` (H3)
subheadings. Each subheading names the concern. This prevents invisible topic
jumps that force the reviewer to re-read. A section with a single concern needs
no subheading.

## Inline code formatting

All class names, type names, function names, field names, decorators, and code
identifiers must be wrapped in backticks (e.g. `DeviceProtocolRequest`,
`@final`, `execute_guarded_chain`). Raw text without backticks reads as prose,
not as code — the reviewer needs to distinguish the two instantly.

## Blank lines

Always insert a blank line before lists, between list items and prose, and
between paragraphs. GitLab Markdown collapses content without blank lines into a
single block — the reviewer loses structure. When in doubt, add the blank line.

## Breaking changes section

Every code review description **must** end with a `## Breaking changes` section — no
exceptions. This section exists so reviewers never have to guess.

- If there are breaking changes, list them concisely: what breaks, who is
  affected, and what they need to do.
- If there are **no** breaking changes, state that explicitly with a
  one-sentence justification (e.g. _"No breaking changes — all modifications are
  internal to the build pipeline and do not affect public APIs or
  consumer-facing contracts."_).

The section must always be the last `##` in the description.

## Anti-patterns

These are the most common failure modes. If the output matches any of them, the
description needs rework — regardless of how accurate the content is.

**1. The changelog.** A flat list of what changed, with no motivation, no
insight, no arc. This is the default output of most LLMs when asked to "describe
the changes." It answers _what_ but never _why_ or _why this way_.

> - Added `GuardModel` class
> - Renamed `instructions` to `body`
> - Extracted `URLBuilder` into separate module
> - Updated tests

**2. The inverted pyramid.** The description opens with the solution ("This change
introduces a guard model using `AsyncExitStack`...") before the reader knows
what problem exists. The reviewer has to reverse-engineer the motivation from
the solution. Flip it: problem first, solution second.

**3. The wall of prose.** Every section is a single dense paragraph with no
subheadings, no bullets, no visual structure. The content may be correct, but
the reviewer can't scan it. Use H3 subheadings at topic transitions and bullets
when enumerating.

**4. The orphaned test section.** "How was it tested" lists test files or
commands without connecting back to the "Why." The reader can't tell whether the
tests actually cover the scenarios that motivated the change. Close the arc: name
the broken scenarios from "Why" and show they are now proven fixed.

## Example

Below is an anonymized code review description that follows these guidelines. Use it as a
reference for tone, structure, and formatting — not as a template to fill in.

---

## TL;DR

REST cleanup steps (logout, release write access) are now structurally
guaranteed via stack-based guard semantics, replacing a flat instruction list
that skipped cleanup on failure. Four known-broken cleanup scenarios are fixed.

## Why is this needed?

When the service talks to a REST device, every parameter operation is wrapped in
a ceremony: log in, optionally request write access, run the actual command,
release write access, log out. The flat `instructions` list in `ProtocolRequest`
modeled this as a sequential chain — and the executor ran each step in order,
breaking on the first failure.

That break-on-failure contract is correct for the body, but fatal for cleanup.
If the parameter read fails after login, the executor never reaches logout. If a
write fails after acquiring write access, neither the release nor the logout
ever runs. The device is left with a dangling session, a held write lock, or
both. Four test scenarios documented this with `xfail` markers — the cleanup
problem was known but had no structural answer.

The root cause is that a flat list cannot express _"this step must run
regardless of what happens later."_ Cleanup needs a different execution contract
than the body.

## What does this change do?

The core idea is to separate _setup/teardown pairs_ from _the work they
protect_, then execute them with stack-based RAII semantics so cleanup is
structurally guaranteed.

### Guard model

`ProtocolRequest.instructions` is replaced by two fields: `guards` (a list of
`InstructionGuard`, each pairing an `acquire` with a `release`) and `body` (the
actual parameter instructions). A new `execute_guarded_chain()` function runs
the chain using `AsyncExitStack`: each successful acquire registers its release
as a callback, releases run in LIFO order when the stack unwinds, and a failed
acquire skips both its release and the body.

### Completion layer

The completion strategy now returns a `CompletionResult` (`NamedTuple` of
`guards` + `body`) and enforces symmetric before/after counts per profile — an
asymmetric profile raises eagerly rather than silently producing an unpaired
chain.

### Structural cleanup

- `URLBuilder` extracted into its own module — it builds HTTP URLs, not
  execution logic.
- Executor sealed with `@final`; internals lifted to module-level functions.

## How was it tested?

The four `xfail`-marked cleanup scenarios now pass without the marker:

- Body failure → logout still runs (read path).
- Body failure → both release-write-access and logout run (write path).
- Inner guard failure → outer guard still cleans up.
- Release failure → doesn't block outer release.

Additional coverage: asymmetric profile rejection, match fallthrough, and all
existing tests migrated to the new `guards`/`body` structure.

## Breaking changes

None — `ProtocolRequest` is an internal model not exposed beyond the service
boundary. All field renames (`instructions` → `guards`/`body`) are consumed
exclusively within the codebase.
