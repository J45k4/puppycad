# puppycad

## TypeScript models

PuppyCAD supports code-first `.pcad.ts` models. TypeScript is the editable source of truth; PuppyCAD evaluates it into a deterministic model graph containing bodies, component instances, frames, mates, and actuators. That graph compiles to the existing v4 project/geometry engine for rendering and export.

The tendon-free three-finger hand is a complete example:

```sh
bun run src/cli.ts inspect examples/three-finger-hand.pcad.ts
bun run src/cli.ts query bodies examples/three-finger-hand.pcad.ts --json
bun run src/cli.ts query assemblies examples/three-finger-hand.pcad.ts --json
bun run src/cli.ts render examples/three-finger-hand.pcad.ts --out hand.png
bun run src/cli.ts model compile examples/three-finger-hand.pcad.ts \
  --out hand.pcad \
  --graph hand.graph.json
```

The model uses a reusable finger component three times. Each instance creates stable hierarchical ids such as `three-finger-hand/index/proximal` and declares its frames, revolute/fixed mates, and servo. Compilation creates 48 part documents plus one assembly containing the corresponding instances, frame connectors, mates, and actuators.

```ts
import { capsule, component, defineModel, v2 } from "./src/model-dsl"

const link = component<{ length: number }, void>("Link", (part, { length }) => {
  part.body("body", {
    outline: capsule(v2(0, 0), v2(0, length), 8),
    depth: 6
  })
})

export default defineModel({ id: "arm", name: "Robot arm" }, (arm) => {
  arm.instance("upper", link, { length: 80 })
})
```

TypeScript model files execute as trusted local code. Generated `.pcad` and graph JSON files are build artifacts, not files intended for manual editing. See [docs/typescript-models.md](docs/typescript-models.md) for the model graph and assembly conventions.

## CLI

The package exposes a `puppycad` CLI entrypoint.

```sh
bun run src/cli.ts --help
bun run src/cli.ts --version
```

The CLI is server-first. By default it talks to `http://localhost:5337`; override that with `--server-url`, `PUPPYCAD_SERVER_URL`, or CLI config.

```sh
bun run src/cli.ts config set server-url http://localhost:5337
bun run src/cli.ts config set default-project <project-id>
bun run src/cli.ts doctor
bun run src/cli.ts project list --json
bun run src/cli.ts project create "Bracket" --json
bun run src/cli.ts --project <project-id> query features --json
bun run src/cli.ts --project <project-id> graph --json
bun run src/cli.ts --project <project-id> graph --mermaid
bun run src/cli.ts --project <project-id> eval --json
bun run src/cli.ts --project <project-id> eval --explain
```

Config is stored as JSON using camelCase fields:

```json
{
  "serverUrl": "http://localhost:5337",
  "defaultProject": "<project-id>"
}
```

Create a project file with an initial part:

```sh
bun run src/cli.ts init my-project.pcad --part-name "Bracket"
```

Inspect a server project or validate a local project file:

```sh
bun run src/cli.ts inspect <project-id>
bun run src/cli.ts inspect my-project.pcad
```
