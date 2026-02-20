# puppycad

## Engine CLI

The Rust engine binary is in `engine/`. Use the `parse`, `validate`, and `render` subcommands to inspect, validate, and preview `.pcad` input:

```bash
cd engine
cargo run -- parse --json ../examples/puppybot.pcad
cargo run -- validate ../examples/puppybot.pcad
cargo run -- render ../examples/puppybot.pcad
```

Useful flags:

- `--ast`: print the parser AST
- `--json`: emit renderer JSON output
- omit both for a declaration count
- no input file: reads `.pcad` from stdin

Validation command flags:

- `validate` reads from `FILE` or stdin when omitted
- `validate` returns non-zero on semantic parse/evaluation failures

Render command flags:

- `render` reads from `FILE` or stdin when omitted
- `--headless`: render without windowed graphics
- `--camera <X Y Z>`: camera position in world coordinates
- `--look-at <X Y Z>`: point the camera looks at
- `--iterations <N>`: run for N engine iterations/frames; defaults to 1 in headless or screenshot mode
- `--output <PATH>`: write a screenshot frame to a specific file path
- `--output-dir <DIR>`: write a screenshot frame into a directory (defaults to `workdir/screenshots`)
- `render` currently previews only `box` and `translate`-based solids

Examples:

```bash
cargo run -- parse --ast - < ../examples/puppybot.pcad
cargo run -- validate --help
```

```bash
cargo run -- render --headless --iterations 10 ../examples/puppybot.pcad
```

```bash
cargo run -- render --headless --iterations 1 --camera 0.0 0.0 10.0 --look-at 0.0 0.0 0.0 --output ./captures/puppybot.png ../examples/puppybot.pcad
```

```bash
cargo run -- render --headless --iterations 1 --output ./captures/puppybot.png ../examples/puppybot.pcad
```

```bash
cargo run -- render --headless --iterations 1 --output-dir ./captures ../examples/puppybot.pcad
```

```bash
cargo run -- parse --ast - < ../examples/puppybot.pcad
```
