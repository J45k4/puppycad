# Native Rust app roadmap

This is the working plan for turning the native app in [`rust/`](../rust/) into
a complete CAD editor. [`rust-native.md`](rust-native.md) documents what works
today; [`sketch-parity.md`](sketch-parity.md) is the feature bar the web app
already meets. Phases are ordered by dependency: the solver unblocks almost
everything after it.

## Phase 0 — Land and stabilize

1. Commit `rust/` and its docs on the `rust` branch. **Done.**
2. CI: build and test the Rust workspace on every push, plus the Docker UI
   smoke (`rust/ui-smoke.sh`).
3. Cross-loader round-trip test (**done**: `rust/tests/roundtrip.rs`, `test/rust-roundtrip.test.ts`): every `rust/fixtures/*.pcad` and every project
   the native app saves must load in the TypeScript `src/project-file.ts`
   loader, and web-saved projects that use only supported features must load
   natively. This is the only guard against format drift between the two apps.

## Phase 1 — Constraint solver

Every relation in `rust/src/model.rs` currently has hand-written propagation.
Replace it with a general solver so relations compose, dragging works under
constraints, and conflicts are diagnosed.

1. (Done) `rust/src/solver.rs`: parameters are every anchor coordinate and dimension
   value; one residual function per relation kind; Gauss–Newton /
   Levenberg–Marquardt with numeric Jacobians. Port the structure of
   `src/sketch-solver.ts` so results match the web app.
2. (Done for solver-backed relations) Edits become mutate → solve → commit as one undo step. Keep the current
   "anchor the first endpoint" behavior as soft weights so solved results stay
   intuitive.
3. (Done) Port `src/sketch-redundancy.ts`: DOF count, over-constrained and conflicting
   relation diagnostics, shown in the header and as per-entity colors.
4. (Done) Dragging under constraints: a temporary target for the dragged anchor,
   solved every frame.
5. (Done for relation propagation: every relation kind is solver-backed; rotating
   direction targets, scaling Equal ellipses and transforming splines with
   linked points are allowed) Remove restrictions the solver makes unnecessary (90-degree-only rotation of
   constrained lines, Equal blocking split/trim/scale, and similar).

## Phase 2 — Sketch tool parity

Each item is ported from the matching `src/sketch-*.ts` together with its tests.

1. Dimensions drawn in the viewport with click-to-edit labels; point
   distance / X / Y dimensions; driving vs. reference; expressions and sketch
   variables (`sketch-value.ts`, `sketch-variables.ts`).
2. Fillet and chamfer.
3. Mirror and the symmetry constraint (**done**: `rust/src/model/symmetry.rs`;
   Shift+S symmetry gesture, Shift+I / MORE → MIRROR).
4. Offset, single and chain.
5. Linear and circular patterns.
6. Tangent arc and smooth join.
7. Remaining spline constraints.
8. Box selection, Shift multi-select, selection filters.

## Phase 3 — Solid modeling

1. Extrude options: direction, symmetric, through-all, up-to-body; preview
   before commit.
2. Sketch on a planar face: face picking in the 3D view, with the plane frame
   derived as `src/solid-model.ts` does so files stay web-compatible.
3. Revolve, loft (`loft.ts`), edge fillet and chamfer (`edge-finish.ts`), all
   through `manifold-rust`.
4. Feature tree with reorder, suppress and rollback; re-evaluation when an
   upstream sketch changes.
5. Export: binary STL, then 3MF or STEP.

## Phase 4 — Assembly

Instances, frames, mates and the `assembly-solver.ts` port; assembly view with
per-instance selection and export.

## Phase 5 — Editor UX

1. Searchable command palette and a shortcut overlay.
2. Hover pre-highlight, relation glyphs beside entities, inference hints while
   placing.
3. Undo history panel.
4. Toolbar icons, tooltips with shortcuts, status bar with cursor coordinates,
   DOF and snap mode.
5. Settings: units, grid size, theme; persisted window layout.
6. Keep the pixel font; add a UI scale factor for HiDPI.

## Phase 6 — Code health (continuous)

1. (Done) Split `model.rs` and `main.rs` into modules by concern; clippy is
   clean with `--all-targets` and blocking in CI.
2. Unit tests for `renderer.rs` and for input handling.
3. Restructure `rust-native.md` into sections and add a Native column to
   `sketch-parity.md`.
