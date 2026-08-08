---
status: accepted
date: 2026-08-08
---

# Prefixing the Filesystem Tools with `fs_`

## Context and Problem Statement

ADR 0005 gave every recipe its own tool, `recipe_<name>`, and picked the
`recipe_` prefix specifically so a recipe could never collide with one of
this server's fixed tool names (`view`, `create`, `insert`,
`str_replace`, `tree`). That solves collision, but leaves an asymmetry:
one group of tools (recipes) announces what it is through its name, and
the other (the fixed five) doesn't — a client reading a tool list can't
tell "touches the filesystem" from "runs a command" by name alone unless
it already knows this server's specific fixed-tool names by heart.

## Considered Options

- Leave the fixed five unprefixed (status quo)
- Drop the `recipe_` prefix instead
- Prefix the fixed five with `fs_`

## Decision Outcome

Chosen option: "Prefix the fixed five with `fs_`", because it makes both
groups self-describing by name (`fs_*` touches the filesystem, `recipe_*`
runs a configured command) rather than only one of them, and because
dropping `recipe_` instead would reopen the exact collision ADR 0005
closed — a recipe named `view` would then shadow the real one.

Renamed: `view` → `fs_view`, `create` → `fs_create`, `insert` →
`fs_insert`, `str_replace` → `fs_str_replace`, `tree` → `fs_tree`. Same
tools, same behavior, same `#[tool_router]` wiring — only the `#[tool]`
function names (and therefore the MCP tool names derived from them)
change. `README.md`'s tool list and the cross-reference in ADR 0005 are
updated to match.

### Consequences

- Good, because both tool groups are now self-describing by prefix, not
  just the newer one.
- Good, because it keeps the `recipe_` prefix and the collision guarantee
  ADR 0005 built it for.
- Bad, because it's a breaking rename for every one of this server's
  original, longest-standing tools — any existing client calling `view`,
  `create`, `insert`, `str_replace`, or `tree` by name breaks. Accepted
  deliberately: this server has no external users yet to break in
  practice, so the rename is cheap now and only gets more expensive to
  make later.

## Pros and Cons of the Options

### Leave the fixed five unprefixed (status quo)

- Good, because it's zero-cost — no rename, no breaking change.
- Bad, because it leaves the asymmetry described above unresolved.

### Drop the `recipe_` prefix instead

- Good, because recipe tool names get shorter and read more naturally.
- Bad, because it reopens the collision ADR 0005 exists to close — a
  recipe named `view`, `create`, `insert`, `str_replace`, or `tree`
  would shadow a fixed tool, with no structural guarantee against it.

### Prefix the fixed five with `fs_`

Described above, under Decision Outcome.

- Good, because it resolves the asymmetry without reopening the
  collision problem.
- Bad, because breaking every fixed tool's name is a larger surface
  than renaming nothing, or than renaming only the newer, less-used
  recipe tools would have been.

## More Information

This decision changes `text/src/mcp/view.rs`, `create.rs`, `insert.rs`,
`str_replace.rs`, and `tree.rs` (the `#[tool]` function name in each),
plus their own unit tests, `README.md`, and the cross-reference note in
ADR 0005. It does not change `recipe_run.rs`, ADR 0004, or ADR 0005's
own decision — only the collision example they cite (fixed tool names
they cite as illustration are renamed, the reasoning is not).
