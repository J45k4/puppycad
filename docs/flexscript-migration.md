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
code through `cad/api.flex`. Biscuit still owns the preview, RGB snapshot
adapter, electrical, component and gripper integrations.

The two-hole mounting plate builds at 80 × 50 × 6 mm with 932 triangles.
The pre-extraction Biscuit fixture, the standalone SDK, the checked VM, and
two invocations of Biscuit's project check tool produced identical 46,684-byte
STLs (SHA-256 `f6b97e5c6580b70614e0122daf9cc2f78d6b46d1b1da579ac70d294bcc506adc`).
The SDK's `libs/cad/tests/geometry.flex` exercises its public API. Biscuit's
`tests/shared_cad.py` compares the VM mesh and STL against native output,
alongside the existing geometry, spatial and assembly suites.

Biscuit's build scripts and CI read the same SDK commit from
`flexscript-sdk.rev`. The local SDK branch `puppycad-cad-sdk` at
`9a630069e57bf094534a4c9a1debe8538d4960fb` includes the shared library and
the FlexDB blob APIs required by the backend. The backend, project checker
and checked VM tests passed at this revision. The frozen-worker and headless browser suites also passed after rebuilding from the recorded pin. Hosted CI verification
requires publishing the SDK commit.

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
middle point out of tangency makes the worker reject the export. Mixed driving constraints and edits requiring
deformation of a closed smooth chain remain open.

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
produces no STL. The next frozen build also fails. Linked mirrors of
points, straight lines and circles, and
reflected anchor pairs are supported. Other whole-curve mirror types,
including finite arcs and splines, still need their geometry constraints.

The native UI trial changed a 160 mm sketch width to 161 mm, exercised undo
and redo, and reopened the saved driving relation. The resulting sketch and
solid are captured under `workdir/flexscript/`. The general constraint solver and
remaining solid feature types still need to be ported.

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
editing and projected arrangements remain open.

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
rollback and evaluation errors. The shared evaluator keeps independent
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
formula. The worker then rejects the drawing. Sketch dimensions for ellipse
axes and rotation and solver support for arc-end constraints remain open.

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

Remaining standalone UI work also includes additional sketch and feature authoring and
inspectors for additional design records. Include its `docs/pcad-electrical.md` workflows
for drawings, board exchange and release artifacts when
checking product parity.

## Later milestone: WASM editor in Biscuit

Once the native workflow is established, add the browser rendering, window/input and file adapters required to run the same editor in WASM. Biscuit will host it and supply authorized workspace file access. Verify opening, editing, saving and reopening the same fixture in Biscuit against the native result. Headless adoption in milestone 5 can ship independently.
