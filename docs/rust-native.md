# Native Rust app

The Rust rewrite lives in [`rust/`](../rust/) and builds a native window with
`winit` and a custom `wgpu` renderer. The package and executable are named
`puppycad`.

```sh
cd rust
cargo run
```

The current implementation establishes the project tree, sketch viewport, and
properties layout. The header shows a clickable breadcrumb trail:
Projects / project (file name) / part / sketch, then the selected shape; in the
solid preview the trail ends at the selected extrusion instead of the sketch.
Click the project to clear the selection, the part to open its solid preview (or
clear the selection when it has no extrusions), or the sketch to return to sketch
view. The last crumb is the current item and is not a link.

Click **Projects** to open the projects page. It starts in the open project's
folder (or the working directory when no project is open) and lists the parent
folder (`..`), subfolders, and `.pcad` files with their part count and revision.
Click a row or use Up/Down and Enter to open it; Backspace goes to the parent
folder, PgUp/PgDn and the wheel scroll long lists, and Esc goes back to the
current project. If the open project has unsaved changes, the first open only
warns: press Ctrl+S to save, or open the same project again to discard the
changes. Web projects saved under `workdir/saved-projects/` are not listed, since
most of them use features the native app cannot open yet.

Select, Rectangle, Circle, Line, rotated Ellipse, and three-point Arc are available through
the toolbar or V/R/C/L/E/A keys. Click twice to draw a line, rectangle, or circle.
Choose **MORE → FIT SPLINE** or **MORE → CTRL SPLINE** to place spline points.
Press Enter to finish an open curve or Shift+Enter to close a fit spline;
control splines require 3n + 1 points. Esc cancels placement. Existing web
`sketchSpline` nodes also load in both modes, including periodic closed fit
splines. The viewport draws the same cubic curves, allows whole-spline
selection and transforms, and preserves points and closure through
save/reopen. Select a spline, then drag one of its visible fit points or control
handles to reshape it; Ctrl+Z restores the previous curve. Click a handle,
then its X or Y value in Properties to enter an exact millimeter coordinate.
Each coordinate edit is undoable and persists through save/reopen. Selected control
splines show a dashed handle polygon. Closed fit splines contribute an extrusion
profile. Hover over a selected spline and press **I** to split an open curve
into two editable control splines, or open a closed fit curve at that point.
The split uses exact cubic subdivision, is undoable, and survives save/reopen.
Select a spline handle and press **Shift+K** to fix its current position;
press it again to release the handle. A fixed handle rejects coordinate edits,
drags, transforms, split, and trim until released. The fixed relation survives
save/reopen. Other spline constraints remain to be ported.
Select a standalone sketch point and click X or Y in Properties to enter its
exact position. Linked line endpoints follow the point; a driving line length
prevents edits that would break its constraint. **Shift+K** fixes or releases
the selected point; fixed points cannot be moved until released.
With a line selected, hover over either endpoint and press **Shift+K** to fix
or release that endpoint. The fixed endpoint survives save/reopen and undo.
Moving the line or changing a dimension that would move the fixed end is blocked;
Split and Trim preserve fixed endpoints on retained pieces.
Shift+K also fixes or releases the center of a selected circle, arc, ellipse,
elliptic arc, polygon, or rotated rectangle. A fixed center blocks transforms
while dimensions such as circle diameter remain editable around that center.
For a selected circular or elliptic arc, hover over an endpoint and press
Shift+K to fix that endpoint instead. Split and Trim preserve fixed original
endpoints on retained pieces; Extend refuses to move a fixed endpoint.
The same hover gesture fixes ellipse axis points, rotated rectangle corners,
polygon vertices, and slot end-cap centers. Dimension or side-count edits that
would move a fixed anchor are rejected; independent dimensions remain editable.
For a corner rectangle, hover over either opposite corner and press Shift+K.
Width and height edits keep a fixed corner in place by moving the other corner.
Use **Shift+R** or **MORE → CENTER RECT** to click a rectangle center and corner;
it saves as an editable rectangle and can be extruded like a corner rectangle.
Use **T** or **MORE → 3PT RECT** to set a rotated rectangle's first edge and
perpendicular height. **Shift+T** or **MORE → 3PT CTR RECT** starts from its
center, side midpoint, and adjacent side. Both save as editable rotated rectangles.
For an ellipse, click its center, first axis endpoint, and a point that sets its height;
for an arc, click its start, end, and a point on the desired curve. Use
**Shift+C** or **MORE → 3PT CIRCLE** to create a circle through three points on
its rim. Use **Shift+A** or **MORE → CENTER ARC** to click the center, radial
start, and end direction. Press **W** while that tool is active to toggle
clockwise sweep. Both arc tools save as editable web arc nodes. The circle saves
as the editable web circle primitive. Click a shape or its
tree row to inspect it, drag a selected shape with the left mouse button to
move it, set signed Move X and Move Y values in Properties and click **Move
selected** or **Copy selected** (Ctrl+D also copies using those values), press
Delete to remove it, Ctrl+Z to undo, drag with the middle mouse button to
pan, and use the wheel to zoom around the pointer.
Press **F** in sketch view to fit all sketch geometry inside the viewport.
Sketch placement snaps to existing anchors, midpoints, and finite line-line,
line-circle, line-arc, line-ellipse, circle-circle, circle-arc, arc-arc,
circle-ellipse, and ellipse-ellipse intersections, plus
crossings on rectangle, polygon, and slot boundaries, within 8 screen pixels, then to the origin or a 1 mm
grid. A square marks the previewed snap target.
Press **N** to toggle geometry snapping and **Shift+G** to toggle grid snapping.
Snapping a new line or open spline endpoint onto a sketch point creates a persistent coincidence
relation. Moving that point moves the linked endpoint, moving, scaling, or
rotating a linked line carries the point with it, and length or axis edits on a
linked line end move the point too. The relation survives save, reopen, and
undo. Intersection snaps currently place a coordinate; other
inferred relations still need to be ported.
Finite line and spline crossings, including tangencies, are available as
intersection snap points. A line segment must actually reach the spline.
With a line, circular arc, or elliptic arc selected, hover over an interior point and press
**I** to split it into two editable pieces. The edit is undoable and survives
save/reopen. Each line
piece keeps construction and horizontal/vertical state, and an endpoint point
relation follows its original endpoint. A driving length is removed because the
original line length has changed. For a selected circle or ellipse, hover over two distinct
points on its rim and press **I** at each point; this creates two editable arcs
of the corresponding curve type. Elliptic arcs keep their center, axes, rotation,
and exact sweep through save/reopen. Splitting an ellipse or elliptic arc adds
the web `sameEllipse` relation. The resulting pieces share center, axes, and
rotation when edited; their Project rows show `[SE]`. Press Esc to cancel a
pending first point.
With a line, circular arc, circle, ellipse, or elliptic arc selected, hover over the section between
crossings and press **Q** to trim that section. One crossing keeps the opposite
side of an open curve; no crossing removes the curve. A trimmed circle becomes
an editable arc, and a trimmed ellipse becomes an editable elliptic arc. The
operation is undoable.
For a selected spline, **Q** trims the clicked span between crossing lines,
circles, circular arcs, ellipses, elliptic arcs, or other splines.
The retained spans become editable control splines and survive save/reopen.
Splitting or trimming a spline preserves coincidence links at retained original endpoints.
With a line, circular arc, or elliptic arc selected, hover near the endpoint to extend and
press **U**. The curve reaches the nearest boundary in that direction. For a
line, existing length and endpoint coincidence relations on the changed
endpoint are removed. The edit is undoable and survives save/reopen.
Set **CX** and **CY** to the transform center in millimeters. The default is the sketch origin. Set a positive scale factor in Properties and click **Scale selected** or **Scale copy** to resize about that center. Existing driving dimension values scale with the selected shape.
The **Rotate** tab uses a signed angle in degrees and offers **Rotate selected** and **Rotate copy** about the same editable center. Rotating an axis-aligned rectangle converts it to a rotated rectangle primitive that the web app can open.
Move, scale, and rotate copies retain their editable driving dimensions.
Click line length, rectangle or ellipse width or height, or circle diameter in Properties
to enter a new millimeter value. Press Enter to apply it or Escape to cancel;
Ctrl+Y or Ctrl+Shift+Z redoes an undone edit. These edits are stored as native
driving dimension relations and remain editable after reopening the file.
Arc radius and regular polygon radius are editable in Properties and save as
web `radius` relations. An arc with fixed endpoints rejects a radius change;
a polygon with a fixed vertex does the same. An arc that sources an Equal
radius pair updates its dependent circle or arc in the same undo step.
Click **SWEEP** on a circular or elliptic arc to enter signed degrees between
-360 and 360, excluding zero. Click **ANGLE** on an ellipse, elliptic arc,
regular polygon, or rotated rectangle to set its rotation, including negative
angles. These values save as web `arcSweep` and `rotation` relations. Scaling
preserves angular values; rotating a shape updates its driving rotation value.
Existing line rotation relations also load and can drive Parallel or
Perpendicular targets.
### Constraint solver

