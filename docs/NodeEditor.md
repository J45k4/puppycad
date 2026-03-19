# Node Editor (Rust)

PuppyCAD now includes an in-workspace Rust node editor scaffold.

## Launch

From workspace root:

```bash
cargo run -p puppycad --
```

To open and visualize a specific `.pcad` file:

```bash
cargo run -p puppycad -- examples/puppybot.pcad
```

## Current behavior (M0)

- Parses the input `.pcad` source with `puppycad-core`.
- Builds a declaration dependency graph (`solid`/`feature` nodes).
- Opens an `egui` editor window with:
  - left panel: searchable project items list (`id`, `kind`, `op`, dependencies),
  - center panel: details for the selected item,
  - right panel: interactive 3D viewer with mode switch:
    - `Graph`: dependency graph nodes/edges,
    - `Result`: render-result preview derived from `build_render_state`.
- When launched with a `.pcad` file path, the window hot-reloads the file after on-disk edits and keeps the last good graph visible if a reload fails.
- `Result` mode supports:
  - `Wireframe`: debug triangulation edges,
  - `Solid`: filled shaded triangles with selected-part highlighting.
- Rendering backend:
  - native path: `eframe` + `wgpu` with an `egui_wgpu` paint callback for `Result` mode,
  - fallback path: CPU painter rendering when `wgpu` state is unavailable.
- 3D viewer controls:
  - right mouse drag: orbit camera,
  - middle mouse drag: pan camera target,
  - mouse wheel: zoom.

## Notes

- Graph editing is not implemented yet in this milestone.
- Canonical formatting support is available in `puppycad-core::format::format_file`.
