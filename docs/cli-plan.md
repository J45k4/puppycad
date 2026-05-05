# Puppycad CLI Plan

The Puppycad CLI is not a terminal clone of the GUI. It is the control surface for a running PCAD/Puppycad engine server.

For now, do **not** design around a custom `.pcad` DSL or text source format. The source of truth is the server/project engine API. A textual CAD format may come later, but the CLI should first become a reliable client for inspecting, mutating, evaluating, rendering, exporting, and debugging server-backed projects.

## Design principle

The CLI talks to the PCAD server and exposes engine capabilities in a scriptable way:

- stable JSON output for agents and scripts
- useful human output for terminal use
- deterministic commands
- stable IDs/names from the engine model
- no duplicate CAD model inside the CLI
- CLI behavior should reuse the same APIs as the GUI/server

If something is hard to expose through the CLI, that probably means the server/engine API needs a clearer abstraction.

## Architecture

```text
CLI -> PCAD server API -> project/feature graph/evaluator/renderer/exporter
GUI -> PCAD server API -> same engine
Agents/scripts -> CLI or PCAD server API
```

The CLI may have local convenience commands, but it should not become a separate implementation of CAD operations.

## Command shape

```sh
puppycad [global options] <area> <action> [target] [options]
```

Global connection options:

```sh
puppycad --server-url http://localhost:5337 inspect <project-id>
puppycad --project <project-id> query features --json
puppycad --server-url http://localhost:5337 --project bracket render --out preview.png
```

Default server can be `http://localhost:5337` while developing. Server URL resolution is:

1. `--server-url`
2. `PUPPYCAD_SERVER_URL`
3. CLI config `serverUrl`
4. `http://localhost:5337`

Project resolution is:

1. explicit command project id, where accepted
2. `--project`
3. CLI config `defaultProject`

The command config key is kebab-case while JSON remains camelCase:

```sh
puppycad config set server-url http://localhost:5337
```

```json
{ "serverUrl": "http://localhost:5337" }
```

## Layers exposed by CLI

1. **Connection layer** — find/connect to PCAD server, health checks, project selection.
2. **Project layer** — create/list/load/save projects through server APIs.
3. **Graph layer** — inspect feature/dependency graph.
4. **Evaluation layer** — compute/query generated geometry.
5. **Output layer** — render/export/package results.
6. **Mutation layer** — apply structured commands/patches to the server-backed project.

## Output modes

Every useful command should support stable machine output:

- `--json`
- `--ndjson` for long-running operations
- `--pretty` / human output
- `--quiet`
- `--verbose`

Human output is for reading. JSON/NDJSON is for scripts and agents. Scripts should never need to parse English summaries.

## Stable identities

Commands should operate on durable IDs or names, not array indexes.

Good:

```sh
puppycad edit f2 --distance 20
puppycad edit extrude base_plate --distance 20
puppycad delete f4 --dry-run
```

IDs should come from the engine/server model and remain stable across CLI, GUI, and API use.

Names are human conveniences. IDs are references.

## Core command groups

Server-first MVP groups:

```text
doctor
server
project
inspect
query
graph
eval
render
export
patch
```

Mutation groups can come after the query/evaluation contracts are solid:

```text
add
edit
delete
rename
move
suppress
unsuppress
sketch
```

## Important project/server commands

```sh
puppycad doctor
puppycad server status
puppycad server start
puppycad project list --json
puppycad project create bracket --json
puppycad project inspect bracket
```

The `server start` command can launch the existing Bun server if needed, but the default CLI mode should be client mode.

## Important query commands

Queries are the main agent/script interface:

```sh
puppycad --project bracket query features --json
puppycad --project bracket query sketches --json
puppycad --project bracket query profiles --json
puppycad --project bracket query bodies --json
puppycad --project bracket query faces --body b1 --json
puppycad --project bracket query edges --json
puppycad --project bracket query bbox --body b1 --json
puppycad --project bracket query refs --target f4 --json
```

JSON should be boring and stable:

```json
{
  "features": [
    { "id": "f1", "type": "sketch", "name": "base_sketch", "status": "ok" },
    { "id": "f2", "type": "extrude", "name": "base_plate", "status": "ok" }
  ]
}
```

## Generated geometry queries

Generated topology must be inspectable and referenceable through the server evaluator:

```sh
puppycad --project bracket eval
puppycad --project bracket query faces --body main_body
```

Example human output:

```text
Faces:
 face:top source=f2 kind=cap normal=+Z area=3200
 face:bottom source=f2 kind=cap normal=-Z area=3200
 face:side[0] source=f2 kind=side from=edge:s1.e0
```

