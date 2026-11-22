# Native Flexscript CAD migration

Build PuppyCAD as a native Flexscript editor on the `flexscript` branch. Put the CAD model and algorithms in `flexscript/libs/cad/` so the editor, Biscuit programs, and Biscuit project builds evaluate the same design through the same implementation. The starting PuppyCAD revision is `e9556cb`. Its Rust and TypeScript implementations provide reference inputs, behavior, and tests throughout the port.

The application entry point is `main.flex` at the PuppyCAD root. The Flexscript
UI is in `src/` and the headless editor tests are in `tests/`. The previous
web app, CLI, SDK, examples and Bun tests live together under `typescript/`.

## Ownership

The native editor opens one `.pcad` file per window. Opening a file goes
directly to its drawing, and Save writes back to that file. A new file starts
with a blank part. The Rust and TypeScript multi-project list and project
management commands are outside the native migration. A single file can
still contain multiple parts, assemblies and their references. Biscuit
continues to manage its own workspaces and frozen builds.

| Owner | Responsibilities |
| --- | --- |
| Flexscript CAD library | Document load/save, stable entity and feature identities, sketch constraints and diagnostics, feature evaluation, solid geometry, assembly mates and motion, interference checks, tessellation, and supported manufacturing exports. Expose these through headless Flexscript entry points. |
| PuppyCAD | Native window, editor commands, selection, tool gestures, undo/redo interaction, panels, camera, picking, and viewport drawing using Flexscript's GUI and rendering libraries. Pass model-space edits to the library and draw its results. |
| Biscuit | Saved program imports, checked VM integration, project permissions, frozen inputs, build profiles, artifact storage, and preview controls. Consume a pinned CAD library for both interactive programs and project builds. |

For example, dragging a constrained endpoint starts an edit in PuppyCAD. The library solves the new position and reports conflicts. PuppyCAD previews the result and records the accepted edit as one undo step. A headless caller can supply the same target and inspect the same result.

## Milestones

| Order | Deliverable | Evidence to advance |
| --- | --- | --- |
| 1 | **Extract one working model.** Inventory existing CAD entry points, import paths, fixtures and callers in all three repositories. Start with Biscuit's two-hole mounting plate. Move its geometry, bounded document evaluation, mesh generation and STL path into the Flexscript library. Resolve its dependency on Biscuit's JSON and runtime helpers. | Build the plate from the same input in a standalone Flexscript program and Biscuit's checked VM through its existing import. Verify dimensions, both holes, mesh closure, volume and binary STL using the existing geometry fixture. Exercise invalid input and exhausted budgets. |
| 2 | **Open and edit a PuppyCAD part.** Implement the first v4 `.pcad` load/save, constrained sketch edit, extrusion and export in the library. Build a native Flexscript editor that opens the part, draws the sketch and solid, edits a driving dimension and supports undo. | Open a fixture, change the dimension, inspect the rebuilt solid, undo and redo, save, reopen, and inspect the exported STL. Compare solved coordinates and dimensions with the reference editor. Capture the actual native viewport under `workdir/`. |
| 3 | **Port remaining editor and model behavior.** Use the [sketch parity inventory](sketch-parity.md), [native feature guide](rust-native.md), existing Rust and TypeScript tests, and representative saved projects. Cover sketch tools, relations, DOF and conflict reporting, feature history, face sketches, supported solid operations, assembly editing and STL/3MF output. Track behavior used by the CLI, SDK and code-authored models when replacing their underlying evaluator. | Run reference and Flexscript fixtures side by side. Compare IDs and references on round trip, geometry within documented tolerances, and export structure. Exercise read-only handling of unsupported records. Open, edit and reopen representative parts and assemblies through the native UI. |
| 4 | **Complete Biscuit CAD parity.** Port rigid transforms, booleans, drilled boards, reusable instances, fixed/revolute/slider/screw mates, coupled drives, pose evaluation, sampled collision checks, playback transforms and software-rendered snapshots. Preserve PuppyCAD's free-placement mate solving alongside Biscuit's specified-pose evaluation. Exercise component and PCB-to-CAD adapters with the shared geometry. | Run Biscuit's geometry, assembly, spatial, playback and project fixtures. Check the phone holder at a known interfering pose, the board in its enclosure, and the gripper motion samples. Verify the checked VM still reports errors for invalid poses and incomplete checks. |
| 5 | **Switch consumers.** Point PuppyCAD and both Biscuit execution paths at the shared implementation. Preserve saved `/biscuit/cad/...` imports through compatibility modules. Pin the new library in the build profile and preserve the identity of historical builds. Retire duplicate production implementations after their callers pass. | Run a saved Biscuit program and a new frozen project build through the ordinary product interfaces. Check preview, reports and downloadable artifacts. Repeat the frozen build and compare deterministic artifact hashes. Verify existing historical artifacts still resolve under their original profile. |

In the sibling Biscuit checkout, use `cad/README.md`, `cad/tests/` and `docs/native-project-builds.md` as the compatibility references. Keep electrical netlist and KiCad behavior covered through the board-to-CAD integration. Record numerical expectations per operation when comparing Biscuit's integer-nanometre geometry with PuppyCAD's floating-point solvers.

## Biscuit standalone PCAD studio

