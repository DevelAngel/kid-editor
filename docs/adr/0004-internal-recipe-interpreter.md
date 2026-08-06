---
status: accepted
date: 2026-08-06
---

# Adding an Internal Recipe Interpreter Alongside `just`

## Context and Problem Statement

`just_run` shells out to an external `just` binary the server does not
control. Two problems follow from that, independent of the write
protection ADR 0003 already gives `justfile`:

- The server has no verified guarantee about which `just` version is
  installed on a given machine, and therefore no verified guarantee
  about which flags, output formats, or behaviors `run_just`'s parsing
  (`parse_recipe_list`, `parse_recipe_description`, `parse_recipe_usage`)
  is actually matching against.
- `just`'s feature set is larger than this server needs, and some of it
  — `import`/`mod` in particular (see ADR 0003's addendum) — has already
  needed a dedicated countermeasure once. Every feature `just` adds is a
  feature this server's threat model has to re-examine, without this
  server having any say in whether it ships.

Bob's build sandbox (mounting a project into `/build/`) does not resolve
either point — it isolates side effects of _executing_ a recipe, not the
trustworthiness of the interpreter parsing it or the size of that
interpreter's feature surface.

There is also a threat this decision inherits from ADR 0003 rather than
introduces: a tool that runs named recipes, and a recipe file an agent
can edit, together amount to letting the agent execute arbitrary
commands — one line added to the recipe file is a new, unreviewed
capability, indistinguishable from every other recipe a human wrote and
approved. Making that file invisible and read-only through this server
(matched by name, at any depth) is what keeps `recipe_run`'s recipes
trustworthy: every recipe it can run is one that was on disk, reviewed,
before the agent ever connected. This ADR restates that boundary for its
own recipe file rather than pointing back at ADR 0003, specifically so
ADR 0003 and `just_run` can be retired later without taking this
protection down with them.

`just_run` also has real users today, and nothing about the above is
urgent enough to justify breaking them. This ADR adds a second,
independent tool rather than replacing the first: `recipe_run`, gated by
an explicit `--recipes-file <FILE>` flag, inactive unless that path is
both configured and present at startup. `just_run`/`--enable-just-run`
are untouched by this decision and continue to work exactly as ADR 0003
describes. Adoption of the new interpreter is therefore per-workspace and
opt-in, not a migration this project imposes on anyone.

## Decision Drivers

- The set of recipe features offered should be a deliberate, small
  choice made by this project, not whatever a third-party binary happens
  to support this month.
- No feature should be reachable that this server hasn't explicitly
  implemented and reviewed — no surprise flags, no surprise import
  mechanism to patch around after the fact.
- Recipe execution must not go through a shell. String interpolation
  into a shell command line (as the current `justfile`'s
  `@git commit --message="{{message}}"` does) is a quoting hazard by
  construction; removing the shell removes the hazard class, not just
  one instance of it.
- Whatever replaces `just` should stay small enough to read in one
  sitting — this is a security-relevant component, not a general-purpose
  build tool.

## Considered Options

- Keep only `just`, add a version check at startup
- Add `cargo xtask` support alongside `just`
- Add a minimal internal interpreter, TOML-based, alongside `just`

## Decision Outcome

Chosen option: "Add a minimal internal interpreter, TOML-based, alongside
`just`", because it is the only option that removes both problems for
whoever opts in — no external version to trust, and no feature surface
beyond what this server explicitly implements — without narrowing the
server to Rust-only projects the way `cargo xtask` would, and without
forcing every existing `just_run` user through a migration to get there.

A recipe file (path given via `--recipes-file`) declares one
`[recipe.<name>]` table per recipe: a `description`, an optional ordered
`args` list of named parameters, and a `run` array — the argv of the
command to execute directly via
`std::process::Command::new(&run[0]).args(&run[1..])`, with named
parameters substituted only into placeholders like `{name}` inside
individual argv elements. No shell is ever invoked. No `import`/`mod` or
equivalent exists in the format, so there is nothing there for a future
addendum to close.

### Consequences

- Good, because there is no version to be uncertain about — the
  interpreter is this server's own code, reviewed the same way any other
  change to it is.
- Good, because the feature surface is exactly what this project chose
  to implement — no flag or mechanism can appear in it without a source
  change here.
- Good, because removing the shell removes an entire class of quoting
  bugs, not just the one already visible in the current `justfile`.
- Good, because `just_run` is untouched — no existing workspace's setup
  breaks, and adopting `recipe_run` is a per-workspace choice made by
  passing `--recipes-file`, not a forced migration.
- Good, because the recipe-file boundary (invisible + read-only,
  independent of ordinary ignore configuration, active only while the
  tool is actually offered) is stated here in full, not borrowed from
  ADR 0003 by reference — `recipe_run` keeps working exactly as
  designed even if ADR 0003 and `just_run` are removed later.
- Neutral, because projects using the new interpreter lose `just`'s
  existing conveniences (recipe dependencies, conditionals, string
  manipulation functions); `just_run` remains available for workspaces
  that need those.
- Bad, because this server now owns a (small) parser and executor it
  previously got for free — bugs in it are this project's bugs, not
  upstream's.

### Confirmation

The interpreter's own module (`recipe_run.rs` plus the `recipe` crate)
carries unit tests covering: TOML parsing into the recipe map,
named-parameter substitution (including a parameter used more than
once, and a `run` argv element with no placeholder), missing-parameter
errors, and that no `Command` is ever constructed through a shell
(verified by construction — no `sh -c` or platform shell path appears
anywhere in the executor). `just_run`'s own tests are untouched, since
that tool's implementation did not change.

## Pros and Cons of the Options

### Keep only `just`, add a version check at startup

Query `just --version` (or `--dump --dump-format json`) once at server
start, refuse to offer `just_run` if the result doesn't match an
expected version or feature set.

- Good, because it is the smallest possible change — no new parser, no
  new format, `just` keeps doing what it already does well.
- Good, because it turns an assumption into a checked fact, which is a
  real improvement over the status quo.
- Bad, because it only narrows the uncertainty to "a version we
  checked once," not "a version we control" — the feature surface is
  still `just`'s to expand, and every expansion is still something this
  server's threat model has to notice and react to after the fact.
- Bad, because it does nothing about the shell-quoting hazard class
  already present in the current `justfile`.

### Add `cargo xtask` support alongside `just`

Recipes become a Cargo workspace member (`xtask`), invoked via
`cargo run -p xtask -- <command>`. No text format to parse or interpret
at all — recipes are ordinary Rust, compiled and reviewed like the rest
of the codebase.

- Good, because there is no runtime interpreter to trust or maintain —
  the compiler and the project's own review process already cover it.
- Good, because there is no shell-quoting hazard class — argv is built
  in Rust, not interpolated into a command line.
- Bad, because it only works for projects with a Cargo toolchain
  already present. This server is meant to work with any workspace, not
  only Rust ones — narrowing recipe support to one ecosystem reopens the
  scaling problem `just` was originally chosen to avoid (ADR 0003:
  "there is always another ecosystem the server hasn't heard of yet").

### Add a minimal internal interpreter, TOML-based, alongside `just`

Described above, under Decision Outcome.

- Good, because it works for any project, independent of language or
  toolchain — a TOML file and an argv array make no assumption about
  what's being built.
- Good, because the feature set is exactly as large as this project
  makes it, and no larger.
- Bad, because building and maintaining even a small interpreter is
  more code than delegating to an existing one — this project now
  carries that maintenance burden itself.

## More Information

This decision introduces a new crate, `recipe/` (library + standalone
`recipe` binary), and a new MCP tool, `recipe_run`
(`text/src/mcp/recipe_run.rs`), gated by `--recipes-file`. It does not
change `just_run` (`text/src/mcp/just_run.rs`), which continues to be
governed by ADR 0003 as before. The recipe-editability threat and the
boundary against it are stated in full above (see "Context and Problem
Statement") specifically so this tool does not depend on ADR 0003
remaining in force. ADR 0002 (ignore-list-as-nonexistence) is reused for
whatever file `--recipes-file` points at, the same way ADR 0003 already
reuses it for `justfile`.

This project's own build/lint/test/git workflow is not migrated to
`recipes.toml` as part of this decision — `justfile` remains this
repository's own recipe file. Migrating it, if done at all, is a
separate, later step once `recipe_run` has seen use elsewhere; this ADR
only adds the capability.

Exposing one MCP tool per recipe, instead of a single `recipe_run` with
an enriched description, was also discussed. It is not part of this
decision: `rmcp`'s `#[tool_router]` builds its router at compile time
from static `#[tool]` annotations, so per-recipe tools would require
constructing `Tool` definitions and dispatch by hand at runtime rather
than via the macro this codebase otherwise relies on throughout
`text/src/mcp/`. That is a larger, separate change and is left for a
future ADR if pursued.