`rust/src/solver.rs` is a least-squares solver ported from the structure of
`src/sketch-solver.ts`. Every anchor coordinate (line endpoints, sketch points,
circle and arc centers) and every circle or arc radius is a parameter; each
relation contributes a residual. Levenberg–Marquardt with numeric Jacobians
first solves the relations together with soft "stay close" weights and then
solves the relations alone, damped by those weights. A solve reports its
residual norm and whether it converged.

Moves, drags, dimension edits, scale, rotate, and creating a relation all apply
the edit, solve, and then either commit everything as one undo step or roll the
edit back. The edited shape (or, for a length or rotation edit, the line's first
endpoint) is held exactly. The remaining intent is a cascade: relation sources
stay put and only targets move; if that fails, sources may move but keep their
first endpoint; if that fails too, only the edited geometry is held. Targets
rotate or stretch about their first endpoint, and direction targets keep their
length when nothing else sets it. A solution that collapses a line is rejected.

Solver-backed relations: horizontal and vertical, line length and line rotation
dimensions, Equal lines, Parallel, Perpendicular, Collinear, signed Angle,
endpoint coincidence between a line and a sketch point, fixed line endpoints,
points and circle/arc centers, Midpoint and point-on-curve on a line,
point-on-curve on a circle, circle and arc radius and diameter dimensions,
Equal radius, radial distance, and Concentric between circles and arcs.