Use `product/pcad-standalone` at `ea4670d8cfdb78ac058617ed9703f38e479a9f1e`
as a second reference alongside Biscuit master. Its standalone studio opens
one `puppycorp.pcad` schema-version-1 document. PuppyCAD's existing fixtures
use v4 `items` and feature graphs. Exercise both formats through native open,
edit, save, reopen and export. Preserve their existing identifiers, unknown
fields, units and serialization on save. Biscuit owns folder navigation,
permissions, revision receipts and draft restoration. [Reference commit](https://github.com/puppycorp/biscuit/commit/ea4670d8cfdb78ac058617ed9703f38e479a9f1e)

| Area | Native migration and acceptance |
| --- | --- |
| Layout and selection | Use the compact studio as the interaction reference. Keep the drawing prominent, provide a collapsible, searchable object tree and show properties for the selected object. Pick the nearest visible occurrence, highlight its own geometry, double-click to isolate it and restore the full assembly. Verify narrow-window access to tools. |
| Camera | Middle-drag pans. Right-drag orbits about the surface point under the pointer, and a tangential right-drag at the rim rolls. Provide zoom, fit, standard views, orientation cube and orthographic/perspective switching. Open standalone drawings in orthographic view. Keep the projected orbit point within one logical pixel and preserve camera center, orientation and scale across projection switches. Check that standard views restore the fitted framing after pan and zoom. |
| Hinges and occurrence pins | Resolve a picked part or fixed child to its independent revolute mate. Follow the picked material point around the solved world axis, respect limits, preview rigid followers locally, cancel without editing, and commit one pose on release. Fix and unfix an occurrence by its full path. A pinned child stays in place when the hinge moves. Cover suppressed, locked, coupled and nested joints. |
| Shared evaluator | Port the branch's sketch constraints, dimensions, parameters, attached profiles, revolve, shell, edge treatment and board outlines. Port nested and patterned assemblies, joint coordinates, fixed occurrence transforms and solved `hinge_frame` metadata to `flexscript/libs/cad`. Check existing Biscuit imports and frozen builds against branch fixtures, including board and electrical consumers of geometry. |

The reference also distinguishes a stationary right-click on an occurrence (Fix or Unfix) from a right-drag (orbit). Verify that a cancelled hinge drag preserves the saved pose and returns the viewport to its original pixels.

The branch's `tests/pcad_studio.py` contains executable camera, picking,
drag/cancel/commit and fix/unfix expectations. Use those assertions alongside
native window captures. Browser embedding remains the later WASM milestone.

## Migration evidence

The sibling Flexscript checkout now exposes constructive solids, mesh/STL,
assembly evaluation and playback through `import "cad";`. Biscuit forwards
its previous import paths to the SDK. Its project checker imports the same
code through `cad/api.flex`. The SDK also renders orthographic RGB snapshots.
Biscuit wraps the pixels in its existing image value and owns preview
delivery, electrical, component and gripper integrations.

The two-hole mounting plate builds at 80 × 50 × 6 mm with 932 triangles.
The pre-extraction Biscuit fixture, the standalone SDK, the checked VM, and
two invocations of Biscuit's project check tool produced identical 46,684-byte
STLs (SHA-256 `f6b97e5c6580b70614e0122daf9cc2f78d6b46d1b1da579ac70d294bcc506adc`).
The SDK's `libs/cad/tests/geometry.flex` exercises its public API. Biscuit's
`tests/shared_cad.py` compares the VM mesh and STL against native output,
alongside the existing geometry, spatial and assembly suites.

Biscuit's build scripts and CI read the same SDK commit from
`flexscript-sdk.rev`. The local SDK branch `puppycad-cad-sdk` at
`657bb4755c5fc083d80c0e5203207d557f49f4a8` includes the shared library and
the FlexDB blob APIs required by the backend. Its default local build
bootstraps the compiler from the pinned revision. PuppyCAD and Biscuit
produced byte-identical compilers. Biscuit's checked VM matched the
archived VM across all 45 historical CAD and runtime import paths. The
`0eec321` headless browser tests passed desktop and mobile builds, protected ZIP
downloads, and integrated PCAD fabrication with the pinned KiCad
executable. The KiCad v3/v4 upgrade preserves the earlier fabrication
ZIP and produces matching board data and rear silkscreen in new builds.
The earlier SDK pin verified v10 artifact lineage. Hosted CI verification
requires publishing the SDK commit.

Biscuit's CI script runs the archived geometry, spatial and assembly
regressions. Native and archived checked guests reproduce the seven gripper
poses and four orthographic RGB snapshots. The checked VM returns the same
errors for out-of-travel poses and an incomplete collision sweep. A change
of SDK within driver v11 preserves the archived release ZIP and journal
while rejecting new admissions under the older fingerprint.

The native editor now authors external and internal tangency for circles
and circular arcs. A construction arc and a follower circle retain their
contact after a radius edit. The checked VM and two v11 frozen jobs
produced the same STL as the native editor. The same three consumers
matched on face-attached TypeScript chamfers from top, mirrored bottom
and aligned side faces. Face selectors still require matching the source
face ordering to rebuild the referenced edge.

The assembly editor can rename a connector ID and attach its frame to an
occurrence or World. The native test checks that the mate retains its
endpoint after both edits, rejects an attachment to the mate's opposite
occurrence and compares the saved assembly export with Rust.
The checked VM and two v11 frozen workers reproduced the native STL and
its three solved occurrences from the resulting assembly graph.

The native editor also moves an existing sketch to a reference plane or
extrusion face. The face picker only shows the geometry before its first
consuming feature. The test reattaches a sketch to Front and then to the top
of a 60 mm block. It exercises undo and redo and checks that the saved file
retains the local outline and extrusion references. Rust confirms the
reattached 3,840 mm³ solid and the original 960,000 mm³ block. Biscuit's
checked VM replays the same edit and two v11 frozen jobs match the native
STL bytes. The SDK rejects support cycles and edits that lose projected
edges. See `workdir/flexscript/sketch-support-picker.png`.

Whole-curve Tangent includes lines paired with circles or circular arcs.
The saved v4 relation resolves a line and curve in either order against the
line's supporting direction. A circular arc must include the contact in
its signed sweep. The headless editor changes the tangent circle's diameter
from 20 to 24 mm, reopens, extrudes and matches Rust's 3MF. The checked VM
and two v11 frozen jobs match the native STL bytes.

Line tangency also supports ellipses and elliptic arcs. The native test
changes the width from 40 to 48 mm and the rotation from 30° to 45°, then
undoes, redoes, reopens and extrudes. Rust verifies the resulting 3MF.
Midpoint supports saved line, circular-arc and elliptic-arc relations.
The native test changes a construction line from 20 to 30 mm and its
point follows from 10 to 15 mm. Rust checks the reopened geometry. Biscuit's
checked VM reconstructs the saved relation and two frozen builds match
the native STL bytes.

Point on curve supports a saved point, line endpoint or supported center
on a finite straight line, closed circle or finite circular arc. The native
test authors the line relation and shortens its guide until the point
reaches the endpoint.
It also opens an imported circular relation, changes the diameter from 40
to 50 mm, undoes and redoes the change, extrudes and reopens the part. The
saved relation keeps its original ID. Rust agrees on the reopened 3MF. The
checked VM and two frozen builds match the STL and reject a stale contact.
For a clockwise arc, the native test changes the radius from 20 to 25 mm
and the sweep from −90° to −30°. The follower moves to (21.650635, −12.5) mm
at the retained endpoint. The saved relation ID round-trips through Rust.
The checked VM and two frozen jobs match this STL. A point on the discarded
sweep fails the mesh check before export.

The same relation now supports an ellipse and a finite elliptic arc. The
headless editor changes a rotated ellipse from 40 to 46 mm wide and moves
its constrained point. A clockwise construction arc changes from −120° to
−60°, moving its point to the new endpoint and keeping the circle STL
byte-identical. The SDK also exercises an independently locked coordinate,
an off-sweep conflict and a drag along the curve. Against TypeScript's
nearest-arc routine, 576 differential cases agree within 1 nm. The native
picker also creates the relation on a sketch containing an ellipse and a
point.

Finite elliptic arcs support directed smooth joins through their start or
end. The native editor creates a join and reopens a TypeScript-authored
clockwise join after a height edit from 10 to 18 mm. The attached 12 mm
line follows the same endpoint and retains its direction relative to the
arc. Against original TypeScript, 144 sampled endpoint frames agree within
0.6 nm. Construction geometry keeps the independent solid STL byte-identical
before and after the edit.

The shared solver handles a point or line endpoint on an open control,
open fit or periodic fit spline. The headless editor drags across both
branches of a saved self-intersection, undoes and redoes the drag, and
reopens the saved file. The chosen point agrees with TypeScript within
1 nm and the construction spline keeps the exported solid byte-identical.
The editor also creates a contact at a periodic seam and moves its follower
when a source handle reshapes the spline. The solver preserves the other
controls and rejects a source edit that conflicts with a fixed follower.
The periodic edit agrees with the TypeScript nearest point within 1 nm.
The native v4 mesh keeps its existing sampling. Its vertices and sampled
face centers are within 0.0075 mm of the original TypeScript export.
Editing and exporting this curved solid use separate checked calls under
the existing Biscuit time limit. Two v11 builds for each of the crossing
drag, periodic source edit and PartBuilder handle drag match their native
STLs byte for byte. The `0eec321` archive passed all 74 SDK tests.

The shared SDK evaluates original TypeScript PartBuilder v4 files in their
`features` and `solidSteps` representation. Six reference fixtures cover a
translated intersection, face sockets before and after a source translation,
a transverse through-all cut, taper with a revolve, and a partial revolve.
In the editor, **History** changes a legacy intersection's depth and X
translation and rejects a disjoint edit and a dependency-breaking reorder.
The saved file retains its original format and sketch references. The
reopened 720 mm³ export matches TypeScript. The checked VM reconstructs the
edited source and two v11 workers emit the same STL bytes. The authenticated
Files and frozen download flows preserve all six original source revisions
and their checked artifact hashes. Six additional circular loft and shell
fixtures verify closed geometry, side offsets, rim area and cavity floor
area against the original exporter. The native test changes wall thickness
from 2.5 to 3.25 mm and loft height from 30 to 32 mm, retains the original
shell source ID and reopens the saved model. The checked VM repeats those
edits and two frozen v11 builds match the native STL. A saved four-line
profile also evaluates without changing its original JSON entities.
Legacy PartBuilder profile chamfers and fillets retain the selected corner,
the order of sequential finishes and the original aperture. The native
History test edits a chamfer from 3 to 4 mm and rejects an overlapping
setback. After undo, redo, save and reopen, its 1,856 mm³ result has the
same bevel corners as TypeScript. The SDK also checks the sampled fillet on
a partial revolve and exposes fillet radius and segment fields. The checked
VM and frozen worker match all five accepted exports. Sampled curves require
a saved outline that agrees with the evaluated vertex order.

Saved TypeScript topology chamfers and fillets preserve the original edge ID.
The editor changes two chamfer setbacks and a fillet radius and segment count,
undoes and redoes the edits, then saves and reopens. The watertight native
exports measure 3,943.125 and 3,961.485 mm³. The SDK also preserves multiple
filled regions of an extrusion as a single Boolean tool. In the joined
fixture, the editor changes the primary depth from 8 to 10 mm and moves the
second region from X = 28 to X = 32 mm. The downstream boss retains its
original face ID. The reopened 2,384 mm³ solid matches the original
TypeScript geometry. Biscuit's checked VM and pinned v11 worker reproduce
the native saves and STL bytes. The same tests reject invalid finish values
and a filled-region extrusion with an explicit topology finish.

TypeScript's saved `sketchArc.arc` form uses its original 0.01 mm adaptive
chord tolerance. The headless editor changes the profile height from 10 to
12 mm and edits a construction arc's width and clockwise sweep. The saved
file retains both wrappers, IDs and signed angles. The native STL and the
original TypeScript export of that file are watertight. Their bounds agree
within 0.000002 mm and their volumes within 0.00005 mm³. Existing Rust-style
elliptic arcs keep their sampling. Biscuit's checked VM repeats the native
edit and two frozen v11 builds match the native STL bytes. The `a492df1`
pin passed all 70 headless SDK tests.

Original PartBuilder sketches with supported lines, circles, rectangles and
points open in the native sketch workspace. The headless test edits a driving
line dimension from 20 to 24 mm, undoes and redoes it, saves and reopens the
extrusion. It preserves the original entity, relation, profile and feature
IDs. Narrowing the outline so the same profile ID would identify the bore
fails before it changes the document. The original TypeScript exporter reads
the native save and reproduces its 2,206.033 mm³ solid and through-hole rim.
The checked VM repeats the width edit and matches the native saved graph.
Two frozen builds match the native STL byte for byte. The same editor adds
a construction point to an original sketch-only file.
Original PartBuilder circular and elliptic arcs also edit in place. The
headless editor changes the ellipse height from 10 to 12 mm and regenerates
the outline cache in its original order. The downstream boss retains saved
face `body-solid-face-39`. The reopened 777.025610 mm³ solid and the
original TypeScript export differ in volume by less than 0.000008 mm³.
The SDK rejects an edit that would change a referenced cap into a wall.
The Biscuit checked VM repeats this edit and the two point contacts.
Two frozen v11 builds of each model match the native STL byte for byte.
The `da58397` pin passed 72 headless CAD SDK tests.

A native cubic handle drag on an original PartBuilder sketch preserves
its control records and the boss on face `body-solid-face-53`. A drag that
would change the sampled face ordering is rejected. The reopened 957.831460
mm³ STL agrees with the original TypeScript exporter within 0.00001 mm³.
Native editing also covers the points of a closed fit spline. The SDK
checks open fit splines with saved endpoint coincidences and sampling close
to TypeScript's subdivision threshold. On a 108-edge periodic fixture,
checked save and checked export fit the existing instruction budget when
run as separate executions.

PartBuilder grid and circular patterns edit in the original saved format.
The native editor resizes a 3 × 2 grid to 2 × 3 in one edit and retains
its boss on `plate-solid-face-30`. The surviving copies keep their IDs.
The SDK rejects an edit that would redirect an attached face, a consumed
profile or an edge finish. The native editor also creates a new grid and
drags an anchored circular pattern with a −180° sweep. Original TypeScript
reopens the edited files. The grid and circular exports have matching
closed meshes, bounds and signed volumes.

A TypeScript v4 plate with a sampled half-ellipse aperture retains its
saved chamfer. The outside edge fixture reopens at 2,985.661458 mm³ after
editing both setbacks. An inner edge fixture reopens at 1,477.697502 mm³
after setting 0.2 and 0.3 mm. The guard certifies the original ordered
samples on a single canonical half-ellipse and its closing chord, including
original TypeScript files whose seams start on either entity.

The shared `55f2890` archive passed all 76 SDK tests. Biscuit's checked VM
replayed each native grid, circular pattern, elliptic join and outer bevel
edit. Two frozen v11 builds per model produced STL bytes identical to
PuppyCAD's exports and repeated the same reports.

Saved Normal constraints rebuild on closed ellipses and finite elliptic
arcs. PuppyCAD tested an authored Normal while resizing its ellipse and a
saved Normal while changing the height of a signed arc. Both files reopened
with their relations and exported geometry intact. The `540b5ff` archive
passed 77 SDK tests. Biscuit's checked VM reproduced both saved graphs and
four frozen v11 builds matched the native STL bytes. The checked ellipse
fixture authors the relation and changes the width in separate calls.

The native editor also reopens imported Normals after increasing a circle's
radius, shortening a signed circular sweep, and lengthening a finite line.
The affected endpoint stays on its source, while the archived relation ID
and the follower's driving length survive. The circle rebuilds its solid.
The two construction guides leave their independent solids byte-identical.

Direct PartBuilder ellipse axes and rotation, capsule width, and regular
polygon radius, rotation and side count edit in the original records. The
profile cache follows the TypeScript sample order. The tests preserve an
attached ellipse cap and reject a polygon side count that would redirect an
attached face or selected corner. Biscuit's checked VM and two frozen v11
jobs per case reproduced the saved native data and STL bytes across these
three forms, the five Normal cases, and the sampled inner chamfer.
The previous `667b2cc` SDK passed all 80 CAD test programs.

Saved spline Normals follow control, open fit and periodic fit splines.
The native editor changes a driving line length, moves open and periodic
spline points, and edits a Normal saved by TypeScript on the retained
half of a split spline. The imported coincidence and Normal keep their
IDs. The dependent periodic extrusion rebuilds. Analytic nearest-point
pruning and reusing validated constraints put the periodic edit, reopen
and export within Biscuit's existing checked VM instruction budget.

Original PartBuilder capsules retain their width expression, named cap
centers, measured span and attached face when edited. The native **Dimensions**
report shows the driving or measured width. Editing a sketch variable
rebuilds the capsule and its linked copies without moving either cap.
The editor previews and commits a free cap drag while rejecting fixed cap
dragging and edits that would redirect a saved face. At the `657bb47` pin,
82 SDK test programs passed. Biscuit's checked VM and 12 frozen v11 jobs
reproduced the six native slot and spline Normal edits and their STL bytes.

Original v1 Biscuit builds preserve the 16-file split profile and the
21-file standalone profile. The migration test creates archived releases
with the original binaries and real KiCad, then verifies the same authorized
ZIP, native artifacts and both fabrication sets after the portal upgrade.
With the original worker and checker granted, rerunning a version with a
completed receipt preserves its frozen source and deterministic artifacts.
Current source and membership permissions still govern those reads and replays.

Standalone History previews suppression, restoration and feature ordering
before Apply writes an edit. It also previews an earlier feature of the
selected part while leaving its saved output intact. With the original
Biscuit history fixture, the native editor and the current and archived
checked VMs agree on the 1,146 mm³ active part and the 1,200 mm³ suppressed
part. Two frozen v11 jobs retained their matching artifacts after restart.
The GUI regression also checks earlier-state previews in a two-part file.
When a known final modifier fails, the native editor can enter History if
both an earlier state and suppression produce a closed body. The GUI checks
that preview and Cancel preserve the source bytes, save and export stay
blocked until Apply, and Undo can return to the failed state. The checked
worker rejects the original invalid model and exports the repaired body.

The pinned compiler and verifier scanner share a 4,096-function source
limit. PuppyCAD and the assembled Biscuit portal build through the direct
pipeline. Compiler tests cover calls at the limit, including annotated
source, and rejection of an extra declaration.

The SDK's `cad_render_rgb` and the existing
`/biscuit/cad/render.flex` import reproduced the original renderer's
ISO, top, front and right RGB bytes. Tests covered a two-hole plate and
cover, depth occlusion in both part orders, and the saved render example.
The Biscuit program endpoint delivered the same authenticated PNGs before
and after a backend restart with the new SDK. Earlier snapshot URLs
continued to return their original PNG bytes.

Two frozen Biscuit project builds reproduced the mounting plate STL. A
subsequent version built the selected part from the PuppyCAD v4 face-sketch
fixture. Its STL matched the native editor's export byte for byte (SHA-256
`50b43e6d2d7326dc25617f6447b3250ff505235aec9c3c4c6cdb3ad4bb5ed30a`).
After switching worker profiles, Biscuit served the historical STL, ZIP and
profile and rejected a new build against the unavailable original profile.
The project worker also rejected invalid selections and empty assemblies.
Checked-program tests exercised the phone holder, assembly playback, gripper
and board-in-enclosure examples.