References should avoid fragile generated names where possible. Prefer semantic selectors once the engine supports them:

```sh
puppycad --project bracket add sketch --on 'feature(base_plate).cap(+Z)' --name hole_sketch
puppycad --project bracket add fillet --edges 'edges(body=main_body, adjacent_to=face:top)' --radius 2
```

## Selector syntax

A small selector language can be a later engine/API feature:

```sh
puppycad --project bracket query 'features(type=sketch)'
puppycad --project bracket query 'faces(body=main_body, normal=+Z)'
puppycad --project bracket query 'profiles(sketch=base_sketch, contains=[10,10])'
```

Commands can use selectors once they are server-supported:

```sh
puppycad --project bracket add extrude --profile 'profiles(sketch="base_sketch", contains=[0,0])' --distance 10
```

## Eval and explainability

Separate mutation from model evaluation:

```sh
puppycad --project bracket edit f2 --distance 20
puppycad --project bracket eval --explain
```

Example explanation:

```text
Recomputing from f2 base_plate

Dirty:
 f2 base_plate reason: parameter changed: distance
 f3 hole_sketch reason: depends on face:f2.top
 f4 mounting_cut reason: depends on f3

Clean:
 f1 base_sketch
```

## Patch system

AI agents should prefer structured server patches over low-level source rewriting:

```sh
puppycad --project bracket patch patch.json --dry-run
puppycad --project bracket patch patch.json --apply
```

Patch example:

```json
{
  "ops": [
    { "op": "set_param", "target": "f2", "param": "distance", "value": "20mm" },
    {
      "op": "add_feature",
      "type": "fillet",
      "name": "soft_edges",
      "edges": { "selector": "edges(body=main_body, adjacent_to=face:top)" },
      "radius": "2mm"
    }
  ]
}
```

Dry-run output should explain affected dependencies before applying.

## Dry-run where risk matters

Source/project-changing commands should support `--dry-run`:

```sh
puppycad --project bracket delete f3 --dry-run
puppycad --project bracket delete f3 --cascade
```

Dependency impact should be explicit.

## Rendering and export

Rendering should be first-class and server-backed:

```sh
puppycad --project bracket render --out preview.png
puppycad --project bracket render --view iso --out iso.png
puppycad --project bracket render --annotate --out annotated.png
```

Export:

```sh
puppycad --project bracket export --format glb --out preview.glb
puppycad --project bracket export --format stl --out part.stl --tolerance 0.05
puppycad --project bracket export --format step --out part.step --units mm
```

## Server API direction

The CLI should drive or help shape these endpoints:

- `GET /health`
- `GET /api/projects`
- `POST /api/projects`
- `GET /api/projects/:projectId`
- `PUT /api/projects/:projectId`
- `POST /api/projects/:projectId/commands`
- `GET /api/projects/:projectId/features`
- `GET /api/projects/:projectId/graph`
- `POST /api/projects/:projectId/eval`
- `GET /api/projects/:projectId/geometry`
- `POST /api/projects/:projectId/render`
- `POST /api/projects/:projectId/export`
- `POST /api/projects/:projectId/patch`

Some exist already; some are future API design targets.

## MVP order

Build server-backed read/evaluate/output before mutation:

1. `puppycad doctor` — can reach server, versions, health. Implemented for `/health`.
2. `puppycad project create/list/inspect` — project lifecycle through server. Implemented for `POST/GET /api/projects` and `GET /api/projects/:projectId`.
3. `puppycad inspect <project-id>` — human summary from server project state. Implemented; local file inspection remains available when the target is a file path.
4. `puppycad query features --json` — stable machine output with `{ projectId, features: [{ id, type, name?, partId, status }] }`. Implemented from the server project snapshot.
5. `puppycad graph --json|--mermaid` — dependency graph output. Implemented from feature references in the server project snapshot.
6. `puppycad eval --json|--explain` — evaluator status/debugging. Implemented as MVP project snapshot validation/materialization.
7. `puppycad query geometry --json` — generated bodies/faces/edges. Implemented from the server project snapshot, with `query bodies`, `query faces`, `query edges`, `query bbox`, and `--body <body-id>` filters.
8. `puppycad render --out preview.png` — server-backed preview.
9. `puppycad export --format glb` — first export path.
10. `puppycad patch --dry-run/--apply` — structured mutation.

Mutation commands (`add`, `edit`, `delete`) should come after stable server-side query/evaluation contracts exist.