Still hand-propagated: normal and endpoint tangency, internal and external
tangency, Midpoint and point-on-curve on arcs, ellipses, and splines, point
links to spline ends, shared and Equal ellipses, concentric pairs that include
an ellipse, and fixed arc endpoints. Shapes in these relations are constants in
the solve, so an edit that would need them to move is rejected. Rotating a
direction target and rotating axis-constrained lines by other than quarter turns
stay blocked.

**Anchor dragging.** A selected shape shows small squares on its draggable
anchors: line endpoints, sketch points, circle and arc centers, all four
rectangle corners, rotated-rectangle corners, ellipse axis points, polygon
vertices, and slot cap centers (spline handles keep their own circles). The
hovered or dragged anchor is filled. Left-dragging an anchor (same 7 px pick
radius as spline handles) moves only that anchor; dragging elsewhere on the
shape still moves the whole shape. For lines, points, circles, and arcs the
anchor becomes a strong soft target that is re-solved every frame with every
hard relation, so it slides along what they allow: a line end with a driving
length moves on a circle around the other end, and coincident points follow.
A dragged line endpoint keeps the other endpoint in place. For the other
shapes the anchor edit keeps driving width, height, radius, and rotation
dimensions (the corner or vertex slides along them), then re-solves like a
whole-shape drag. Release commits one undo step, Esc cancels. A fixed anchor
(or any fixed anchor on a non-line shape) refuses to drag, and shapes in the
still hand-propagated relations above fall back to whole-shape dragging.