The [native Flexscript editor](flexscript-native.md) now opens, edits, saves
and exports rectangular extrusions. All 16 Rust v4 fixtures passed JSON
round-trip checks. Eight export a solid at their saved selection in both
editors. Both 3MFs contain closed bodies with matching titles, body names,
bounds and signed volumes. The other eight return an export error in both editors.
All 16 fixtures pass the native editor’s document access check. On
`tangent-arc.pcad`, dragging the end of the 40 mm line to 50 mm carries
the arc and fit spline with it. The headless window test cancels a drag,
commits and undoes a second drag, removes and restores the line-to-arc join,
then closes the profile with a tangent arc from the free spline endpoint.
After save and reopen, the native extrusion matches the Rust 3MF in bounds,
body name and signed volume. Biscuit’s checked program adds the closing arc and
10 mm extrusion to the same drawing. Two frozen worker runs produce identical
STL bytes with the Rust reference volume of 67,199.537 mm³. Moving a spline’s
middle point out of tangency makes the worker reject the export.

The native dimensions tool also changes a 40 mm line to 50 mm while keeping
its start fixed. The joined arc moves 10 mm and retains its radius, sweep
and relation ID. An independent driving circle in the same sketch stays
at its saved diameter. The window test undoes, redoes, saves and reopens the
edit, then closes and extrudes the outline. Its 3MF matches Rust. Biscuit's
checked VM reproduces the native part, and two frozen worker runs export its
exact STL. The SDK rejects an incompatible follower constraint or a solid
rebuild failure without publishing the edit. The dimension report shows an
upper bound for degrees of freedom while smooth joins are active. Edits
that require deforming a closed smooth chain still need a general solve.
See `workdir/flexscript/driving-tangent.png` and
`workdir/cad-migration/mixed-smooth-consumer-proof-actual.json`.

The shared line solver also accepts saved parallel and perpendicular
relations together with fixed points, coincident endpoints and driving
lengths. A perpendicular relation keeps the selected source line in place
while rotating its 10 mm follower. After driving the follower to 15 mm,
the two lines still meet at the same right angle. The native editor can
remove and restore the relation in one undo step. The library rejects an
incompatible fixed endpoint or a saved direction that disagrees with its
geometry before export. The window test saves and reopens the right-angle
sketch, closes it and extrudes a 1,500 mm³ triangular prism. Rust matches
its bounds and volume. Biscuit's checked VM reproduces the native part and
two frozen jobs produce the same STL. See `workdir/flexscript/line-perpendicular.png`.

The native **Relations** control also links straight-line lengths with
Equal. It preserves the parallel or perpendicular relation and the saved
coincident and fixed anchors. The window test drives the follower to 25 mm,
undoes that change, then drives the source to 30 mm and reopens and extrudes
the linked profile. Rejected dual drivers leave the original sketch and any
downstream solid intact. The saved 9,000 mm³ profile extrusion matches Rust's
3MF, and Biscuit's checked VM and two frozen builds reproduce the native
STL. See `workdir/flexscript/line-equal.png`.

Concentric relations tie the centers of saved circles, ellipses and their
arcs. The native window test picks a circle and a rotated ellipse, then
drives their shared center from X 10 mm to X 16 mm. It preserves both sets
of dimensions and the saved relation IDs through undo, redo, save and
reopen. The resulting annular extrusion matches Rust's 3MF. The checked VM
and two frozen builds reproduce the native STL. The SDK also rejects
incompatible fixed centers and saved geometry that disagrees with a
named Concentric relation. See `workdir/flexscript/concentric-seat.png`.

The native editor also authors three-point ellipses and circles. The
headless window test draws a 50 × 20 mm ellipse rotated 53.130102° and
places a 4 mm radius hole using three points on its rim. Undo and redo
preserve both primitive definitions. After save and reopen, the 10 mm
extrusion matches the Rust 3MF and the volume computed from the ellipse’s
128 segments and the hole’s 64 segments. Biscuit’s checked VM creates the
same CAD graph, including the feature and profile IDs. The native file records
three accepted edits in its revision. The checked program constructs the graph
at its initial revision. Two frozen project builds produce byte-identical STLs.
Four rotations of a 420 × 18 mm ellipse, including odd segment counts,
exercise the full-precision sampler. Each sampled STL rim vertex lies within
0.00002 mm of the analytical ellipse at its segment angle and 0.00015 mm of
the corresponding Rust export.

The headless native window test creates a linear grid from a picked circle
and a circular pattern from a picked line, then attaches the rotation center
to a sketch point. In Rust's `patterns.pcad`, the test resizes a 3 × 2 grid
to 2 × 3 with 25 mm column spacing and 12 mm row spacing. Surviving cells
keep their IDs. It changes a four-position full turn to three positions
over −180°, undoes and redoes the edit, saves and reopens the file. The
sketch reports nine degrees of freedom. Changing the source diameter from
8 to 10 mm updates all six circles. A 10 mm extrusion of the first disk
matches the Rust 3MF in closure, bounds and signed volume. The SDK test
also moves the rotation center and checks the rotated endpoints. The native
window capture is `workdir/flexscript/sketch-pattern.png`. The native viewport now previews both spacing vectors, the circular
sweep, and free or referenced center drags. Headless pointer tests cover
Shift direction and angle snapping, single-step undo and redo, cancellation
with pixel-identical restoration, reopening a saved drag, and moving a
referenced fit-spline point. See `workdir/flexscript/pattern-grid-handles.png`
and `workdir/flexscript/pattern-ring-handles.png`.

Biscuit's checked VM edits the same grid and anchored circular pattern,
drives the source radius to 5 mm, and extrudes all six disks. Each extrusion
opens the preceding checked edit's saved document within the standard VM
instruction budget. Two frozen worker builds produced byte-identical STLs
with a combined signed volume of 4,682.17 mm³. Changing just one saved
copy's radius caused the worker to reject the build before returning an STL.

