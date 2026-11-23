# PuppyCAD

PuppyCAD is a native Flexscript editor for a single `.pcad` file. The application
starts at [main.flex](main.flex). Its window, viewport and editing tools live in
[src/](src/). The editor and Biscuit share the CAD solver and manufacturing code
from Flexscript's `libs/cad/`.

## Build and run

Set up the [local Flexscript SDK checkout](docs/flexscript-native.md#build-and-run)
under `workdir/flexscript/checkout` and put the `flexscript` bootstrap compiler
on your PATH. Then run from the repository root.

```sh
./run.sh drawing.pcad
```

The launcher builds and opens the native editor. Running `./run.sh` with no
arguments opens `untitled.pcad` in the current directory. An unused filename
starts a blank part. Ctrl+S saves to that path.
The same executable exports existing drawings headlessly.

```sh
workdir/flexscript/puppycad --export rust/fixtures/extruded-rectangle.pcad /tmp/block.stl
```

Run `./build.sh test` for the model regressions and the headless Xvfb window suite.

The **Design** sidebar shows the parts, sketches and features in the open
file. Expand a sketch to select its geometry, or select a feature to open
its history. F3 collapses or reopens the tree.

The [editor guide](docs/flexscript-native.md) covers controls, file formats,
and headless tests. The [migration plan](docs/flexscript-migration.md)
tracks CAD parity and the Biscuit integration.

The earlier [TypeScript web editor, CLI and SDK](typescript/README.md) and
[Rust desktop editor](docs/rust-native.md) provide reference implementations
and fixtures during the migration.