Press **K** with a shape selected to toggle construction geometry. Its tree row
and viewport outline dim, the flag persists in the web project graph, and
construction geometry cannot be used as the native extrusion profile. Copies
keep the flag; Ctrl+Z and Ctrl+Y undo and redo the toggle.
With a line selected, press **H** for a horizontal constraint or **J** for a
vertical constraint. The line snaps about its first endpoint while retaining
its length. Press the same key again to remove the constraint; the Project row
shows `[H]` or `[V]`. The relation survives copies, undo/redo, and save/reopen.
Select a target line, hover a source line, and press **Shift+P** to make the
lines parallel or **Shift+N** to make them perpendicular. The target rotates
about its first endpoint without changing length. Rotating the source or
applying a horizontal/vertical constraint to it updates the target in the same
undo step. Press the same shortcut over the source again to remove the relation.
The Project rows show `[//]` or `[T]`, and both relations save in the web graph.
**Shift+L** makes the target collinear with the hovered source: it projects the
target onto the source's infinite line while preserving its length. Moving or
rotating the source keeps the target on that line. The Project row shows `[CL]`.
**Shift+B** records the signed Angle between the selected target line and the
hovered source line. Click the Angle value in Properties to enter signed degrees;
rotating the source updates the target in the same undo step. The Project row
shows `[A]`, and the web `angle` relation survives save/reopen.
Select a target line, hover another line, and press **Shift+E** to make their
lengths equal. The target resizes about its first endpoint. Editing the source
length then updates both lines in one undo step. Press Shift+E again over the
same source to remove the relation. Equal pairs save as web `equal` relations;
line split, trim, and extend require removing Equal first. A length typed on the
Equal target drives the source as well.
A line can carry Equal, a direction relation, an axis constraint, a driving
length, and endpoint point links at the same time, and relations can chain
(A parallel to B, B perpendicular to C). Each line can be the target of at most
one Equal and one direction relation. Combinations that cannot be satisfied are
rejected and leave the sketch unchanged.
The same **Shift+E** gesture also matches the radius of two circles or arcs.
Editing a source circle's diameter updates the dependent circle or arc in one
undo step. The target diameter is driven by Equal and cannot be edited directly.
For two circles, select the target, hover the source, and press **Shift+D** to
record their signed radial distance. Click the Diff value in Properties to
edit it; changing the source diameter updates the target radius. The Project
rows show `[RD]`, and the web `radiusDifference` relation survives save/reopen.
Select a circle or arc, hover another circle or arc, and press **Shift+Y** to
make them internally tangent. The smaller curve moves inside the larger one;
their shared contact must lie on each arc sweep. Changing the outer radius or
moving either curve keeps contact. Press Shift+Y again to unlink them. Project
rows show `[IT]`, and the web `internalTangent` relation survives save/reopen.
Select a circle or arc, hover another circle or arc, and press **Shift+Q** to
make them externally tangent. The selected curve moves to contact the hovered
curve, and edits to either curve keep the contact on both arc sweeps. Press
Shift+Q again to unlink them. Project rows show `[T]`, and the web `tangent`
relation survives save/reopen for these curve pairs.
Shift+Q also makes a line tangent to a circle or circular arc. Select the
curve or line, hover the other, and press the same shortcut. The selected
shape moves to contact the other; subsequent moves preserve tangency and arc
contacts stay on their sweeps.
The same gesture supports a line tangent to an ellipse or elliptic arc. Its
contact uses the rotated ellipse's analytic support point; arc contacts must
fall on the retained sweep.
Select a sketch point, hover a line, circular arc, or elliptic arc, and press
**Shift+M** to link the point to the curve's midpoint. Moving, scaling, or
editing the source keeps the point at its midpoint. Press Shift+M again over
the same curve to unlink it. Project rows show `[MID]`, and the web `midpoint`
relation survives save/reopen.
Select a sketch point, hover a line, circle, arc, ellipse, elliptic arc, or
spline, and press **Shift+V** to constrain the point to that finite curve. The
point can slide along the curve; source edits project it back onto the updated
curve. Press Shift+V over the same source to unlink it. Project rows show
`[POC]`, and the web `pointOnCurve` relation survives save/reopen.
Select a line, hover a separate line or curve, and press **Shift+F** to make
the nearer selected endpoint normal to the hovered geometry. The endpoint
snaps onto the finite curve while the line keeps its length. Moving the source
keeps contact and normal direction. Shift+F again over the same source removes
the relation; Project rows show `[NORM]`, and the web `normal` relation survives
save/reopen.
The same gesture matches both width and height for two ellipses or elliptic
arcs. Editing a source axis updates the dependent curve in one undo step;
target axes are driven. Elliptic arc width and height can now be edited in
Properties unless a fixed endpoint would move.
Select a circle, arc, ellipse, or elliptic arc, hover another such curve, and
press **Shift+O** to make their centers concentric. The selected curve moves to
the hovered curve's center. Moving or dragging either curve then moves both
centers in one undo step. Shift+O over the same source removes the relation;
the Project rows show `[O]`. A fixed anchor blocks moves that would displace it.
Rotating a constrained line is supported in 90-degree increments, which swaps
horizontal and vertical when appropriate.
Web projects that store line length, rectangle width or height, horizontal, or
vertical constraints as separate CAD graph nodes also open for editing. Native
edits update those graph nodes in place; deleting an entity removes its attached
supported constraint nodes. Midpoint relations for sketch points, plus the
coincident cases documented above, are also supported; other point and curve
relations still need native solver support.
The **MORE** menu (M) contains Point (P), a regular Polygon (G), and Slot (S). Click once
to place a point or twice to set a polygon's center and first vertex. A polygon
starts with six sides; click its side count in Properties to enter 3–64 sides.
For a slot, click the two end-cap centers, then a point on its side wall. Click
its width in Properties to enter a new millimeter value.
Click **+ TOP**, **+ FRONT**, or **+ RIGHT** (keys 1/2/3) to add a sketch on a
reference plane, and click a sketch in the project panel to switch between them. Unsaved edits in each sketch
are retained while switching; Ctrl+S saves them together.

