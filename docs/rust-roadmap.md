# Native Rust app roadmap

This is the working plan for turning the native app in [`rust/`](../rust/) into
a complete CAD editor. [`rust-native.md`](rust-native.md) documents what works
today; [`sketch-parity.md`](sketch-parity.md) is the feature bar the web app
already meets. Phases are ordered by dependency: the solver unblocks almost
everything after it. Each item shows its final status.

## Phase 0 — Land and stabilize

1. Commit `rust/` and its docs on the `rust` branch. **Done.**
2. CI: build, test, fmt, blocking clippy and the Docker UI smoke
   (`.github/workflows/rust.yml`). **Done.**
3. Cross-loader round trip (`rust/tests/roundtrip.rs`,
   `test/rust-roundtrip.test.ts`, `--roundtrip`). **Done.**

## Phase 1 — Constraint solver

1. `rust/src/solver.rs`, Levenberg–Marquardt port of `src/sketch-solver.ts`. **Done.**
2. Edits as mutate → solve → commit, one undo step. **Done.**
3. DOF and conflict/redundancy diagnostics (`src/sketch-redundancy.ts`). **Done.**
4. Dragging anchors under constraints, solved every frame. **Done.**
5. Remove restrictions the solver makes unnecessary. **Partial:** pattern
   copies are regenerated rather than solved; H/V lines rotate only by
   quarter turns; Equal lines cannot be split, trimmed or extended.

## Phase 2 — Sketch tool parity

1. Canvas dimensions, point dimensions, reference, expressions, variables. **Done.**
2. Fillet and chamfer (line-line, the web scope). **Done.**
3. Mirror and symmetry. **Done.**
4. Offset, single and mitered chain (the web scope). **Done.**
5. Linear and circular patterns. **Partial:** no skipped instances, at most 64 copies.
6. Tangent arc and smooth join. **Done.**
7. Spline constraints to the web scope. **Done.**
8. Box selection, Shift multi-select, selection filters. **Done.**

## Phase 3 — Solid modeling

1. Extrude options and ghost preview. **Done.**
2. Sketch on a planar face, web-compatible frames. **Done.**
3. Revolve, loft, edge fillet and chamfer. **Partial:** stored as native
   extension nodes the web ignores; edge finishes only on straight edges
   between planar faces; web `chamfer` nodes open read-only.
4. Feature tree: reorder, suppress, rollback, cached re-evaluation. **Done.**
5. Export. **Partial:** binary/ASCII STL and 3MF done; STEP missing.

## Phase 4 — Assembly

1. Instances, connectors, all five mate kinds, servos, solver. **Done.**
2. Assembly view, insert/delete, face-to-face mate gesture, undo, export. **Done.**
3. Editing mates after creation (select, delete, servo angle), edge axes,
   world connectors, `--export` for assemblies, edge overlay and scene cache.
   **Done.** Mate limits and changing a mate's kind remain open.

## Phase 5 — Editor UX

1. Command palette and shortcut overlay. **Done.**
2. Hover pre-highlight, relation glyphs, inference hints. **Done.**
3. Undo history panel, cross-view undo. **Done.**
4. Toolbar icons, tooltips, status bar. **Done.**
5. Settings: units, grid, theme, window layout. **Done.**
6. Pixel font kept, UI scale for HiDPI. **Done.**

## Phase 6 — Code health (continuous)

1. Split `model.rs` and `main.rs` into modules; clippy clean and blocking. **Done.**
2. Unit tests for `renderer.rs` and input handling. **Done.**
3. Restructure `rust-native.md`; Native column in `sketch-parity.md`. **Done.**

## Remaining work

Consolidated from the Known gaps in [`rust-native.md`](rust-native.md#known-gaps):

- STEP export.
- Web-side support for native extensions: extrude end conditions and flip,
  suppression and rollback, revolve, loft and edge finish nodes, instance
  names, and solving non-fasten mates.
- Native support for web `chamfer` graph nodes (topology edges).
- Pattern copies solved instead of regenerated; skipped instances, more copies.
- Assembly mate limits, changing a mate's kind, servos on non-revolute mates.
- Inferred midpoint and intersection relations from snaps.
- Lifting the Equal/offset split-trim-extend block and quarter-turn rotation
  of H/V lines.
- Diameter circle tool; edge finishes on curved edges.
- Web-wide gaps shared by both apps: conics, text, curved-edge fillets,
  ellipse/spline offset, closed control splines, curvature constraints.
- Pattern prompts in the display unit.
