---
status: accepted
date: 2026-08-07
---

# One MCP Tool per Recipe Instead of a Single `recipe_run`

## Context and Problem Statement

ADR 0004 introduced `recipe_run(name, args)`: one generic MCP tool that
looks up `name` in `recipes.toml` and runs it. That ADR's "More
Information" section already flagged the alternative — one tool per
recipe — and set it aside as a larger, separate change.

The gap this leaves: an MCP client can only grant or deny `recipe_run`
as a whole. A client that's happy to let an agent run `check` and `test`
unsupervised, but wants to approve `git-commit` and `git-rebase`
individually every time, has no way to express that against a single
tool — approving `recipe_run` approves every recipe behind it at once,
including ones added to `recipes.toml` later. Whatever access-control
model a client applies to tools (allow-lists, per-call confirmation,
audit logging) can only be as fine-grained as the tools it's applied to.

## Decision Drivers

- Client-side permission granularity should be possible per recipe, not
  only per recipe *file*.
- The change should not touch `just_run`, for the same reason ADR 0004
  didn't: real users, no urgency to break them.
- `rmcp`'s `#[tool_router]` builds its router at compile time from
  static `#[tool]` annotations — the set of recipes, and therefore the
  set of tools, is only known once `recipes.toml` is read at startup.
  Whatever approach is chosen has to work within that constraint.

## Considered Options

- Keep `recipe_run(name, args)` as the only interface
- One MCP tool per recipe, built and dispatched by hand outside `#[tool_router]`

## Decision Outcome

Chosen option: "One MCP tool per recipe, built and dispatched by hand
outside `#[tool_router]`", because it's the only option that gives MCP
clients per-recipe granularity at all — a single generic tool can't
expose that distinction to a client no matter how it's described.

Each recipe becomes a tool named `recipe_<name>`, with `-` replaced by
`_` (MCP tool names are restricted to `[A-Za-z0-9_]`; recipe names use
`-`, see `recipes.toml`). The `recipe_` prefix guarantees no collision
with this server's fixed tool names, even for a recipe literally named
`view` or `create`. Each tool's input schema is built by hand from the
recipe's own `args`: one required string property per declared
parameter, named and described from that parameter's `help`.

`McpService::list_tools` and `call_tool` call into a small
`recipe_run::tools`/`recipe_run::call` pair directly, alongside — not
through — `self.tool_router`. `call` returns `Option<Result<...>>`:
`None` means the requested name isn't one of the generated recipe tools,
so `call_tool` falls through to `tool_router` for everything else
(`view`, `create`, `just_run`, ...).

### Consequences

- Good, because an MCP client can now grant, deny, or gate each recipe
  independently — the thing this ADR exists to enable.
- Good, because the `recipe_` prefix and mechanical name derivation mean
  no recipe name, however chosen, can shadow a fixed tool.
- Good, because `just_run` and its single-tool shape are untouched —
  this only changes how `recipe_run`'s recipes are exposed.
- Neutral, because the input schema is now built by hand (`serde_json`
  values) instead of derived via `#[derive(JsonSchema)]` — unavoidable
  given the schema's shape is only known at runtime, but it does mean
  schema correctness is this code's responsibility, not the derive
  macro's.
- Bad, because tool dispatch for recipes now bypasses `tool_router`
  entirely, so this one corner of the server no longer benefits from
  whatever `tool_router` does under the hood (argument validation
  against the schema, consistent error shapes) — `recipe_run::call`
  re-implements the minimum of that by hand (missing-argument checks).

### Confirmation

`recipe_run`'s tests cover: tool-name derivation (hyphens to
underscores), that generated tools carry one schema property per
declared argument with the right `required` list, and that `call`
returns `Some` for a known recipe's derived tool name and `None` for an
unrecognized one. The existing "does this repo's `recipes.toml` parse
and declare the expected recipes" sanity test is unaffected — it tests
`RecipeFile`, not tool generation.

## Pros and Cons of the Options

### Keep `recipe_run(name, args)` as the only interface

- Good, because it's already implemented (ADR 0004) and simpler — one
  tool, one schema, routed through the same macro as everything else.
- Bad, because it caps client-side permission granularity at "all
  recipes or none" — the problem this ADR exists to solve.

### One MCP tool per recipe, built and dispatched by hand

Described above, under Decision Outcome.

- Good, because it solves the granularity problem directly.
- Bad, because it requires bypassing `#[tool_router]` for this one
  feature, the only place in this codebase that does so — a deliberate,
  documented exception rather than the norm.

## More Information

This decision changes `text/src/mcp/recipe_run.rs` (its `#[tool]`
function and `RecipeRunInput` are replaced by `tools`/`call`) and
`text/src/mcp/mod.rs`'s `list_tools`/`call_tool`. It does not change
`just_run`, ADR 0003, or ADR 0004's write-protection boundary for
`recipes.toml`, which continues to apply unchanged.
