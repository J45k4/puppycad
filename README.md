# puppycad

## Engine CLI

The Rust engine binary is in `engine/`. Use the `parse` and `validate` subcommands to inspect and validate `.pcad` input:

```bash
cd engine
cargo run -- parse --json ../examples/puppybot.pcad
cargo run -- validate ../examples/puppybot.pcad
```

Useful flags:

- `--ast`: print the parser AST
- `--json`: emit renderer JSON output
- omit both for a declaration count
- no input file: reads `.pcad` from stdin

Validation command flags:

- `validate` reads from `FILE` or stdin when omitted
- `validate` returns non-zero on semantic parse/evaluation failures

Examples:

```bash
cargo run -- parse --ast - < ../examples/puppybot.pcad
cargo run -- validate --help
```

```bash
cargo run -- parse --ast - < ../examples/puppybot.pcad
```