The native editor opens `offset.pcad` with 13 sketch degrees of freedom. The
headless window test changes the chain offset from +5 to −3.5 mm, checks the
miter at (43.5, −3.5), rejects a collapsed corner, and exercises undo and redo.
It changes the concentric offset from 3 to 6 mm and saves. After reopening,
a 10 mm extrusion of the ring has the same bounds and signed volume as
Rust's closed 3MF. A second Rust comparison covers a closed rectangular
chain with two reversed source edges. Its 10 mm extrusion has a volume of
6,000 mm³. Flipping one saved direction makes export fail and preserves the
previous 3MF. Biscuit's checked VM edits the ring drawing. Two frozen
builds produced identical STLs, and a stale miter caused the worker to
reject the build before returning an STL.

The native offset tool creates these relations from source geometry. In a
headless window test, one picked line created the mitered chain and one
picked circle created its concentric copy. After reopening and changing
the source diameter from 20 to 24 mm, the outer radius grew from 16 to
18 mm. The exported ring matched Rust. A separate viewport test created
a slot offset, changed it by picking its outline, and exported a closed
156 × 56 × 10 mm slot-shaped ring accepted by both editors. Headless
comparisons also matched Rust for rotated rectangular and regular hexagonal
rings, with independently calculated volumes. The SDK tests
also exercise circular arcs, rotated and corner rectangles, polygons,
reversed closed chains, forks, and endpoint gaps within Rust's 1 µm join
tolerance. A checked Biscuit program created the chain and circle from
bare geometry. The frozen worker produced the same STL bytes as the
equivalent drawing with saved relations.

The native editor opens `mirror-symmetry.pcad` with 11 sketch degrees of
freedom. Dragging a circle center from (30, 10) to (40, 20) moves its linked
copy to (−40, 20). Editing the copy's diameter from 24 to 28 mm resizes both
circles. The native window regression also mirrors a line, toggles point
symmetry, undoes the removal, and verifies the original relation IDs after
saving. Reopening and extruding each region gives closed solids with the
same bounds and volumes as the Rust reference. See
`workdir/flexscript/sketch-mirror.png`.

Biscuit's checked program edits the opposite circle and extrudes both
regions in the same file. Two frozen worker builds produce identical
STLs. Shifting only one saved center breaks the mirror relation. The native
editor then opens the drawing read-only, preserves its bytes on Save and
produces no STL. The next frozen build also fails.

Mirror now picks a finite circular arc and links its reflected center,
start angle, radius and signed sweep using the existing v4 relation. The
window test changes the mirrored radius to 12 mm and sweep to −110°,
then undoes, redoes and reopens the linked result. The source's reference
dimensions track both edits. The checked VM replays that geometry and
closes two sectors with radial lines. PuppyCAD and Rust agree on the two
extrusions. Two frozen builds produce the same facets as the two native
STLs combined, and a stale start angle fails the frozen build. Whole-curve
mirrors of elliptic arcs and splines still need their geometry constraints.
See `workdir/flexscript/sketch-arc-mirror.png` and
`workdir/cad-migration/linked-arc-actual/verification.json`.

The native UI trial changed a 160 mm sketch width to 161 mm, exercised undo
and redo, and reopened the saved driving relation. The resulting sketch and
solid are captured under `workdir/flexscript/`. The general constraint solver and
remaining solid feature types still need to be ported.

The native extrusion dialog previews the evaluated solid and authors or
edits depth, operation, direction and end condition. The headless test
cancels and applies a 12 mm symmetric extrusion, exercises undo and redo,
then reopens it and flips a blind extrusion. A separate cut uses a picked
earlier body to set the extent. All three reopened 3MFs match Rust in
body bounds and signed volume. Biscuit's checked VM and independent frozen
jobs produced byte-identical STLs from these same saved drawings. A
missing target fails the frozen build without an STL.

The native editor and CLI export 3MF through the shared library. FreeCAD
opened both the Flexscript and Rust exports of a two-body fixture as two
closed components with matching bounds and a combined volume of 1,088,000 mm³.
The window regression also exported the reopened part twice via Ctrl+Alt+E
and verified that the two files had distinct names and identical contents.

The native editor can create and edit v4 revolves through the shared CAD
library. A headless window test drew a profile, rejected an axis crossing it,
picked an oblique axis, saved the new feature, reopened it and exported its
3MF. Another changed an existing sweep from 360 to 95.125 degrees, undid
and redid the change, and checked the saved graph and the reopened export.
The native preview now chooses a sweep and Boolean operation before Apply.
It accepts a sketch line, two picked points or a sketch axis. In the
headless test, changing a rejected full-turn cut to 15° produces an editable
revolve. After reopening, its 123.4421 mm³ solid matches Rust's 3MF.

Native straight-line Trim and Extend use the shared sketch graph editor.
The window test removes the clicked interval between two boundaries,
preserves the surviving endpoint identities through save and undo, and
extends one fragment back to the nearest finite boundary. The CAD tests
cover projected boundaries and reject edits that would invalidate a
dependent solid.

The Trim and Extend tools also use exact circle, ellipse and finite arc
intersections. Rectangles, polygons, capsules and projected lines, circles
and circular arcs can serve as boundaries. The window regression trims a
line at the two intersections of a 20 mm radius circle, saves and reopens
the sketch, and extends one fragment back to the circle. Its intersections
remain at −16 and +16 mm with the line at Y = 12 mm.

Fit and control splines are also boundaries. The SDK intersects finite
cubic spans directly, including tangent contacts and the seam of a periodic
fit spline. The native test saves and reopens the two fragments at ±7.5 mm,
extends one to the next contact, and closes a second profile by trimming to
the spline endpoints. Its reopened extrusion exports as a closed STL.

Circular arcs now extend to finite boundaries along their signed sweep.
The native test extends a 90° arc to 120°, reopens it, and trims it to 60°.
It also trims a circle at a diameter, saves the retained semicircle and
extrudes it. The reopened STL is closed and agrees with the Rust export
within their sampling tolerances. Biscuit's checked VM and two independent
frozen v7 builds reproduce the editor's STL byte for byte.

Elliptic arcs now extend on their original supporting ellipse. The native
test extends a 40 × 20 mm elliptic arc from 90° to 120°, reopens it and
trims it to 60°. It trims the full ellipse at a diameter and extrudes the
retained half. The exported STL agrees with the Rust result within their
sampling tolerances.

Spline Trim keeps the exact retained Bézier portions. Split opens a periodic
fit spline or divides an open spline into joined control chains. The native
test trims the middle out of a cubic, opens a four-span periodic spline and
splits the arch of a closed profile. Undo and Redo restore the intended
control points. The saved endpoint join survives reopening and the resulting
extrusion agrees with Rust in bounds and signed volume.
The rebuilt Biscuit checked VM replays both the ellipse trim and the spline
split. Its STL bytes match the native editor, and two independent frozen
v8 jobs reproduce each saved part. The verification record is
`workdir/cad-migration/ellipse-spline-consumer-proof.json`.

The graph suite compares directed axes, axis-touching profiles, partial and
full turns, rectangular and round holes, mirrored face sketches, joins and
cuts with the Rust exporter. It checks closed topology, body names, bounds
within 0.0001 mm and signed volume within 0.001%. Open CASCADE reopened
the faceted STEP for a partial sweep with two circular holes on a mirrored
sketch plane, validated both solids and matched their bounds and volumes. Biscuit's checked VM
authored a full turn and edited it to 90 degrees. Two frozen native worker
jobs exported identical quarter-ring STLs with the expected bounds and
volume. The complete `rust/fixtures/revolve-loft-finish.pcad` exports three
named bodies in both editors. The chamfered block (7,910 mm³), revolved
ring (28,093.006 mm³) and rectangle-to-circle loft (4,858.787 mm³) have
matching bounds and signed volumes. Open CASCADE validated all three STEP solids.

V4 chamfers and fillets use saved edge endpoints and support normals to find
straight edges after an upstream edit. The headless native test selects two
edges, checks their highlight, edits the distance or radius to 2.125 mm,
undoes, redoes, saves, reopens and exports. The graph suite covers inside
corners that gain material, outside corners, adjacent edges, suppression and
rollback. On one mirrored pair of cube edges Rust silently exports the
original 8,000 mm³ cube. Flexscript rounds both edges and produces the same
7,963.064 mm³ volume as the corresponding corner where Rust succeeds.
Biscuit's checked VM picks and edits a bevel on a plate. Two frozen worker
builds exercise a fillet, revolve and loft together and compare their STL bytes.

Saved TypeScript `chamfer` nodes can also reference a numbered extrusion
edge through an existing v4 `edge` node. The shared evaluator resolves that
selector on a single corner rectangle extruded blind from a reference plane.
It applies both bevel distances before combining the source body with a join
or cut. Six comparisons with the TypeScript exporter cover unequal distances,
the default second distance, vertical-edge aliases and both Boolean operations.
The native window test edits `d1` and `d2`, rejects a negative distance, and
preserves the feature, selector and opaque metadata on save. Reopening and
exporting the edited body matches the TypeScript bevel vertices and 3,960 mm³
volume. The checked VM and two frozen builds reproduce the native STL.

The same blind reference-plane selector now resolves a single rotated
rectangle, regular polygon, circle, ellipse or capsule. The SDK uses the
original TypeScript loop order so a saved edge ordinal still picks its
intended side. All five models reopened after editing both setbacks in
the native editor. Their TypeScript exports match the bevel vertices
within 0.00012 mm, and Biscuit reproduces the native STL bytes. The ordered
selector for a sampled interior boundary covers the single canonical
half-ellipse and chord case described above.
For a closed convex loop of straight segments, TypeScript's edge ordinals
and vertical aliases match native geometry with shuffled and reversed
segments. The edited square's closed 3,960 mm³ STL also matches the checked
VM and two frozen builds. Face attachments are covered for the documented
aligned cases. See `workdir/flexscript/rotated-chamfer.png`.

A saved selector on an eligible convex edge of a concave straight-line
profile uses a bounded solid subtraction. A deep bevel can cross the notch
without filling it. The native edit yields a closed 2,750 mm³ STL. The
checked VM and two v11 frozen jobs match it byte for byte. The test also
checks that ineligible inset and re-entrant edges fail without an STL.