Pass a `.pcad` path to open a v4 project or start a new one. Ctrl+S saves
supported reference-plane sketch projects in the existing v4 project format. The native
app opens the selected top-level part when `selectedPath` names one, preserves
other project documents, and refuses to save when the file changed
on disk. Projects with unsupported sketch entities, constraints, face-based
sketches, or solid features open read-only until those features are ported.
Rotated rectangle primitives from the web app can now be opened, selected,
dimensioned, moved, scaled, rendered, and saved while preserving their angle.
The native rectangle drawing tool starts axis-aligned; use Rotate to turn it.
For projects with multiple top-level parts, Ctrl+Page Down and Ctrl+Page Up switch
between them. Their names also appear as clickable rows above the sketch list in
the Project panel. Unsaved sketch edits in each part stay in memory; Ctrl+S saves the
edited parts together and records the active part in `selectedPath`.
Choose **MORE → NEW PART** or press **B** to add a top-level part with an empty
Top sketch. The current part's unsaved edits remain available when you switch
back, and Ctrl+S saves both parts.
Press **F2** to rename the active part or **Shift+F2** to rename its active
sketch. Enter applies the name and Ctrl+S persists it.

```sh
cd rust
cargo run -- my-sketch.pcad
```

The pixel font is an intentional visual direction for the native interface.
The web app remains the reference for feature parity: the remaining sketch
constraints and tools, full solid modeling, assemblies, graph editing, full project
browsing, SDK/server behavior, and export still need native ports. A project with
one or more new-body extrusions of closed rectangles, rotated rectangles, circles,
ellipse, polygon, or slot on the Top, Front, or Right reference plane opens in a shaded
3D preview. The preview and STL export use the web app's XY, XZ, and YZ plane
frames. Construction
guides may coexist with that closed shape and are ignored for the profile. Draw one of
those closed shapes on a reference-plane sketch and choose **MORE → EXTRUDE** or press **X** to create a 50 mm new-body
extrusion, then Ctrl+S to save it in the web graph format.
Separate open line components and sketch points may also coexist with the
closed profile; they do not contribute to the extrusion.
The same extrusion command accepts closed simple loops assembled from line
entities and sampled arcs, including concave paths, line-drawn through-holes,
and lines drawn in either direction. Arc segment counts from web projects are
preserved, as are circle, ellipse, and slot segment counts. A sketch containing only
open geometry has no extrusion profile; branching and self-intersecting paths
are rejected.
When a sketch contains several separate closed regions, the selected region is
outlined in cyan. Press **[** or **]** to choose a profile, then **X** to extrude
that region. The selected web `profileId` is saved and reopened. Once a profile
is extruded, press Tab to return to the locked sketch, choose another profile,
and press **X** to create another body from the same sketch. Reusing an already
extruded profile is rejected.
Separate closed shapes fully nested inside one outer profile can form multiple
through-holes. The native preview and STL export triangulate the cap around
them; touching or overlapping boundaries are rejected. A closed island inside a
hole becomes another selectable profile.
In the solid preview, click **DEPTH** in Properties, enter a positive millimeter value,
and press Enter. With several bodies, use **[** and **]** to select the body first.
Ctrl+S saves the updated depth. Use **PERSPECTIVE / ORTHO** in the top-right
header to switch the solid camera projection. Orthographic is the default;
perspective makes nearer geometry appear larger. Switching preserves orbit, pan,
and zoom. The choice resets when opening another project; sketch view remains flat.
Press Tab to switch between the solid preview
and sketch view. The solid viewport uses the web part view's mouse controls:
right-drag orbits freely in screen axes (0.01 rad per pixel, no pitch limit, so the
view can roll) around the surface point under the cursor, middle-drag pans so the
model follows the pointer, and the wheel zooms about the view center with the web's
sensitivity. Left-click does not orbit. The view starts at the web's default angle
(pitch π/5, yaw π/4), and orbiting does not change the model's on-screen size. The extruded sketch locks after extrusion, including on reopen;
add a new reference-plane sketch to create another body in the same part. The 3D
preview shows supported bodies together. Press **Shift+X** on a later closed
reference-plane sketch to cut its 50 mm extrusion from the most recent body.
Press **Shift+J** or **MORE → JOIN** to fuse a later overlapping extrusion with
that body. The shaded preview and STL export use the evaluated Boolean mesh.
The Project panel lists each extrusion feature; select the cut or join
feature to edit its depth or delete it. Earlier features with dependent cuts or
joins cannot be deleted in isolation. Face sketches and chamfers still need native support.
In the 3D preview, press Delete to remove the selected extrusion. Its graph and
feature-tree references are removed; when the sketch has no remaining extrusion,
its geometry is editable again. Supported extrusion projects reopen without a
project-wide read-only lock, so new reference-plane sketches can be added.
Extrusions appear as clickable rows in the Project panel between sketches and
sketch entities. Clicking a row selects that body and opens the 3D preview;
clicking a sketch or entity row returns to the sketch view.
Press Ctrl+E in the solid preview to export each supported body as an ASCII STL
beside the project file. Existing STL files are preserved; additional bodies
and repeated exports use numbered filenames.

