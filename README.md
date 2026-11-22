# PuppyCAD

PuppyCAD is a native Flexscript editor for a single `.pcad` file. The application
starts at [main.flex](main.flex). Its window, viewport and editing tools live in
[src/](src/). The editor and Biscuit share the CAD solver and manufacturing code
from the sibling Flexscript checkout at `../flexscript/libs/cad/`.

## Build and run

Build from the repository root with the Flexscript SDK next to this checkout
and the `flexscript` compiler on your PATH.

```sh
./build.sh
workdir/flexscript/puppycad drawing.pcad
```

An unused filename opens a blank part. Ctrl+S saves to that path.
The same executable exports existing drawings headlessly.

```sh
workdir/flexscript/puppycad --export rust/fixtures/extruded-rectangle.pcad /tmp/block.stl
```

Run `./build.sh test` for the model regressions and the headless Xvfb window suite.

The [editor guide](docs/flexscript-native.md) covers controls, file formats,
and headless tests. The [migration plan](docs/flexscript-migration.md)
tracks CAD parity and the Biscuit integration.

The earlier [TypeScript web editor, CLI and SDK](typescript/README.md) and
[Rust desktop editor](docs/rust-native.md) provide reference implementations
and fixtures during the migration.