Primitive apertures retain their saved edge numbering for inner rims and
outer bevels. The headless tests edit both rim setbacks, preserve the
selectors and reopen closed 3,484.173 mm³ and 3,437.651 mm³ results. The
second test targets the rotated rectangular hole after a circular hole.
Eight TypeScript exports with both holes agree with the native sampled
surfaces within 0.000015 mm. The checked VM and two frozen builds match
the edited STL. Re-entrant and out-of-range selectors fail without an export.
Saved selectors also cover traced straight-line apertures, including reversed
or shuffled authored segments mixed with a circular aperture. The headless
test reopens the edited 2,169.048 mm³ solid and verifies the rim vertices.
Seven corresponding native and TypeScript exports agree within 0.000010 mm
at sampled surfaces. The checked VM and two frozen jobs match the edited STL.
Sampled curved apertures and ambiguous endpoint welds fail before export.
For face-attached chamfers, supporting extrusions count every inner and
outer side before the two caps. Six closed exports match TypeScript on a
pierced top cap, mirrored bottom cap and an aligned aperture wall. The editor
changes both bevel distances on that wall and reopens the 3,889.031 mm³
solid. The checked VM and two frozen jobs match the native STL. A saved side
whose origin or direction would shift fails before export.

V4 lofts resolve two sketch profile references, match their perimeters and
build a ruled surface. The native picker supports a new body and a join.
The headless window regression creates a loft, undoes and redoes it, saves,
reopens and exports. Raising the base extrusion from 20 to 28.75 mm moved
the face-attached section, scaled the loft volume by 28.75/20 and preserved
the earlier export. The fixture’s original rectangle-to-circle loft measures
4,858.7875 mm³ in Flexscript and 4,858.7871 mm³ in Rust. The graph tests
also cover reversed sections, oblique planes, joins, suppression, rollback
and intersecting surfaces. For an intersecting loft Rust exports its earlier
body. The shared evaluator reports the failing loft and preserves the last
successful export. The native viewport can show the surviving bodies and
the failing feature’s diagnostic. Open CASCADE validated both solids
in the STEP export. Biscuit’s checked VM authored a 312 mm³ square frustum. Two frozen
worker builds produced identical STLs for the frustum and its base.

The headless editor test set a v4 block’s width to 162.125 mm and depth to
60.625 mm. After reopening, Flexscript and Rust produced the same 3MF bounds
and volume. A second test entered −15.125° for the arm’s servo, saved the
solved assembly and compared all three exported bodies with Rust. On Biscuit’s
sensor assembly the editor rejected 91° at the 90° hinge limit, accepted
41.375°, and reopened the document with all six bodies. The original sensor
document and the saved graph differed only in the joint value.
The object browser also selected the second active extrusion by height and
changed it to 52.75 mm. Flexscript and Rust exported the joined solid as
`Body 1` with a volume of 539,562.5 mm³. The shared 3MF writer preserves
authored names before a join and uses Rust’s numbered names for a merged
body list. Biscuit’s checked VM wrote a pocketed model package,
and the host test reopened it and verified the object name.
The width input is captured at `workdir/flexscript/exact-width.png`.

The shared evaluator and native sketch view now use the same sampled profile
loops. The segmented slot fixture and the ellipse with a circular hole exported
as closed solids with identical bounds to Rust and signed volumes within
two parts per million. The native window regression edited the latter from
40 to 41 mm depth, saved and reopened it, checked both outline crossings on
each side of the hole, and compared its STL to Rust. The headless regression
also covers construction entities, rotated profiles, disjoint regions, nested
islands and rejection of intersecting boundaries.

Closed chains of individual sketch lines now form profiles and holes.
Open and branched components can coexist with a closed contour. In a
single new file the window regression drew six lines, saved and reopened
the sketch, extruded the concave bracket and reopened the solid. Both
Flexscript and Rust exported a watertight 50 × 40 × 10 mm bracket with a
volume of 11,000 mm³. The shared evaluator also supports three-point arcs, rotated elliptic arcs,
open and periodic fit splines and cubic control splines. The saved spline
fixture and the tangent-arc fixture (with a closing chord) extruded to the
same bounds as Rust, with volume differences below 0.003 mm³. The native editor saves the original curve records and can reopen and extrude
them. Biscuit's checked program and frozen worker both built a v4 quarter
ellipse closed with a chord. Its mesh measures 10 × 5 × 10 mm and encloses
132.68343 mm³. The legacy per-part volume report sums quantized tetrahedra
and returns 132.607 mm³ for this example. General constraint-driven curve
editing remains open.

Face-attached sketches now resolve saved side and cap IDs against the generating
extrusion, including its hole boundaries. The face-sketch fixture’s pocket and
side boss exported as a closed solid with the same bounds as Rust and a volume
difference below 0.003 mm³. In the native editor the window test changed the
support from 20 to 21 mm, saved, reopened, and deepened the pocket from 8 to
9 mm. A vertical probe of both resulting STLs found the pocket floor at 12 mm.
The headless tests cover mirrored caps, faces of face-attached extrusions,
hole-wall orientation, inactive supports, rollback and extrusion end conditions.
The native editor can now pick a visible extrusion face, draw on its local
plane and save its stable reference. The window regression picked the block’s
top face, drew a 20 × 20 mm rectangle, reopened the sketch, made a 10 mm
inward cut and exercised undo and redo. After reopening, Flexscript and Rust
both exported a closed 956,000 mm³ solid with the pocket floor at Z = 50 mm.

Native **Use edges** and **Pick edge** author straight projected lines in
the Rust v4 relation format. The SDK locks their endpoints and resolves
their sources before the sketch's first consuming feature. The native
test projects a side, cuts the block, and changes the supporting depth
from 60 to 74 mm. The four linked IDs survive undo, redo and reopening.
The rebuilt STL has a volume of 1,110,000 mm³ and matches Rust in bounds
and signed volume. Biscuit's checked VM edits the same saved drawing to
82 mm and matches both frozen builds byte for byte at 1,230,000 mm³.
Separate frozen jobs prove that a stale endpoint cache rebuilds to the
same STL as a refreshed drawing.

Crossing straight lines can divide a projected outline into separate sketch
profiles. The line tool snaps to the projected boundary and the sketch
viewport highlights the selected region. The headless native test picks
both regions of an off-center side cut, saves and reopens each drawing, and
matches Rust's profile order and volumes of 921,000 and 939,000 mm³. The SDK
tests cover a hole contained in one region and confirm that reversing the
first boundary segment reverses the order of the two regions. Biscuit's
checked VM and two frozen builds per region produced the same STL bytes as
the native editor from both saved drawings.

**Use edges** also recognizes circular rims and open arcs on the evaluated
face. SDK tests extrude a projected arc-and-line profile. The native test
projects the circular pocket in Rust's `face-sketch.pcad` and saves it as a
v4 circle. It also projects a separate annular cap, cuts it to 15 mm thick,
and drives the original hole from 20 to 26 mm in diameter. The projection
and extrusion retain their IDs through save and reopen. The final native
and Rust STLs match in bounds and within 0.01 mm³ in volume. The checked VM
and two frozen builds reproduce the native STL byte for byte. A regular
16-sided face retains its straight projected edges. The private display
capture is `workdir/flexscript/projected-circular-rim.png`.

After orbiting the camera, the test also picked the right wall and joined
a 20 × 20 × 10 mm boss. Both exports reached X = 90 mm and placed the boss
between Z = 20 and 40 mm. Biscuit's checked VM picked a cap and verified the
resulting pocket floor. The SDK regression covers hole boundaries, occluding
bosses, missed picks and inward-pocket surfaces without a matching v4 face.
The window captures are `workdir/flexscript/picked-face-sketch.png`,
`workdir/flexscript/picked-face-pocket.png` and
`workdir/flexscript/picked-side-boss.png`.

New project paths now open a blank XY sketch. The editor uses the shared
authoring API to add rectangles, circles and extrusions to the v4 graph.
The native mouse and keyboard regression drew an 80 × 50 mm plate with a
10 mm radius hole, saved and reopened its sketch, then extruded it. Undo
returned to the sketch and redo restored the solid. After editing the depth
to 11 mm, the test saved and reopened the project and compared closed STL
bounds and volume with Rust. First-save collision handling also preserved
a file created externally while a new drawing was open.

A selected v4 assembly now opens directly in the native window. **Next item**
and F6 switch between parts and assemblies in the open file. The arm
fixture exercises a fastened mount, two occurrences of the same arm part,
and a servo-driven revolute mate. The window regression moved the elbow
from 30° to 31°, undid and redid the edit, saved the solved transforms,
reopened the same file and exported three named 3MF objects. Each object's
bounds matched the Rust export within 0.0001 mm and volumes agreed within
ten parts per million. Biscuit's checked VM reproduced the solved instance
bounds. Its frozen worker produced the same binary
facets as the native editor's three per-instance STLs at the original pose.

The shared free-motion solver moves fastened groups together to satisfy
revolute, prismatic, planar and ball mates. On four off-axis reference
drawings, every Rust STL corner of the moving instance was within 0.001 mm
of a Flexscript vertex. Two ball joints on a rigid bar placed both ends and
its fastened companion. Moving one joint beyond the bar's reach produced a
conflict and preserved the previous export. For a slider starting off its
axis, Biscuit's checked VM placed the moving arm at Z = 25–30 mm, and its
frozen worker exported the same facets as the native editor. The window regression solved a
hinge, undid and redid the edit, saved and reopened its transforms, and
exported both instances.

The native editor now creates v4 world fastens and face-to-face mates. The
headless window regression grounds a translated plate rotated 15°, undoes
and redoes the ground mate and verifies the saved local and world connectors.
It picks two plates, cancels the edit, then creates fastened, revolute,
prismatic, planar and ball mates. All five saved drawings produce the same
instance bounds and volumes as Rust within 0.0001 mm and ten parts per
million. The fastened drawing also reopens and exports from the native UI.
See `workdir/flexscript/assembly-mate-choices.png`.

Biscuit's checked VM authors a ground mate and a second fasten between the
side faces of two copies of a 10 × 6 × 4 mm plate. The moving plate ends at
X = 10 mm. Two frozen jobs produce byte-identical 480 mm³ STLs. A dangling
connector fails the build. The shared library also rejects an additional
fasten between incompatible grounded poses and preserves the original graph.

The v4 occurrence inspector lists its mates and their partners. Selecting a
revolute mate can create a servo or edit its angle. The native window
regression drives the arm fixture from 0° to 42°, saves, reopens and detaches
the elbow. Undo restores its driver and both connectors. Redo releases it in
the displayed pose, and Rust reproduces the exported assembly. The regression
also removes all three occurrences, reopens the empty assembly and inserts
the base again. The shared library covers endpoint IDs reused on different
instances and connectors shared by multiple mates.

