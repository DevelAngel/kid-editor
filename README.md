# kid-editor

*Keep It Done: one precise edit, not a conversation about the file.*

## TL;DR

`kid-editor` is a small MCP server that gives a language model exactly two things:
precise file-editing tools, and a hard boundary around where it can use them.
One workspace, five tools, no way out.
Behind an OAuth 2.1 door, so only clients you've explicitly trusted get to walk through it.

## The problem

Picture a model trying to fix one line in a thousand-line file.
What does it usually do?
It reads the whole file, rewrites the whole file, and sends the whole file back.
One line changed—nine hundred and ninety-nine untouched, but rewritten anyway.
Slower. Riskier. Harder to review.

That's the editing problem.
There's a second one, quieter but sharper: access.
Give a model a shell and a folder, and you're trusting it not to wander.
Most of the time it won't.
But "most of the time" isn't a boundary—it's a hope.

You shouldn't have to choose between a model that edits clumsily and a model you can't fully trust with your filesystem.
You shouldn't have to choose at all.

## What it does

`kid-editor` exposes five tools for text editing.
Together, they mirror what a careful person does when they sit down with an unfamiliar project:
look around, read closely, change precisely.

- `tree` gives the overview — the shape of the project, at a glance, the way the Unix command of the same name always has.
- `view` shows a file with line numbers, or a slice of one, or a directory's contents.
- `str_replace` changes exactly one thing:
  it demands that the text you're replacing appears exactly once.
  Show up twice, and the edit is refused—not silently guessed at, refused, with a request for more context.
- `create` writes a new file, or overwrites an old one, on purpose.
- `insert` adds text after a given line.

Five tools. Not fifty.
That's not an accident — it's the whole design philosophy, stated as a list.

## Why it matters

**Precision over collateral damage.**
A `str_replace` that requires a unique match can't silently touch the wrong instance.
It either finds exactly one match, or it stops and asks.
That's not a limitation — it's the point.

**A sandbox, not an open door.**
Every path—relative or absolute — gets resolved against the workspace root before anything happens to it.
Try to step outside that root, and the request fails.
Not "discouraged." Fails.
The model isn't asked nicely to stay inside the workspace.
It structurally cannot leave it.
The one thing this doesn't cover is a symlink someone placed inside the
workspace before the server ever started — nothing here can create one,
so if it exists, that was a decision made outside `kid-editor`, not
something a client talked it into.

**Cutting the noise.**
`.git`, `target`, `node_modules` — directories like these clutter every overview and rarely hold anything a model needs to see.
An ignore list makes them disappear.
Not hidden, not flagged: invisible.
As far as any tool is concerned, they don't exist, the same way a path outside the workspace doesn't exist.

**A locked door, not a curtain.**
The server speaks OAuth 2.1 and serves its own metadata—discovery documents, authorization endpoint, token endpoint, all of it.
Only clients on an explicit allowlist can complete the flow.
There's no implicit trust extended to whoever happens to knock.

**Built on an open standard.**
`kid-editor` implements the Model Context Protocol (MCP), the same protocol behind a growing ecosystem of AI tooling.
Any MCP-capable client can talk to it.
Want to poke at the tools yourself, outside of any particular client?
The [MCP Inspector](https://modelcontextprotocol.io/legacy/tools/inspector) — the reference tool for the standard — is built exactly for that.

## How this differs from a coding agent

You've probably used something like this already — Goose, Claude Code, Cursor, Pi, Open Code.
Tools that read your code, write your code, run your tests, open your terminal.
So why build another one?

Because `kid-editor` isn't one of those.
It's not an agent at all.

A coding agent is a loop:
a model, a set of tools, a planning strategy wrapped around all of it, deciding what to do next and when to stop.
It usually runs commands, manages its own context window, and makes judgment calls about scope.
`kid-editor` does none of that.
It's the opposite of an agent, really—it has no opinion about what should happen next.
It's a door with a lock on it, not the person walking through the door.

That difference has a consequence:
`kid-editor` doesn't care which model is on the other end, or what agent framework is asking it for a file.
Point any MCP client at it—an agent you built yourself, a chat interface, a script running unattended on a server — and it behaves identically.
The boundary lives in the server, not in the agent's good behavior.
Swap out the model, swap out the framework, the sandbox doesn't move.

So if you already run a coding agent:
`kid-editor` isn't a competitor to it.
It's what you'd put underneath it, or next to it, when you need one specific guarantee — provable, not promised — about where that agent's file access ends.

## License

`kid-editor` is licensed under the GNU Affero General Public License, version 3 (AGPLv3).
See [LICENSE](./LICENSE) for the full text.