## Cross-loader round-trip

The project, model, spline and STL code is a library crate (`rust/src/lib.rs`);
the windowed app in `main.rs` builds on it. For headless use,

```sh
cargo run --manifest-path rust/Cargo.toml -- --roundtrip IN.pcad OUT.pcad
```

loads a project and writes it back exactly as Ctrl+S would, without bumping the
revision or touching `IN.pcad`. It exits non-zero on any load or save error.

Two tests guard against format drift between the apps:

- `rust/tests/roundtrip.rs` (`cargo test`) loads every `rust/fixtures/*.pcad`,
  saves it, reloads the saved copy and saves again; both saves must be identical.
- `test/rust-roundtrip.test.ts` (`bun run test`) loads every Rust fixture in the
  TypeScript loader and checks its part and sketch counts. It also builds a
  web-side project (rectangle and line on the XY plane), passes it through
  `--roundtrip`, and checks the TypeScript loader reads the same entities back.
  That part is skipped, with a warning, when `cargo` is not installed.

## Isolated UI check

The UI smoke test uses a rootless Docker container, Xvfb, and software Vulkan.
It runs Rust unit tests, opens the native app in a virtual display, draws a
rectangle, center rectangle, both three-point rectangle modes, circle, line, ellipse, three-point arc, center arc, point, polygon, and slot with simulated clicks, edits dimensions, polygon sides, and slot width,
copies, moves, scales, and rotates selected shapes, adds Front and Right sketches and switches between them, saves a v4 project, opens a single-extrusion fixture and toggles its view, then authors and saves an extrusion. It writes
screenshots and a project file to `workdir/`. It also extrudes a concave line
loop, line-drawn through-hole, slot, arc-and-line profile, and rectangle with two circular through-holes,
and exports their STLs. The authored extrusion scenario also checks a separate
open line and point beside its closed profile. A web-compatible fixture checks
an ellipse with 16 segments and a circular hole with 12 segments, plus a slot
with 12 segments per end cap. Another scenario extrudes both disconnected
regions from one sketch and checks their saved profile IDs.
It also creates and renames a second part, switches between parts, renames a
sketch, and saves geometry in both parts. A two-body scenario creates extrusions
on Top and Front sketches, saves their graph, and exports both STLs.
It also clicks each extrusion row and edits the selected body's depth.
An axis-constraint scenario snaps one line horizontally and vertically, then
undoes the second constraint before saving.
A fixed-line scenario pins an endpoint with Shift+K, checks the saved relation,
and captures the Properties panel.
A fixed-center scenario pins a circle center and checks its saved web relation
and screenshot.
A fixed-arc scenario pins its end anchor and checks the saved `p1` relation
and screenshot.
An ellipse-axis scenario pins `p0` and checks the saved relation and screenshot.
A corner-rectangle scenario pins `p0`, edits its width, and captures the saved
relation and screenshot.
An Equal scenario creates two lines, matches their lengths with Shift+E,
checks the saved relation, and captures the resulting geometry.
A concentric scenario matches a circle to an ellipse with Shift+O, checks the
saved relation, and captures both curves in the viewport.
Parallel, Perpendicular, and Collinear scenarios match two lines with Shift+P,
Shift+N, and Shift+L,
check their saved relations, and capture each result.
An Angle scenario creates a signed line angle with Shift+B, edits its value,
checks the saved relation, and captures the result.
A Radial distance scenario links two circles with Shift+D, edits the signed
difference, checks the saved relation, and captures both circles.
An Internal tangent scenario links two nested circles with Shift+Y, checks the
saved relation, and captures their contact in the viewport.
An external tangent scenario links two circles with Shift+Q, checks the saved
relation, and captures their contact in the viewport.
A line-circle tangent scenario creates the same web `tangent` relation and
captures the line touching the circle.
A line-ellipse tangent scenario checks the saved relation and captures the
line touching the ellipse's curved boundary.
A midpoint scenario links a point to a line with Shift+M, checks the saved
relation, and captures the point at the line's midpoint.
A Point on curve scenario links a point to a circle with Shift+V, checks the
saved relation, and captures the point on the circle's rim.
A Normal scenario links a line endpoint to a circle with Shift+F, checks the
saved relation, and captures the radial line touching the circle.
An ellipse Equal scenario matches both axes with Shift+E, checks the saved
relation, and captures the two curves.
A radius scenario edits an arc and polygon in Properties, checks both saved
`radius` relations, and captures the result.
An angular scenario edits an arc's signed sweep and a polygon's rotation,
checks the saved relations, and captures both shapes.
A deletion scenario opens a saved extrusion, removes its solid, draws new sketch
geometry, and saves the updated graph.
A graph-constraint fixture checks that an existing horizontal line and length
constraint render correctly and that a length edit updates its graph node.
A three-point circle scenario draws the circle, checks its screenshot, and saves
the web circle primitive.
It does not open a host desktop
window.

```sh
docker --context rootless build -f rust/Dockerfile.ui-test -t puppycad-ui-test rust
docker --context rootless run --rm -v "$PWD/workdir:/out" puppycad-ui-test
```

For a focused projection check, run the following after building the image. It
captures both modes in `workdir/rust-projection-*.png`, checks that the rendered
model changes, and verifies that switching back restores it exactly.

```sh
docker --context rootless run --rm -v "$PWD/workdir:/out" \
  -v "$PWD/rust/ui-projection-smoke.sh:/app/ui-projection-smoke.sh:ro" \
  puppycad-ui-test sh /app/ui-projection-smoke.sh
```

## Continuous integration

`.github/workflows/rust.yml` runs on pushes and pull requests to `master` and
`rust`. The `build-test` job runs `cargo fmt --check`, `cargo build --workspace`,
and `cargo test --workspace` in `rust/`; `cargo clippy --workspace -- -D warnings`
also runs but is non-blocking until existing lints are fixed. The `ui-smoke`
job builds `rust/Dockerfile.ui-test` with plain Docker, runs the smoke test
with `workdir/` mounted at `/out`, and uploads the screenshots as the
`rust-ui-smoke` artifact.