The native Design connector inspector edits all six frame components.
Changing Base top from Z 10 mm to Z 25 mm in the v4 arm assembly moves
the upper arm and hinged forearm exactly 15 mm without moving the base.
The window test also changes rotation, undoes and redoes frame edits,
saves and reopens the model, then compares its three bodies with Rust.
The checked VM replays the same frame edit. Two frozen builds reproduce
the native STL facets. A conflicting fasten leaves the saved assembly
intact. See `workdir/flexscript/assembly-connector-frame.png`.

The native Design tree can replace an occurrence's part definition in
place. The window test changes the upper arm from Arm to Slim arm, then
undoes, redoes, saves and reopens the model. Only that occurrence's part ID
changes in the authored graph. The stored transform and both mate endpoints
remain intact. Rust agrees on the reopened 3MF. The checked VM and two frozen
jobs reproduce the STL, and the base and forearm exports remain byte-identical.
See `workdir/flexscript/replace-occurrence.png`.

Biscuit's checked VM drives the elbow to 90°, detaches it and removes the
upper-arm occurrence. The frozen worker produces identical STL bytes before
and after detaching the elbow. Two builds after removing the upper arm also
match byte for byte. Their triangle records are exactly the original base
and forearm records in order.

The native occurrence inspector now edits translation, rotation and scale
along each axis through the shared solver. The headless window regression
moves one of two copies by −87.5 mm, rotates it by 90°, rejects a zero
scale and reflects it by a factor of −1.5. It verifies undo, redo, save and
reopen. A separate arm fixture keeps its mounted occurrence at X = 0 after
an attempted move to 8 mm. Moving the base by 12 mm shifts all three
occurrences by exactly 12 mm and preserves the mates and servo.
The mirrored 3MF matches Rust in body names, bounds and absolute volumes.
The reflected Flexscript body has outward triangle winding and a positive
signed volume of 24,000 mm³. The Rust reference writes it with inward
winding. Biscuit's checked VM edits a two-plate assembly using the same
API. Two frozen worker builds reproduce the STL bytes, per-body bounds and
600 mm³ total signed volume.

Assembly mates support visible vertices, straight-edge midpoints, curved-edge
centers and face hits in both the toolbar and the J shortcut. The shared
picker uses each occurrence's evaluated boundary, filters occluded elements
and aligns edge axes in the solved world frame before saving the two local
connectors. The native regression picks a plate corner and a cylinder rim,
undoes that mate, then fastens the plate's straight edge to the rim.
It reopens the saved drawing and compares its 3MF with Rust. The selection
markers are visible in `workdir/flexscript/assembly-rim-mate.png`.

Biscuit's checked VM uses the same picking API to attach a 24-segment pin
to a plate. Two frozen builds of the resulting drawing produce identical
STL bytes and the expected signed volume of the plate and pin together.
The SDK regression also covers rear corners hidden by a larger occurrence,
isolating a rear edge, reversed edge axes on a rotated copy and a semicircular
prism whose arc ends use the adjacent planar cap normal.

The native window regression added a second part and an empty assembly to one
`.pcad` file, saved and reopened it, then placed three instances from its two
parts. Undo and redo preserved their distinct IDs and shared part references.
After reopening, 3MF export matched Rust for all three names, bounds and
volumes (960,000, 4,000 and 4,000 mm³). Biscuit’s checked VM also authored
two instances from a single part and verified their placed bounds. The reopened
native drawing is captured at
`workdir/flexscript/authored-assembly.png`.

The v4 native viewport now highlights individual occurrences, pans,
orbits about a picked surface, rolls at the rim and fits the view. The
window regression dragged the forearm around its solved elbow axis from
30° to 49.8125°. The base stayed put during preview. Escape restored the
previous pixels and file contents. Releasing the drag created one undo
step, and the reopened three-object 3MF matched the Rust export. The
captured result is `workdir/flexscript/dragged-hinge.png`. The v4 gesture and schema-v1 gesture share the same camera and undo handling.

The native editor opens schema-version-1 drawings directly. The standalone
commit's sensor unit and three-wire sensor unit each produce 920 triangles at
45°, 46° and 90°. At all six combinations, the full evaluated JSON, including
board geometry, occurrence transforms and routing reports, matched the original
standalone evaluator byte for byte. The branch's geometry, spatial and assembly
fixtures also passed against the extracted kernel.

The native window test changes the sensor lid from 45° to 46°, undoes and
redoes it, reopens it and compares all six named 3MF bodies. Pinning the lid
and changing the joint to 47° preserves all six bodies' bounds and volumes.
Unfixing removes the occurrence pin. The saved document preserves its original
electrical records. The window also switches projection, picks and drags the
lid in orthographic mode, and restores the original orthographic viewport
pixel for pixel. See
`workdir/flexscript/sensor-unit.png` and `workdir/flexscript/sensor-orthographic.png`.

Biscuit's checked VM and pinned project worker both evaluated a standalone
joint at 90°. The worker's STL places its arm at X = −8…0, Y = 0…50 and
Z = 8…14 mm. The original Biscuit electrical checker retains its supported
board schema. The worker rejects a standalone document with an unsupported
schema version. Browser tests cover deterministic frozen builds and retrieval
of historical artifacts under the previous profile.

Biscuit's native `pcad_input` profile freezes the assembly and its linked
board layouts from one Files revision. The SDK supplies each board's pad,
pin and net map and its mounted occurrence transform. The worker uses the
same frozen input for the assembly, authored drawings, editable KiCad
files and `pcb-assembly.json`. A headless browser built `sensor-unit.pcad`
twice and downloaded a protected ZIP carrying the exact source. The two
builds matched in every deterministic artifact. The STL matches all 920
facets from the six PuppyCAD exports. A new 30° hinge revision produced a
STEP assembly with the same board files, and the earlier ZIP still returned
the original bytes. Removing a via from the sensor board failed the
frozen build and withheld its exports. Revoking Files access also revoked
the reader's access to the archived outputs.

The shared PCB checker, editable KiCad writer and board mesh renderer also
process the original standalone `sensor-unit-3-wire.pcad`. The sensor board
uses a three-pin through-hole header, an axial resistor and an undrilled
oval capacitor footprint. Biscuit's existing electrical imports forward to
the shared code. The legacy LED fixture still produces the same netlist,
check report, KiCad board, CAD mesh and front/back previews byte for byte.

The `pcad.fabrication` check requires a pinned KiCad receipt for every
board exported from the selected frozen PCAD build. Both three-wire boards
passed KiCad 10.0.7 DRC with zero violations and zero unconnected items.
Each produced six Gerbers, a job file and separate plated and non-plated
drill files. The headless browser verified that one receipt blocks
promotion and that two passing receipts unlock the release ZIP. A board
with a deleted route failed DRC, withheld fabrication files and left the
previously promoted release available to authorized readers.

The extended `kicad-pcb-exports-4` descriptor adds the rear silkscreen and
requires all seven plot names in the Gerber job file. The test uses the
original v3 worker to promote a six-plot release, switches the same portal
database to v4, and downloads the earlier ZIP byte for byte. The v4 worker
produces a valid empty rear legend for a front-only board and rear drawing
commands for a board with a back-mounted footprint.

The shared continuity evaluator follows every declared terminal path
through physical copper, mating contacts and harness conductors. The
original two sensor fixtures and eight trace variants match the previous
native evaluator byte for byte. Biscuit's checked VM produces the same
three-wire report through `/biscuit/runtime/pcad-trace.flex`.

Biscuit's `native-project-checks-4` profile includes a numbered product BOM,
the continuity paths and the authored-pose harness report in the frozen
release. The three-wire drawing set adds one schedule for the three cuts
and two pinouts with their viewing directions and mating identities.
Unplugging a connector or invalidating a termination review fails the
private build even though both board checks pass. The headless release
test checks the frozen source digest and the additional continuity and
manufacturing requirements before promoting the two-board package.

Biscuit's `native-project-checks-5` profile runs the original three-wire
fixture's voltage window, lid clearance and three harness motion studies.
The ideal RC experiment reports 4.965463 V at 5 ms after 500 backward-Euler
steps. The same exported circuit runs in ngspice 47 and reaches 4.966312 V.
The report binds the checked experiment to the frozen ground, sample count
and final time. The five poses give a minimum lid clearance of 6.4 mm, and
the three wires clear their checks at all 15 sampled positions.

The headless browser promotes the frozen release with its board receipts,
STEP, combined nine-page PDF, SPICE deck and four CSV tables. Open CASCADE
reopens the STEP as six valid placed solids and verifies the mounting bores
of both boards. A failed voltage window prevents promotion even with
passing board checks and leaves the earlier release downloadable. The
shared circuit, motion and schematic APIs retain the standalone import
paths for checked scripts.

Biscuit's `native-project-checks-6` profile restores the integrated PCAD
assembly check for STL and STEP. Interference, a suppressed mate or a
missing root assembly leaves the earlier promoted release available and
withholds the new exports. The separate CAD input still supports mesh-only
STL for a standalone part. The headless upgrade test downloads the original
v5 ZIP byte for byte and reruns the exact pinned v5 checker with the worker's
explicit v5 operator profile. Checked programs can import
`/biscuit/runtime/pcad-mechanical.flex` again for part, assembly and individual
feature previews.

The saved copper review, zone preview, board checks and product BOM imports
run against the shared SDK in the checked VM. Historical copper reports,
zone runs, BOM pages and procurement CSVs match the original standalone
implementations byte for byte. Biscuit's native build now uses the shared
BOM and wire-stock CSV writers.

The `native-project-checks-7` profile records board diagnostics keyed by
the frozen board ID and includes each routed net's class and effective
clearance in the PCB assembly manifest. A failed board keeps its own
diagnostic alongside the remaining boards. The worker and broker verify
that both copies of the passing board evidence agree. Oversized diagnostics
retain their counts and a bounded sample. The original three-wire bundle
produces identical CSVs, KiCad boards, STEP, PDF and SPICE outputs under v6
and v7. The two JSON reports add the board checks. After the operator upgrade,
archived v5 and v6 releases still download byte for byte and the original
checkers can reproduce their outputs under their recorded profiles.

