# puppycad

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