The `native-project-checks-8` profile restores saved `pcad.step` and
`pcad.drawings` checks. Each binds to the frozen PCAD input. STL leaves the
STEP criterion unknown. A PDF containing only schematics or wire schedules
leaves the mechanical drawing criterion unknown. The authenticated release
test verifies both checks against the original sensor fixture and confirms
that archived v7 packages and journals retain their original bytes.

The `native-project-checks-9` profile restores the standalone branch's
`pcad.input` criterion. It binds the successful schema, units, root assembly,
size, release options and product hierarchy checks to the frozen PCAD source.
`pcad.assembly` evaluates the solved assembly. Authenticated tests verify
both criteria, re-execute the original v8 checker and preserve the archived
v8 ZIP and journal byte for byte. The original three-wire sensor release
also passes the headless browser and real KiCad 10.0.7 fabrication tests.

The `native-project-checks-10` profile validates connector revision bindings
even when a board release has no harness. The authenticated release tests
accept renumbered pads with the reviewed binding and reject a mismatched
revision before publishing manufacturing outputs. They also download and
re-execute the original v9 release under its exact profile and verify its
archived ZIP and journal byte for byte.

The `native-project-checks-11` profile restores the original advisory
`continuity.json` for a declared harness without required paths. The
report is `unknown` and preserves the original frozen-source digest.
Release jobs with optional continuity can complete, while a requirement
for verified continuity still blocks promotion. Populated and empty
harness cases match the historical report bytes. An eight-board fixture
still fits the 24-file limit, and a board-only job retains its v10 outputs.
The authenticated upgrade preserves the original v10 ZIP, journal and
toolchain descriptor, including exact replay under the v10 worker profile.

The original authenticated Files CAD routes also use the checked VM and
shared CAD SDK. They cover revision-bound previews and checks, copper and
continuity review, BOM and drawings, sweeps and circuit results. The original
standalone database test reopens a persisted report with its original run ID,
metadata and download bytes. The Files API retains the original revision
cursor and checks access to a saved revision or an unsaved draft before
serving results.

The native standalone editor authors and edits Shell features on convex
bodies. The headless window test rejects a 32 mm wall, previews and applies a
2 mm wall on the +Z face, and exercises undo and redo. It widens the source
box, edits the existing feature to a 1.5 mm wall with a −X opening, and saves
and reopens the same part and feature IDs. The exported 30 × 16 × 9 mm shell
is closed and has a signed volume of 2,097 mm³. The preview capture is
`workdir/flexscript/shell-part-preview.png`. Biscuit's checked VM and two
frozen worker runs reproduced the STL bytes for both this shell and the
native extrusion closed by trimming to the spline. The verification record
is `workdir/cad-migration/spline-shell-consumers/verification.json`.

The native object tree browses bodies, nested occurrences and driving fields.
The window regression searched for `right/upper` across two copies of one
subassembly, selected the occluded occurrence, pinned it, and verified the
exact path in the saved document. A field selected by name changed the
source plate from 30 to 31 mm. The overlap fixture also double-clicked the
lower plate, picked it through its hidden neighbor, exported both parts and
restored the upper hit with **Show all**. The checked VM verified the same
filtered ray behavior. Headless tests verified face-attached sketch picking
on the isolated body.

The native orientation cube follows the rendered view and selects all six
standard views plus isometric. At a window width of 420 pixels the regression
changed a driving field and exported the resulting two-body 3MF through the
paged toolbar. Captures are under `workdir/flexscript/` as
`nested-occurrence.png`, `view-cube.png` and `compact-objects.png`.

The native viewport draws authored part colors and the evaluator's wire routes,
shared jackets and positioned splices. The sensor fixture shows both green
boards inside the grey enclosure with an orange wire, blue jacket and purple
splice. The window regression checks the colors, isolates a part, restores
the harness and drags a lid with a mounted splice. It checks that the splice
moves during preview and returns to its original pixels on Escape. See
`workdir/flexscript/sensor-harness.png` and `workdir/flexscript/hinge-harness-preview.png`.

The shared faceted STEP writer exports v4 parts, v4 assemblies and standalone
drawings. Biscuit’s STEP importer reopened the sensor assembly with six
occurrences and five part definitions, including one shared by both mounting
supports. Its nested fixture preserved two levels of repeated subassemblies.
Open CASCADE independently checked solid validity, hole clearance, volume,
dimensions and placement. A v4 reflected, scaled arm reopened with bounds and
volume matching the editor's 3MF. Both the toolbar and Ctrl+Shift+E produced
the same file after reopening a sensor drawing.

The Biscuit native build profile honors `mechanical_export: "step"` on
standalone assemblies. It records the frozen source SHA-256 and verifies
clearance and unsuppressed mates before releasing `assembly.step`. Two jobs
for one frozen version produced identical STEP bytes. The checked VM also
exported a two-occurrence assembly with a single part definition.
For both the flat and nested sensor fixtures, the frozen checker's entire
STEP file matched the original writer at `ea4670d` byte for byte. The
protected release ZIP contained the same bytes as the direct download.

The original standalone Chromium suite at `ea4670d` passed against its
own server. It also covers choosing which occurrence to isolate from a
repeated *part definition*. The native tree supports both direct occurrence selection and a chooser
for each part definition. The native camera regression tracks an off-center marker on a solid.
Its centroid stayed within one pixel of the starting position during a
right-drag in both orthographic and perspective views. After pan and zoom,
selecting the same standard view restored the fitted geometry pixel for
pixel in both projections. A stationary right-click opens Fix or Unfix on
the picked occurrence. The window regression pinned the sensor lid, changed
its joint and checked the exported bodies, then removed the pin and verified
the saved file. The headless window test also selected the sensor mounting
support’s second copy (`post-b`), isolated it, and used the keyboard menu
to fit, pin and unpin it. The v4 arm test isolated the forearm from the
shared Arm part definition. See `workdir/flexscript/occurrence-menu.png`
and `workdir/flexscript/part-occurrence-chooser.png`.

The native **History** panel exposes the v4 feature order, suppression,
deletion, rollback and evaluation errors. The shared evaluator keeps independent
bodies visible when a feature fails. Manufacturing still requires every
active feature to succeed. The headless editor test opens Rust’s
`feature-tree.pcad` and unsuppresses the cut. Moving the cut past the join
changes the 3MF volume from 527,500 to 511,500 mm³, matching Rust. The test
also rejects moving the cut across its target, saves and reopens a
rollback to zero, then checks a partial rollback and a saved suppression.
Another test opens a drawing with a zero-depth cut, checks its diagnostic,
suppresses the cut, saves, reopens and compares the repaired 3MF with Rust.
The checked VM exercises the same edits and keeps an independent ring in
the preview when the mounting plate is suppressed and its dependants fail.
Two frozen builds produced identical STLs for the rolled-back graph.
A build with broken dependencies failed without producing an STL.

The editor deletes independent solid features and keeps their source
sketches. The headless test uses the Delete feature button, undoes and
redoes the edit, saves, and reopens the sketch. A separate deletion
preserves the earlier block and refuses to remove its supporting feature
while a suppressed cut still depends on it. After reopening, the native
STL of the remaining block matches Rust. Biscuit's checked VM and two
frozen builds produce the identical 500,000 mm³ STL from the saved file.
The SDK tests check rollback positions, face attachments, projected edges
and up-to-body references.

The shared source inspector checks semantic records across v4 parts and
assemblies, including suppressed features and inactive items. It also
recognizes unsupported standalone node and mate types, sketch constraints,
schema versions and units. Unrecognized JSON metadata round-trips with an
editable drawing. The native read-only viewer can show the last supported
geometry and the original source. The headless window regression exercises
reopen, keyboard and mouse edit attempts, and source navigation. The
round-trip command preserves the original bytes of a read-only input.
The native editor now edits the Rust `graph-constrained-line.pcad` fixture
through `cad/project-lines.flex`. Setting its length to 125.125 mm holds the
start at (−50, 20) and moves the end to (75.125, 20), matching Rust.
Undo, redo, save and reopen preserve its horizontal and length constraint
IDs and its two degrees of freedom. A horizontal-only line can be dragged
along its axis with the opposite endpoint held. A length-only line follows
a circle around that endpoint. Dragging the coincident corner of two free
segments from either line moves both endpoints to the same target, as in Rust.

The SDK's `cad/tests/fixtures/constrained-plate.pcad` has four coincident
corners and includes both inline and graph constraints. Resizing the bottom
line from 20 to 32.125 mm changes both ends of the right edge and rebuilds
the 5 mm extrusion. The solved outline agrees with Rust and retains zero
degrees of freedom. The native window regression edits the plate, saves it,
reopens it and exports a 3MF with the same body bounds and volume as Rust.
The resulting solid measures 1,606.25 mm³ in PuppyCAD and Biscuit's checked VM.
Two frozen Biscuit jobs exported identical STLs of the resized drawing.
The F2 dimension editor and F3 variable editor share the v4 expression
evaluator with Biscuit. The headless window regression saves a plate width of
`#w + 6.725 mm` with `w = 1 in`, reopens the formula, edits it, undoes the
edit and changes `w` to `2 cm`. Rust reopens the same formula and independently
recomputes the 26.725 mm outline with zero degrees of freedom. The checked VM
rebuilds the 1,336.25 mm³ solid from the same edits. Two frozen builds produce
identical STLs. A third job with an edited variable and stale saved width
fails without emitting an STL. Native export also preserves its previous
file when a saved endpoint disagrees with the formula.

Both editors solve `rust/fixtures/dimensions.pcad` with three degrees of
freedom. Changing its variable `w` from 1 in to 2 cm moves the line end to
(40, 0) and the point to (−0.8, 24.6) mm. From the original file, setting
the signed X dimension to −32 mm moves the point to (18.8, 30) mm.
The private-display test edits that dimension, toggles it to reference and
back, saves, reopens, and picks two real anchors to author a reference
dimension. The saved graph retains the selected anchor IDs.

On a closed plate with a construction datum, Biscuit's checked VM applies
`#w + 6.725 mm` to an X distance, solves the 26.725 mm width and updates
the measured diagonal. Two frozen jobs reproduce the 1,336.25 mm³ plate as
byte-identical STLs. A third job rejects an inconsistent saved datum before
writing an STL.

The native editor also places ordinary and construction points in a blank
v4 drawing. The window regression undoes and redoes placement, saves,
reopens, cancels and accepts a point drag, then authors a 22 mm X dimension
between the saved points. In the Rust dimensions fixture, dragging toward
(20, 40) mm lands at (20, 30) mm and translates both line ends 10 mm along X
in both editors. A separate fixed-origin fixture projects a drag onto a
50.8 mm radius. Biscuit’s checked VM places and drags two dimensioned points
on a closed plate. The frozen worker exports exactly the same 1,000 mm³ STL
with and without the points. Rust and native PuppyCAD both reopened the
checked drawing and exported a single 20 × 10 × 5 mm body in 3MF.

The F8 editor solves circle diameters, radii and circle-center distances in
sketches containing lines, circles and points. A free circle starts with
three degrees of freedom. Driving its size leaves two. The native regression
opens the circular pocket in the v4 face-sketch fixture, drives its diameter
with `#bore + 6 mm`, and adds a reference radius. Changing the variable
from 1 cm to 2 cm updates the diameter from 16 to 26 mm and the reference
radius from 8 to 13 mm. Undo, redo, save and reopen preserve the formula
and relation IDs. The reopened 3MF matches Rust in bounds and signed volume.
See `workdir/flexscript/sketch-circle-diameter.png`.

Biscuit's checked VM edits the same drawing. Two frozen builds reproduce
identical STL bytes and the volume of the block and boss minus the
32-sided pocket. A stale saved radius fails the next build before it can
produce an STL. The SDK tests include a variable rename, a signed distance
to the circle center and conflicting drivers. Changing the radius or dragging
the center across the outer loop of an extruded ring leaves the original
solid intact and reports the failed edit.

The native dimensions tool also edits circular-arc radius and signed sweep,
and elliptic-arc sweep. A headless window test reverses a semicircular
extrusion with `-2 * #turn`, undoes and redoes the edit, and saves and
reopens the file. The exported 3MF matches the Rust part in bounds and
signed volume. The SDK tests cover degree and radian inputs, variable
renaming, reference conversion and rejection of edits that open the
extrusion. See `workdir/flexscript/sketch-arc-sweep.png`.

Biscuit’s checked program changes its quarter-ellipse fixture to the
complementary −270° arc. Two frozen worker builds reproduce the same STL
and its independently calculated volume. Replacing the saved sweep with
+90° keeps the endpoints coincident with the chord but disagrees with the
formula. The worker then rejects the drawing. Constraints on an arc’s endpoints
remain open.

The F8 dimension editor drives full width, full height and rotation on
ellipses and elliptic arcs. A native edit changes the washer’s first axis
using `2 * #span`, sets its height to 24 mm, and rotates it to −30°. The
test undoes and redoes the rotation and toggles it between driving and
reference. The reopened 60 × 24 mm washer matches the Rust 3MF. Replaying
the edits in Biscuit produces the same CAD graph. Two frozen builds verify
STL byte identity, volume and the 128 oriented outer rim vertices. Changing
the saved width to 61 mm without updating its driver makes the worker reject
the export. See `workdir/flexscript/ellipse-rotation-dimension.png`.

The native split tool preserves the original entity ID and links each new
elliptic arc to the original with a `sameEllipse` relation. The headless
window test splits a 40 × 20 mm ellipse into three pieces, undoes and redoes
the second cut, then reopens the file and drives all three widths to 50 mm.
Its 10 mm extrusion matches the Rust 3MF. Biscuit’s checked program produces
the same CAD graph. Two frozen Biscuit builds export byte-identical STLs
with the expected volume. Moving only one saved arc’s center makes the
worker reject the export. The library tests also cover clockwise arcs and
the explicit coincident endpoint relations saved by the TypeScript editor.

The native sketch editor converts mixed selections to construction geometry
and restores all-construction selections to profile geometry in one undo
step. Both root and nested flags in saved v4 records are supported. The
window regression converts a circle and line, checks undo, redo, cancel
and reopening, then extrudes the surrounding rectangle. Its 8,000 mm³ 3MF
matches the Rust export. Biscuit’s checked program reproduces the native
graph and two frozen builds produce byte-identical STLs. Converting the
last remaining profile edge loop makes the worker reject the extrusion.
See `workdir/flexscript/construction-geometry.png`.

The headless library reads faceted STEP bodies from Part 21 files and
validates their closed shells. All five definitions in the native sensor
STEP export match the standalone branch’s `assets/pcad-step-format.js`
reader vertex for vertex, with identical triangle indices (96, 24, 376,
376 and 24 triangles). A second fixture exercises centimetres, reversed
bounds, concave planar faces, sparse IDs and an escaped body name. Native
PuppyCAD and Biscuit's checked VM both reproduce the reference PCAD graph.
Two frozen builds export byte-identical STLs with a 1,500 mm³ volume. A
malformed shell fails without producing an STL. Open CASCADE reopens the
bracket's STEP re-export as one valid 20 × 20 × 5 mm solid with the same volume.

PuppyCAD previews a selected STEP body before adding or replacing it. The
private-window regression opens the sensor STEP, imports the lid, undoes and
redoes the import, saves and reopens the drawing, then replaces the lid
geometry with the base and exports it. The part reference survives
replacement. See `workdir/flexscript/step-body-preview.png`.

The shared STEP reader also imports whole faceted assemblies. The flat and
nested sensor drawings match the standalone reader's PCAD graphs. Both
retain five part definitions and six placed solids. The nested version
retains both reusable subassembly definitions and their local +90° and −90°
frames. The bounds of every re-exported solid match the original within
0.001 mm and volumes within 0.01 mm³. Imported placements are fixed mates
at the saved pose. The STEP file contains no editable hinge limits.

The native window test previews the whole sensor assembly, imports it, undoes
and redoes the change, saves, reopens and exports six closed STLs. Its
private display capture is `workdir/flexscript/step-assembly-preview.png`.
Biscuit's checked VM matches the reference PCAD for two occurrences of one
nested L bracket. Two frozen builds produce byte-identical 3,000 mm³ STLs.
Open CASCADE reads the native STEP re-export as two valid 1,500 mm³ solids.

The native Design sidebar opens for blank sketches and evaluated solids.
The headless window test selects a rectangle in the sketch tree, opens the
extrusion’s history, and finds a saved drawing sheet in the sensor assembly.
The same test opens the second of two parts with identical sketch IDs and
verifies that editing its outline changes only that part. The separate
Objects panel still supports occurrence picking and isolation.

The v4 editor shows distinct Assembly, Part and Sketch workspaces in one
window. The headless test clicks Base in the Design tree from the Arm
assembly and expands Base to open its sketch with one click. With the
sidebar collapsed, the breadcrumb returns to Base or directly to the Arm
assembly. The Part link also closes the Design context menu and returns to
Base in one click. Both ancestor links remain available while typing a sketch
dimension. The headless test follows each link, then checks that saving leaves
the sketch unchanged. Undo and Redo preserve the active sketch during an edit.
The saved file retains the other items and the assembly references. A blank
sketch can also return to Part before it contributes a solid.

Part mode draws visible sketches on their actual reference planes and
attachment faces through the shared CAD path evaluator. The Design tree
eye and context menu toggle a sketch independently of its geometry. A
headless window test shows a rectangle before extrusion, switches to a
standard view, saves and reopens a hidden sketch, then reveals a sketch
used by an extrusion. The SDK cases also check that a face sketch follows
its supporting extrusion when its depth changes.

The Part workspace creates sketches on Top, Front or Right, or starts a
face sketch. The SDK reuses existing plane IDs and adds missing planes to
legacy v4 parts. The native test draws on Front, saves and reopens the
sketch, and extrudes it along +Y. Its 20 × 10 × 10 mm envelope and
2,000 mm³ volume match Rust. Biscuit’s checked VM produces the same CAD
graph and two frozen builds export byte-identical STLs.

The shared drawing renderer exports the standalone sensor fixture’s four A4
sheets, including the projected dimensions, section, enlarged hinge detail
and exploded parts list. The native window opens each sheet from the
Design tree and draws the shared scene with its dimension values, section,
detail markers and BOM balloons. The headless test switches among all
four previews, exports the complete PDF, and returns to the model with
the original file bytes preserved. Biscuit’s checked VM renders the same
four sheets through the standalone import path. The plan resolves 102.00 mm
and 48.00 mm ± 0.150 mm, and the exploded view resolves all six BOM balloons. The frozen worker
produces a PDF alongside STEP or STL and discards exports for duplicate
sheet IDs. Two independent build jobs downloaded byte-identical PDFs with
the frozen source digest in the footer. Earlier build artifacts remain
retrievable under their recorded profiles.

PuppyCAD creates projection sheets and edits the view settings through the
shared library. The native test creates an empty top view, names it
Inspection, changes it to a front section at 1:2 and −6 mm, and exercises
undo, redo, save, reopen, and PDF export. The CAD SDK rejects invalid scales
and out-of-model section planes. Biscuit’s checked VM authors the same
document. Two frozen builds produce byte-identical PDFs that include the
frozen source digest and preserve the 1,728 mm³ block in STL.

The native sketch editor authors diameter circles and all three
center-based or rotated rectangle placements. The headless test draws
a 50 × 20 mm centered rectangle at 53.130102°, creates a diameter-defined
hole, and drives the width to 60 mm. After reopening and extrusion its
3MF matches Rust in bounds and signed volume. Biscuit’s checked program
produces the same CAD graph. Two frozen builds export byte-identical
STLs. A saved width that disagrees with its driver fails the build.
See `workdir/flexscript/centered-rectangle-preview.png`.

The native editor creates a regular polygon and a three-point slot with
the v4 primitive records. The headless test changes six sides to eight,
drives the polygon radius to 24 mm, and reopens the drawing to widen the
slot from 6 mm to 8 mm without moving its cap centers. The resulting
extrusion matches Rust’s 3MF. Biscuit’s checked program produces the same
CAD graph, two frozen builds produce byte-identical STLs, and a saved
polygon radius inconsistent with its driver fails the build.
See `workdir/flexscript/polygon-slot-preview.png`.

Remaining standalone work includes additional sketch and feature authoring
and design inspectors, along with mixed-constraint solving and the other
open cases in the parity inventory.

## Later milestone: WASM editor in Biscuit

Once the native workflow is established, add the browser rendering, window/input and file adapters required to run the same editor in WASM. Biscuit will host it and supply authorized workspace file access. Verify opening, editing, saving and reopening the same fixture in Biscuit against the native result. Headless adoption in milestone 5 can ship independently.
