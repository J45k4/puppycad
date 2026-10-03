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
solid preview the trail ends at the selected feature (extrusion, revolve,
loft or edge finish) instead of the sketch. Project panel rows too long for
the panel are clipped with `...`.
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

**Multi-selection.** In Select mode, drag from empty canvas to draw a rubber
band. Dragging left to right (blue window) selects only shapes fully enclosed
by the box; dragging right to left (green crossing box) selects every shape the
box touches, matching the web workspace. Hold Shift while releasing to add to
the selection. Shift+click a shape (in the viewport or its Project row) to
toggle it in or out of the selection; a plain click selects only that shape,
and Esc or a click on empty canvas clears the selection. The last selected
shape is the primary: Properties shows its fields with the count ("Line
(2 selected)"), and relation gestures that combine the selection with the
hovered shape use it. Every selected shape is highlighted in the viewport and
the Project list. Move/Copy selected, Ctrl+D, Scale/Rotate (about the shared
CX/CY), Delete and **K** act on the whole selection as one undo step (K makes
everything construction unless all of it already is), and dragging any
selected shape moves the whole group; anchors are not individually draggable
while more than one shape is selected. The **GEOMETRY / CONSTR / POINTS**
toggles in the status bar limit what click and box selection can pick
(regular shapes, construction shapes, sketch points); filtered shapes still
render.
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
**Fillet** a line-line corner: select one line, hover a second line whose
endpoint meets (or nearly meets, within a few pixels) the selected line's
endpoint, and press **Shift+W** (or pick **MORE → FILLET** and click the two
lines). Type the radius in the Properties prompt and press Enter; an empty
prompt keeps the last radius (5 mm by default). Both lines are trimmed back
from their intersection and a new arc is inserted, joined to each line by a
web `coincident` relation and kept tangent by an `endpointTangent` (smooth
join) relation. Its radius is a driving `radius` relation, editable in
Properties like any arc radius. **Chamfer** uses the same gesture with
**Shift+H** or **MORE → CHAMFER**: the prompt asks for the first setback and
then the second setback; **Tab** switches to distance-angle mode, where the
second value is the angle in degrees between the first line and the chamfer.
The chamfer line is joined to both trimmed lines with `coincident` relations
and its values are stored as the web `chamfer` relation (`value`,
`secondValue`, `mode`), which the solver keeps driving. Press Shift+H with a
chamfer line selected and nothing hovered to edit its values. Fillet and
chamfer are single undo steps and reject parallel lines, lines that do not
meet at a corner, and values too large for the lines. As in the web app,
relations at the cut corner (fixes, coincidences, joins, non-reference
lengths) are removed; relations at the far ends and line-wide relations stay.
Split and Trim move fillet/chamfer joins to the retained piece. Native
rectangle corners and curved edges are not supported (the web app does not
support them either).
**Offset**: select a line, circle or arc and press **Shift+U** (or pick
**MORE → OFFSET**). Type a signed distance in the Properties prompt and press
Enter; an empty prompt keeps the last distance (5 mm by default). As in the
web app, positive distances move a line to the left of its direction and grow
circles and arcs; negative distances go the other way. The copy is linked to
its source by the web `offset` relation (`a` source, `b` copy, signed
`value`). When the selected line joins other lines end to end (shared
endpoints, with or without coincident links, through vertices where exactly
two lines meet), the whole chain is offset with mitered joins at the
intersections of neighboring offset lines and stored as one web
`offsetChain` relation (ordered `sources` with `reversed` flags, `targets`,
`closed`, `side`, `value`), exactly as the web Offset button stores it. A
closed chain offsets outward for positive distances. The distance is a
driving value: select an offset copy and click **OFFSET … MM** in Properties
to edit it. The solver keeps the copy at that distance when the source moves
or changes size (the source wins, the copy follows); a chain also keeps its
sources joined. Rows show `[OFS]`. Creating or editing an offset is one undo
step. Zero distances, radii that would collapse, reversing corners, edges
that would collapse or reverse, and self-intersecting chain offsets are
rejected. Like the web app, ellipses, elliptic arcs and splines have no
offset yet; mixed line/arc chains and rounded joins are out of scope. Split,
trim, extend, fillet and chamfer refuse offset members; deleting a source or
copy removes the relation.
**Tangent arc**: press **,** (or pick **MORE → TANGENT ARC**), click an
endpoint of a line, arc or open fit/control spline (only curve endpoints
within 8 px are picked), then click the arc's far end. The arc starts at that
endpoint continuing the source's direction and is previewed while the pointer
moves. It is saved with a web `smoothJoin` relation from the source endpoint
to the arc's `p0`, which the solver keeps (the endpoints coincide and their
tangents line up). A spline endpoint with a zero tangent (a control spline
whose first handle sits on its endpoint) is rejected with a status message,
as is an arc end on the tangent line; neither adds an undo step.
**Smooth join**: select a line, arc or open spline, hover another one and
press **.**: the nearest endpoint pair of the two gets a `smoothJoin`, and the
solver moves the hovered curve so its endpoint meets the selected curve's
endpoint and continues its tangent. Press **.** again over the same pair to
remove it. Each endpoint takes at most one smooth join. Joined curves show
`[TJ]` in the project rows. Web `smoothJoin` relations between line, arc and
open spline endpoints (`p0`/`p1`, `point0`/`pointN`) load and save; Split
and Trim move them (and the fillet's joins, fixes and point links) from a
curve's end to the retained piece and drop them from a cut-away end.
Spline relations hold on both fit and control splines through the solver:
fixed handles, point-on-curve, Normal, endpoint coincidence with sketch
points, symmetry/mirror, smooth joins and tangent-arc attachment. Midpoint
on a spline is rejected, as in the web app; closed control splines,
curvature constraints and handle continuity are not supported (they are open
on the web as well).
Set **CX** and **CY** to the transform center in millimeters. The default is the sketch origin. Set a positive scale factor in Properties and click **Scale selected** or **Scale copy** to resize about that center. Existing driving dimension values scale with the selected shape.
The **Rotate** tab uses a signed angle in degrees and offers **Rotate selected** and **Rotate copy** about the same editable center. Rotating an axis-aligned rectangle converts it to a rotated rectangle primitive that the web app can open.
Move, scale, and rotate copies retain their editable driving dimensions.
Click line length, rectangle or ellipse width or height, or circle diameter in Properties
to enter a new value or expression (see Dimensions). Press Enter to apply it or Escape to cancel;
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

### Dimensions

Dimensions are sketch objects (`model/annotations.rs`), keyed by
`DimensionKey`: a per-shape dimension (`Field`), a line angle relation
(`Angle`), or a point dimension (`Point`).

**Canvas labels.** Every dimension is drawn in the viewport: a line length
parallel to the line and offset to one side, rectangle and ellipse width and
height along their edges or axes, a diameter with a leader through the
circle, a radius with a leader from the center, arc sweep inside the arc,
rotation next to the pivot, an angle at the vertex of its two lines, and
point distances along an aligned, horizontal (below the points) or vertical
(right of the points) dimension line. Offsets are fixed screen pixels, so the
default placement is deterministic per zoom. In Select, click a label to edit
it inline (type, Enter applies, Esc cancels) or drag it to move it; a moved
label is stored as the web `labelPosition` (absolute sketch coordinates) and
gets a leader back to its geometry. Clicking a label selects the dimension;
Properties then shows its kind, value, a **[R] MAKE REFERENCE / MAKE
DRIVING** button, and Del removes it.

**D tool.** Press **D**. Click an entity to add its next missing driving
dimension at its current size (length, diameter, radius, or width then
height), or click two anchors (sketch points, line ends, curve centers,
spline handles) for an aligned distance; hold Shift on the second click for
an X distance and Ctrl for a Y distance. Picking the origin as the second
point makes an absolute X/Y (or radial) dimension, measured from a fixed
construction point at the origin that is created once and reused (the web
format has no origin anchor). The new label opens for typing its value.
Point dimensions save as web `distance` relations
(`{a, b, value, axis?}`, signed `b - a` along an axis) and solve as the
`PointDistance` and `AxisDistance` residuals. Web `distance` relations with a
`direction` keep the sketch read-only. Line-to-point distance is not offered:
the web app has no such relation.

**Driving and reference.** **R** (or the Properties button) toggles the
selected dimension. A reference dimension shows its measured value in
parentheses, adds no solver constraint, and saves as the web relation with
`reference: true` and the measured value. Making it driving again starts
from the measured value. Angle relations cannot be references.

**Expressions and variables.** Dimension values (labels and the Properties
fields) accept expressions as in `src/sketch-value.ts`: `+ - * / ( )`, `pi`,
units `mm cm m um in inch ft deg rad`, and `#name` sketch variables
(`rust/src/sketch_value.rs`). Adding mismatched units is an error, as in the
web app. A label with an expression shows `expression = value`. With nothing
selected, Properties lists **SKETCH VARIABLES**: click a row to edit it as
`name = expression` (an empty expression deletes an unused variable,
renaming rewrites `#name` references), or **+ ADD VARIABLE**. A variable's
kind is inferred (its previous kind, else length, angle, scalar). Changing a
variable re-evaluates every dependent dimension and solves as one undo step;
any failure rolls everything back. Expressions save as the web relation
`expression` next to the exactly evaluated `value` (the web loader checks
they match), and variables as the sketch node's `variables` array. An
expression that no longer produces the stored value (after a direct edit such
as scaling) is dropped on save. Dimensions stored as graph `sketchConstraint`
nodes move to sketch relations once they get a label position, reference
flag or expression.

### Constraint solver

`rust/src/solver.rs` is a least-squares solver ported from the structure of
`src/sketch-solver.ts`. Every anchor coordinate (line endpoints, sketch points,
curve centers, spline handles), every circle or arc radius, ellipse semi-axes
and rotation, and arc start and sweep angles are parameters; each relation
contributes a residual. A smooth join (web `smoothJoin`) is four residuals:
the two endpoints coincide and their outward unit tangents sum to zero; it
works between any line, arc and open spline endpoints. Point-on-curve and Normal relations on a spline add an
auxiliary curve parameter. A curve keeps its size and its angles unless a
relation or driving value may change them (Equal, radial distance, shared
ellipses, radius, axis, sweep or rotation dimensions, smooth joins, fixed arc
or axis endpoints), so tangency and point relations move curves rigidly. Levenberg–Marquardt with numeric Jacobians
first solves the relations together with soft "stay close" weights and then
solves the relations alone, damped by those weights. A solve reports its
residual norm and whether it converged.

Moves, drags, dimension edits, scale, rotate, and creating a relation all apply
the edit, solve, and then either commit everything as one undo step or roll the
edit back. The edited shape (or, for a length or rotation edit, the line's first
endpoint) is held exactly; an edited shape that is the target of a
projecting relation (point-on-curve, Normal, tangency, smooth join) is instead
held as a strong preference, so the relation snaps it back onto its source.
The remaining intent is a cascade: relation sources
stay put and only targets move; if that fails, sources may move but keep their
first endpoint; if that fails too, only the edited geometry is held. Targets
rotate or stretch about their first endpoint, and direction targets keep their
length when nothing else sets it. A solution that collapses a line is rejected.

Every relation is solver-backed: horizontal and vertical, line length and line
rotation dimensions, point distance and point X / Y dimensions, Equal lines, Parallel, Perpendicular, Collinear, signed
Angle, endpoint coincidence between a sketch point and a line or spline end,
fixed line endpoints, points, centers, spline handles, and arc, elliptic arc
and ellipse axis endpoints, Midpoint on lines, arcs and elliptic arcs,
point-on-curve on lines, circles, arcs, ellipses, elliptic arcs and splines,
Normal and smooth-join (endpoint) tangency, internal tangency between circles
and arcs, external tangency between circles and arcs and between a line and a
circle, arc, ellipse or elliptic arc, circle and arc radius and diameter
dimensions, Equal radius, radial distance, shared and Equal ellipses, ellipse
axis, rotation and sweep dimensions of related curves, and Concentric between
any circular or elliptic curves. Point-on-curve keeps the point on the finite
segment, sweep, or spline span, and tangency keeps the contact inside an arc's
sweep. Point symmetry (the axis line is the perpendicular bisector of
the two anchors) and whole-shape mirror relations of lines, points, circles,
arcs, ellipses, elliptic arcs and splines (anchor pairs, equal radii or axes,
mirrored arc start and ellipse rotation, negated sweeps) are solver residuals
too, as are offsets (a line copy at a signed distance to the left of its
source, concentric circles and arcs with a radius difference and equal
angles, and a mitered line chain offset that also keeps its sources joined).
The only hand-propagated relation is a mirror copy of a rectangle,
polygon or slot, which is regenerated from its source and axis after every
solve. Pattern copies (see **Patterns**) are likewise regenerated from their
sources after every solve, for every shape kind.

Rotating a direction target now turns its source with it; Equal ellipses can
be scaled; splines with linked end points can be moved, scaled, rotated, and
have their end handles edited or dragged, carrying the points along. Rotating
axis-constrained lines by other than quarter turns stays blocked.

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
whole-shape drag (all relations, including tangency and shared ellipses,
follow). Release commits one undo step, Esc cancels. A fixed anchor
(or any fixed anchor on a non-line shape) refuses to drag; every relation
the shape takes part in follows through the solver.

#### Degrees of freedom and diagnostics

`Problem::analyze` ports `src/sketch-redundancy.ts`: it builds the numeric
Jacobian of the relation residuals over the free parameters, orthonormalizes
it relation by relation (two-pass Gram–Schmidt), and reports the rank, the
remaining degrees of freedom, which parameters are fully determined, and for
each relation whether it is satisfied, conflicting (residual above tolerance at
the current geometry) or redundant (adds no rank after earlier relations).
`Document::analyze_constraints` runs it over every solver-modeled shape: lines,
sketch points, circles, arcs, ellipses, elliptic arcs and splines (rectangles,
polygons and slots are not counted). A free line has 4 DOF, a point 2, a circle
3, an arc 5 (center, radius, start, sweep), an ellipse 5 (center, axes,
rotation), an elliptic arc 7, a spline 2 per handle.

In the sketch view the header's right edge shows **DOF n** (green at 0),
**OVER-CONSTRAINED** (purple) when a relation is redundant, or **CONFLICT**
(red) when a relation is unmet. Project rows of shapes in a conflicting
relation get **[!]** and red text, shapes in a redundant relation **[R]** and
purple text; the viewport tints those shapes the same way and draws fully
constrained shapes green. The analysis is cached by `Document::revision()` and
recomputed once per committed edit, undo or redo, never during a drag. Because
edits that cannot be solved are rolled back, a conflict normally appears only
in documents loaded that way.

#### Viewport feedback

In Select mode the shape under the cursor (honoring the selection filters)
is pre-highlighted in a lighter accent and its anchors show as faint squares
before any click. Pointer moves in idle Select mode repaint only when the
hovered shape, anchor or glyph changes; placing, dragging, panning and the 3D
view still repaint on every move.

Every relation draws a small glyph beside each entity it involves
(`model/glyphs.rs`, `Document::placed_glyphs`): `H`/`V`, `//` parallel, `T`
perpendicular, `COL`, `ANG`, `=` equal, `O` concentric, `TAN` tangent
(including endpoint tangents), `MID`, `POC`, `NORM`, `TJ` smooth join, `SE`,
`RD`, `SYM`, `OFS`, `PAT`, and `FIX` with a lock square at each fixed anchor.
The 8x8 font is ASCII only, hence `//`, `T` and `TAN` instead of the Unicode
symbols. A line's glyphs start at its midpoint and a circle's or arc's on its
rim; they stack outward along the entity's normal (above/right of a line,
radially for curves) so the glyphs of one entity never overlap, and use the
entity's DOF tint. Hovering a glyph highlights every partner of its relation;
clicking it selects the entity it sits beside. Key **6** hides or shows all
glyphs (`GLY` in the status bar).

While placing a segment (a line, the first edge of a three-point rectangle,
an arc chord or a slot axis) an inference hint appears at the cursor with a
dashed guide when the segment is within 2° of horizontal/vertical (`H`/`V`),
parallel or perpendicular (`//`/`T`) to a line within 30 px of the cursor, or
tangent (`TAN`) to a circle it ends on or an arc whose end it starts from.
Hints are visual only, except that with automatic constraints on (the web
"Automatic constraints" checkbox; key **7**, `AUTO` in the status bar, on by
default) a line committed while its `H`/`V` hint showed snaps exactly to the
axis and gets that relation in the same undo step.

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
**Symmetry.** Select a mirror line (usually a construction line) and press
**Shift+S**, then click the first shape and its mirror. Two shapes of the same
kind (other than sketch points) become whole-shape symmetric: the second one
jumps to the mirror image of the first (web `mirror` relation). Otherwise the
anchors nearest to the two clicks (line ends, points, centers, spline points)
become symmetric about the line (web `symmetric` relation). Esc cancels; the
same gesture over the same pair removes the relation. Project rows show
`[SYM]`. Editing either side, or the mirror line, updates the other side in
one undo step; the mirror line stays put while a side is edited when it can.
**Shift+I** or **MORE → MIRROR** mirrors the selected shape across the next
clicked line: it adds a mirror copy linked by a `mirror` relation, as one
undo step (a corner rectangle's copy is a rotated rectangle, as in the web
app). The mirror line must be an existing line. Deleting the line, the source
or the copy removes the relation.
**Patterns.** Select one or more shapes (click, Shift+click or box select) and
press **Shift+Z** or **MORE → LIN PATTERN**: Properties prompts for COUNT
(cells per row, source included), SPACING X / Y (mm), ROWS (1 for a single
row) and, for a grid, ROW SPACING X / Y; Enter keeps the shown value. The copies
are linked by a web `linearPattern` relation (`step`, `columns`, `rowStep`).
**;** (semicolon) or **MORE → CIRC PATTERN** (the two pattern items open a
second MORE column left of the first) prompts for COUNT (source included) and
ANGLE (default 360), then waits for a click on the center: a sketch point, line
end or curve center under the cursor becomes a referenced center
(`centerAnchor`, the pattern follows it), a click on the origin uses the
origin, anywhere else uses that point. Full circles step by angle/count, open
arcs by angle/(count-1) and include both ends, as in the web app. Each pattern
is one undo step, copies keep the source's construction flag, and Project rows
show `[PAT]`. Copies follow the source: editing, moving or dimensioning a
source updates every copy (copies themselves are not independently editable).
With a pattern member selected, Shift+Z / ; reopen the prompts with the
pattern's current values and edit that pattern (copies of surviving cells are
kept, removed cells are deleted). Deleting the source, a copy or the
referenced center removes the pattern relation and leaves the remaining
copies as free geometry, matching the web app. Patterns hold 2-32 cells and at
most 64 copies; skipped instances are not supported.
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
on disk. Projects with unsupported sketch entities, constraints, or solid
features open read-only until those features are ported. Face sketches open
editable when their face still exists on the supporting extrusion.
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
those closed shapes on a reference-plane sketch and choose **MORE → EXTRUDE** or press **X** and Enter to create a 50 mm new-body
extrusion (see *Extrude options and preview*), then Ctrl+S to save it in the web graph format.
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
Every body (plain extrusions, cut/join results, revolve, loft and edge
finish results) is drawn the same way: each triangle is lit per face normal
by a key light from the upper right plus ambient and a weak head-on fill, and
sharp edges (adjacent face normals more than 20° apart, the same edges an
edge finish picks) are drawn as dark lines, with pieces hidden behind the
mesh left out, so holes and chamfer bevels read clearly.
The Project panel lists each solid feature in tree order (see Feature
tree); select the cut or join feature to edit its depth or delete it. Earlier features with dependent cuts or
joins cannot be deleted in isolation. Revolve, loft and edge chamfer/fillet
features are described below; web `chamfer` graph nodes still need native
support.

### Sketches on faces

In the 3D preview, hovering a planar face outlines it in yellow (the
evaluated mesh is ray-cast under the cursor and its coplanar, edge-connected
triangles are grouped into one face) and names it, e.g. **TOP FACE**. Press
**4** or **MORE → SKETCH ON FACE** to add a sketch on that face and open it.
Faces that do not belong to an extrusion prism (for example the wall of a
cut) are outlined in grey and cannot carry a sketch.

The face sketch is saved like the web app's: a graph `face` node
`face-<extrudeId>-<faceId>` with `sourceId` (the extrusion) and `faceId`
(`<extrudeId>-solid-face-<n>`), targeted by the sketch's `targetId`. Faces and
plane frames follow `getExtrudedFaceDescriptors` in `src/cad/extrude.ts`: one
side face per profile point (outer loop, then holes), then the bottom and top
caps. A side face's origin is its first profile point, x runs along the edge,
y along the extrusion normal and the normal is x × y; caps keep the source
sketch's axes, the bottom with a negated normal, the top moved by the depth.
Frames are recomputed from the graph on every evaluation, so a face sketch
follows its source when that extrusion's depth changes, as on the web.

While a face sketch is open, the supporting extrusion's edges are projected
into the sketch plane and drawn dashed between the grid and the sketch (web
`sketchSupportEdges`); their ends and midpoints snap like sketch geometry.
Press **5** to hide or show this context. They are display references only and
take no relations.

**X**, **Shift+J** and **Shift+X** on a face sketch extrude along the face
normal: new bodies and joins grow outward, cuts run into the body from the
face. The result joins the same Boolean evaluation as reference-plane
extrusions. The web graph has no direction field and the web part view does
not evaluate graph cuts, so the inward cut direction is a native rule; faces
of such cuts are not offered for further face sketches.
In the 3D preview, press Delete to remove the selected extrusion. Its graph and
feature-tree references are removed; when the sketch has no remaining extrusion,
its geometry is editable again. Supported extrusion projects reopen without a
project-wide read-only lock, so new reference-plane sketches can be added.
Extrusions appear as clickable rows in the Project panel between sketches and
sketch entities. Clicking a row selects that body and opens the 3D preview;
clicking a sketch or entity row returns to the sketch view.
Press Ctrl+E in the solid preview to export each supported body as a binary STL
beside the project file (80-byte header `PuppyCAD <part> / <body>`,
little-endian facet count, 50 bytes per facet with a per-facet normal and a zero
attribute). Ctrl+Shift+E writes the same triangles as ASCII STL. Existing files
are preserved; additional bodies and repeated exports use numbered filenames
(`name.stl`, `name-2.stl`, …). Ctrl+Alt+E writes one `name.3mf` for the whole
part: a deflated ZIP with `[Content_Types].xml`, `_rels/.rels` and
`3D/3dmodel.model` (core 2015/02 namespace, `unit="millimeter"`, the part name
as `Title` metadata, one welded-vertex `object` and one `build` item per body).
When cut/join features merge extrusions, every format exports the evaluated
Boolean bodies. STEP export is not implemented yet.

Headless export uses the same code:

```sh
cargo run --manifest-path rust/Cargo.toml -- --export IN.pcad OUT.stl   # binary, OUT-2.stl… per extra body
cargo run --manifest-path rust/Cargo.toml -- --export IN.pcad OUT.3mf
```

It prints the written paths, replaces existing files at those paths, and exits
non-zero when the input is missing, has no supported solid, or the output
extension is not `.stl`/`.3mf`. `rust/tests/export.rs` and
`test/rust-export.test.ts` cover it.

### Extrude options and preview

**X**, **Shift+X** (cut) and **Shift+J** (join), and the MORE menu entries, do
not create the extrusion right away. They switch to the 3D view with a
translucent ghost of the would-be tool solid (orange, red-tinted for cuts) over
the existing bodies and open a DEPTH prompt in the viewport and Properties.
Type a depth, press **E** to cycle the end condition and **F** to flip; the
ghost updates on each key. **Enter** commits (an empty prompt keeps the old
one-key default of 50 mm blind), **Esc** cancels and returns to the sketch.

For a selected extrusion in the solid view, Properties shows DEPTH, END and
FLIP rows, plus TARGET for up-to-body. Click END or press **E** to cycle
Blind → Symmetric → Through all → Up to body → Blind (through all and up to
body only for cut and join features once a body exists). Click FLIP or press
**F** to reverse the direction. Click TARGET or press **U** to arm a body pick,
then click a body in the viewport (**Esc** cancels). Each edit re-evaluates the
Boolean mesh and is one undo step: **Ctrl+Z** / **Ctrl+Shift+Z** (or Ctrl+Y)
in the solid view undo and redo extrusion edits, including depth edits. An
edit that would not evaluate is rejected and leaves the graph unchanged.

End conditions:

- **Blind**: `depth` along the sketch normal (`-depth..0` when flipped).
- **Symmetric**: `depth` is the total length, centred on the sketch plane;
  flip has no effect.
- **Through all**: spans the previous body along the sketch normal in both
  directions with a small margin, like the web's through-all solid step.
  Flip has no effect.
- **Up to body**: from the sketch plane to the far side of the target body's
  bounding box along the (flipped) normal. The target must be an earlier
  new-body extrusion.

File format: the `extrude` graph node keeps the fields the web writes
(`operation`, `depth`) and adds optional `endCondition` (`"symmetric"`,
`"through-all"` — the web's solid-step spelling — or `"up-to-body"` with
`targetId` naming the body's extrude node) and `flip: true`. Defaults are
omitted, so blind extrusions stay byte-identical. The web loader reads such
files and evaluates them as blind with the stored depth (it ignores the extra
fields). Unknown end conditions, a non-boolean `flip`, a missing or later
up-to-body target, or through-all/up-to-body on a new body open read-only with
an `extrusion options:` reason.

Face sketches use the same options along the face normal. A cut from a face
sketch runs into the body by default and new bodies/joins grow outward; flip
inverts that default. Through all and up to body measure the target's extent
from the face plane. Fixture: `rust/fixtures/extrude-options.pcad`.

### Feature tree

Each part's solid features (`FeatureKind`: extrusions — new body, cut, join
— revolves, lofts and edge finishes; sketches are not features) form an
ordered list. The Project panel shows one row per feature in that order, then a
**ROLLBACK** row. Row glyphs: `[S]` and dim text for suppressed features, red
`[!]` for features in error (clicking the row shows the reason in the status
bar), dim text for features after the rollback bar. When the rows do not
fit, the mouse wheel over the Project panel scrolls them (one row per notch);
after a key changes the selected feature (or the active sketch) its row is
scrolled into view.

With a feature row selected in the 3D view:

- **Ctrl+Up / Ctrl+Down** move it one place earlier or later. A move is
  refused (status message) when a feature would come before something it
  depends on — its up-to-body target, the extrusion owning its sketch face,
  graph `sourceId`/`targetId` — or a cut/join would end up applying to a
  different body (cuts and joins apply to the nearest earlier new body), or
  the reordered tree would not evaluate.
- **Ctrl+H** toggles suppression.
- **Ctrl+[ / Ctrl+]** move the rollback bar up or down; the ROLLBACK row can
  also be dragged with the left mouse button and dropped between features.

Each of these is one undo step (**Ctrl+Z** / **Ctrl+Shift+Z** in the solid
view) and re-evaluates the part.

Evaluation runs features in order up to the rollback bar and skips suppressed
ones. A feature whose inputs are missing (sketch or profile gone, its target
body suppressed or failed, the face it is sketched on belongs to an inactive
feature, the end condition reaches no body) or whose Boolean result is empty
is marked as an error and skipped; the rest of the part still evaluates. The
3D view, STL/3MF export (UI and `--export`) use only active features, so
export respects suppression and rollback. Creating a new feature moves the
rollback bar to the end; deleting a feature above the bar keeps it below the
same features.

Results are cached per feature, keyed by the feature node, its resolved
profile and frame, and its position relative to the bar. A feature is rebuilt
only when it or an earlier feature changed. Sketch edits are copied into the
project when you return to the 3D view (**Tab** or the part breadcrumb), so
the features using that sketch and everything after them re-evaluate there.

File format (native extensions the web loader ignores): order is the order of
the feature nodes in `data.cad.nodes`, mirrored in `data.cad.rootNodeIds` and
`data.tree.orderedNodeIds`. A suppressed node has `"suppressed": true` (same
name as the web `BaseFeature.suppressed` flag; omitted when false). The
rollback bar is `data.tree.rollbackIndex`, the number of features above it,
omitted when the bar is at the end. The web reads such files with all
features active and drops `rollbackIndex` when it re-saves the tree. Files
that already have unresolvable features when opened stay read-only, as
before. Fixture: `rust/fixtures/feature-tree.pcad` (body, suppressed cut,
join); tests in `rust/src/project/features_tests.rs`.

### Revolve, loft and edge finishes

These are `FeatureKind` variants (`Revolve`, `Loft`, `EdgeFinish`) of the
feature tree above, evaluated in tree order together with extrusions through
`manifold-rust` (`rust/src/project/features.rs`), with the same suppression,
rollback, error status and per-feature caching (revolve and loft cache keys
include their resolved profiles). Revolve and loft cuts/joins apply to the
nearest earlier new body, like extrusions; an edge finish applies to the body
that has its edges. They appear as feature tree rows (`REVOLVE NEWBODY`,
`LOFT JOIN`, `CHAMFER`, ...); selecting one shows its value (ANGLE,
DISTANCE or RADIUS) in Properties, where clicking the value edits it (one
undo step, **Ctrl+Z** / **Ctrl+Shift+Z** in the solid view), and **Delete**
removes it unless a later feature depends on it (also one undo step).

- **Revolve** — key **8** or **MORE → REVOLVE** in a sketch with a closed
  profile selected (**[ ]** cycle profiles as for extrude). Click a sketch
  line as the axis, or press **X** / **Y** for the sketch axes. The view
  switches to a translucent ghost with an ANGLE prompt (empty = 360°); **O**
  cycles new body / cut / join, **Enter** commits, **Esc** cancels. The
  profile (with holes) is swept in 32 segments per full turn (fewer for
  partial angles, capped at both ends); it may touch the axis but not cross
  it.
- **Loft** — key **9** (or **MORE → LOFT**) remembers the selected profile of
  the current sketch; open another sketch, select its profile and press **9**
  again for a new body or **Shift+9** to join the last body (**Esc** clears the
  first pick). Both outer loops are resampled at the union of their
  arc-length parameters (every original vertex is kept) after aligning the
  second loop's start to the point nearest the first loop's start and
  matching winding; the side quads and both caps form the mesh. Profiles with
  holes, edge-on profiles and self-intersecting results are rejected.
- **Edge chamfer / fillet** — key **0** (chamfer) or **Shift+0** (fillet), or
  **MORE → EDGE CHAMFER / EDGE FILLET**, in the solid view. Hovering
  highlights the nearest sharp edge (adjacent face normals more than 20°
  apart, collinear mesh segments between the same two planes merged); click
  edges to add or remove them, type the DISTANCE / RADIUS (empty = 2 mm),
  **Enter** commits. Only straight edges between planar faces are accepted;
  facets of tessellated curves are shown grey and rejected. Convex edges are
  cut with a prism along the edge whose cross-section is the material outside
  the chamfer line or the fillet arc (16 segments per 90°); concave edges get
  the same prism added. Hover skips edges hidden behind the bodies (the
  point of the edge nearest the cursor is ray-cast against the mesh).

File format: none of these exist as web graph nodes (the web `chamfer` node
references web topology `edge` nodes the native app cannot resolve), so they
are native extension nodes that the web loader drops when it normalizes the
graph:

```json
{"type":"revolve","id":"native-revolve-1","name":"Revolve 1","sketchId":"…","profileId":"<sketchId>-profile-1",
 "axis":{"a":{"x":0,"y":0},"b":{"x":0,"y":1}},"angle":360,"operation":"newBody"}
{"type":"loft","id":"native-loft-1","name":"Loft 1","operation":"newBody",
 "profiles":[{"sketchId":"…","profileId":"…"},{"sketchId":"…","profileId":"…"}]}
{"type":"edgeFinish","id":"native-edge-finish-1","name":"Chamfer 1","kind":"chamfer","distance":2,
 "edges":[{"a":[x,y,z],"b":[x,y,z],"normals":[[nx,ny,nz],[nx,ny,nz]]}]}
```

The axis is in sketch coordinates and the angle in degrees (0, 360]; loft
operations are `newBody` or `join`; a fillet stores `radius` instead of
`distance`. Edges are stored by their world endpoints and the outward normals
of the two faces and are re-identified on every evaluation: first by
endpoints, then by an edge between the same two planes on the same line, so a
finish survives upstream changes that keep its edge (for example a deeper
extrusion under a bottom edge). A finish whose edge no longer exists, an
invalid angle, axis or operation in a file being opened, and web `chamfer`
nodes, open the part read-only with the evaluation error as the reason;
edits made natively mark the feature `[!]` instead. Sketches used by a revolve
or loft lock like extruded sketches. STL/3MF export uses the evaluated bodies.
Fixture: `rust/fixtures/revolve-loft-finish.pcad`; tests in
`rust/src/project/feature_tests.rs`, `rust/src/project/features_tests.rs`
and `rust/src/app/tests.rs`.

## Shortcuts

Every command lives in one registry (`rust/src/app/commands.rs`): label,
shortcut, category, availability (view, editable sketch, selection) and the
action it runs. Keyboard bindings, the MORE menu, the command palette and the
cheat sheet all dispatch through it.

- **Ctrl+K** (or **Ctrl+Shift+P**) opens the command palette over the
  viewport. Typing filters by label, category and shortcut text (label
  prefix first, then word starts, substrings and finally subsequences, so
  `mirr` or `ctrl+s` find their command). Up/Down move, Enter runs the
  highlighted command, Esc or a click outside closes it; clicking a row runs
  it. Commands that cannot run right now (wrong view, read-only sketch, no
  selection) are dimmed and skipped by the selection.
- **?** (Shift+/) or **F1** toggles a full-window cheat sheet grouped by
  category, generated from the registry and balanced over as many columns as
  the window fits (three at 1280×800). Esc, ?, F1 or a click close it.

Some keys mean different things in a modal context: **R** toggles a selected
dimension between driving and reference instead of drawing a rectangle,
**W** flips the sweep while the center-arc tool is active, **Enter** finishes
a spline, **Esc** cancels an armed up-to-body pick, **E**/**F**/**U** edit the
selected extrusion in the 3D preview (end condition, flip, up-to body) but
are Ellipse/Fit/Extend in the sketch, and **[** / **]** cycle profiles in the sketch but solids in the 3D
preview (with Ctrl they move the rollback bar). Revolve, loft and edge
finishes use **8**, **9** / **Shift+9** and **0** / **Shift+0** because **6** and
**7** toggle relation glyphs and automatic constraints. A unit test checks that no chord is bound twice within the same
context. The table below is the output of
`cargo run --manifest-path rust/Cargo.toml -- --print-shortcuts`
("editable": not in read-only sketches; "selection": needs a selected shape
or dimension):

#### Draw

| Shortcut | Command | When |
| --- | --- | --- |
| `V` | Select | sketch |
| `L` | Line | sketch |
| `R` | Rectangle | sketch |
| `Shift+R` | Center rectangle | sketch |
| `T` | 3-point rectangle | sketch |
| `Shift+T` | 3-point center rectangle | sketch |
| `C` | Circle | sketch |
| `Shift+C` | 3-point circle | sketch |
| `E` | Ellipse | sketch |
| `A` | Arc (3-point) | sketch |
| `Shift+A` | Center arc | sketch |
| `W` | Flip center arc direction | center-arc tool |
| `,` | Tangent arc | sketch |
| `P` | Point | sketch |
| `G` | Polygon | sketch |
| `S` | Slot | sketch |
| palette / MORE | Fit spline | sketch |
| palette / MORE | Control spline | sketch |
| `Enter` | Finish spline | placing a spline |
| `Shift+Enter` | Finish closed spline | placing a spline |

#### Edit

| Shortcut | Command | When |
| --- | --- | --- |
| `I` | Split curve at cursor | sketch, editable, selection |
| `Q` | Trim curve at cursor | sketch, editable, selection |
| `U` | Extend curve to cursor | sketch, editable, selection |
| `Shift+I` | Mirror | sketch, editable, selection |
| `Shift+U` | Offset | sketch, editable, selection |
| `Shift+W` | Fillet selected + hovered line | sketch, editable, selection |
| `Shift+H` | Chamfer selected + hovered line | sketch, editable, selection |
| palette / MORE | Fillet (pick two lines) | sketch, editable |
| palette / MORE | Chamfer (pick two lines) | sketch, editable |
| `Shift+Z` | Linear pattern | sketch, selection |
| `;` | Circular pattern | sketch, selection |
| `Ctrl+D` | Copy selected | any view, editable, selection |
| `Delete` / `Backspace` | Delete selection / extrusion | any view |
| `K` | Toggle construction | any view, editable, selection |
| `Ctrl+Z` | Undo | any view, editable |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo | any view, editable |
| `Esc` | Cancel / clear selection | any view |

#### Constrain

| Shortcut | Command | When |
| --- | --- | --- |
| `H` | Horizontal | sketch, editable, selection |
| `J` | Vertical | sketch, editable, selection |
| `Shift+S` | Symmetric about selected line | sketch, editable, selection |
| `Shift+V` | Point on hovered curve | sketch, editable, selection |
| `Shift+Q` | Tangent to hovered curve | sketch, editable, selection |
| `Shift+Y` | Internal tangent | sketch, editable, selection |
| `Shift+O` | Concentric | sketch, editable, selection |
| `Shift+E` | Equal | sketch, editable, selection |
| `Shift+P` | Parallel | sketch, editable, selection |
| `Shift+N` | Perpendicular | sketch, editable, selection |
| `Shift+L` | Collinear | sketch, editable, selection |
| `Shift+M` | Midpoint of hovered line/arc | sketch, editable, selection |
| `Shift+F` | Normal to hovered curve | sketch, editable, selection |
| `Shift+K` | Fix anchor at cursor | any view, editable, selection |
| `.` | Smooth join (G1) | sketch, editable, selection |

#### Dimension

| Shortcut | Command | When |
| --- | --- | --- |
| `D` | Dimension tool | sketch |
| `R` | Toggle driving / reference | dimension selected, selection |
| `Shift+B` | Angle to hovered line | sketch, editable, selection |
| `Shift+D` | Radial distance to hovered circle | sketch, editable, selection |

#### Solid

| Shortcut | Command | When |
| --- | --- | --- |
| `X` | Extrude new body | any view |
| `Shift+J` | Extrude join | any view |
| `Shift+X` | Extrude cut | any view |
| `4` | Sketch on hovered face | any view |
| `5` | Toggle face body context | any view |
| `E` | Cycle extrusion end condition | 3D preview |
| `F` | Flip extrusion direction | 3D preview |
| `U` | Pick up-to body | 3D preview |
| `Esc` | Cancel body pick | picking an up-to body |
| `Tab` | Toggle 3D preview | any view |
| `[` | Previous profile | sketch |
| `]` | Next profile | sketch |
| `[` | Previous solid | 3D preview |
| `]` | Next solid | 3D preview |
| `Ctrl+Up` | Move feature up | 3D preview |
| `Ctrl+Down` | Move feature down | 3D preview |
| `Ctrl+H` | Suppress / unsuppress feature | 3D preview |
| `Ctrl+[` | Roll back one feature | 3D preview |
| `Ctrl+]` | Roll forward one feature | 3D preview |
| `8` | Revolve profile | sketch |
| `9` | Loft (new body) | sketch |
| `Shift+9` | Loft (join) | sketch |
| `0` | Edge chamfer | any view |
| `Shift+0` | Edge fillet | any view |

#### View

| Shortcut | Command | When |
| --- | --- | --- |
| `6` | Toggle relation glyphs | any view |
| `7` | Toggle automatic constraints | any view |
| `F` | Fit sketch to view | sketch |
| `Shift+G` | Toggle grid snapping | any view |
| `N` | Toggle geometry snapping | any view |
| `M` | MORE menu | any view |
| `Ctrl+K` / `Ctrl+Shift+P` | Command palette | any view |
| `?` / `F1` | Keyboard shortcuts | any view |
| `Ctrl+,` | Settings | any view |

#### Project

| Shortcut | Command | When |
| --- | --- | --- |
| `B` | New part | any view |
| `1` | New sketch on TOP | any view |
| `2` | New sketch on FRONT | any view |
| `3` | New sketch on RIGHT | any view |
| `Ctrl+PageDown` | Next part | any view |
| `Ctrl+PageUp` | Previous part | any view |
| `F2` | Rename part | any view |
| `Shift+F2` | Rename sketch | any view |

#### File

| Shortcut | Command | When |
| --- | --- | --- |
| `Ctrl+S` | Save project | any view |
| `Ctrl+E` | Export binary STL | any view |
| `Ctrl+Shift+E` | Export ASCII STL | any view |
| `Ctrl+Alt+E` | Export 3MF | any view |

## Interface

### Status bar, tooltips and toolbar glyphs

The status bar keeps the message on the left (truncated with `...` before
the info block; a message too long for the full block narrows the block by
110 px, scaling its text down, to give the message more room) and the GEOMETRY/CONSTR/POINTS filter toggles and zoom on
the right. Between them a two-line block shows the cursor's world
coordinates in display units (`X - Y -` outside the sketch viewport), the
current tool, the DOF label (also still in the header) and the snap state
(`SNAP GEOM+GRID`, `GEOM`, `GRID` or `OFF`, toggled with N and Shift+G).
The text is rebuilt only when one of those values changes (`StatusCache`);
it adds no repaints of its own.

Hovering a toolbar button or an open MORE row for 400 ms shows a one-line
tooltip with the command name and shortcut. Label and shortcut come
from the command registry (`app/commands.rs`). The event loop waits until the tooltip is due and repaints
once. Every toolbar label is preceded by a 16x16 pixel-art glyph painted as
rects (select arrow, rectangle, circle, line, ellipse, three planes, arc,
`...`); button geometry is unchanged.

### Settings

Ctrl+, (or MORE > SETTINGS, the 22nd MORE row) toggles the SETTINGS panel;
Esc or CLOSE closes it. Clicking a row cycles its value; every change is
applied live and saved to `settings.json` in `$PUPPYCAD_CONFIG_DIR`, else
`$XDG_CONFIG_HOME/puppycad`, else `~/.config/puppycad`:

```json
{ "units": "mm", "grid_mm": 1.0, "theme": "dark", "ui_scale": null,
  "window": { "width": 1280, "height": 800, "x": 0, "y": 0 } }
```

- `units` (mm, cm, in): Properties values, viewport dimension labels,
  numeric prompts and the status bar show this unit; the model and files
  stay in millimeters. A plain number typed into a length field is read in
  the display unit; dimension text gets the unit appended (`2` becomes
  `2in`) while expressions with their own units are kept as typed.
  Pattern prompts and status messages still say mm.
- `grid_mm` (0.5, 1, 2, 5, 10, 25.4): grid snap step. Grid lines are drawn
  on the multiple of the step nearest 25 mm.
- `theme` (dark, light): all colors come from a `ThemePalette` with `DARK` and
  `LIGHT` variants (`app/settings.rs`); the `const` colors are the `DARK`
  fields and the painter substitutes the active palette.
- `ui_scale` (`null` = monitor scale factor rounded to 1.0/1.5/2.0, or 1.0,
  1.5, 2.0): see below.
- `window`: logical size and position, saved when the window closes.

Unknown or invalid fields fall back to the defaults.

### UI scale

`Layout::scaled` divides the physical surface by the UI scale, so all fixed
layout coordinates are logical units and every rect grows linearly with the
scale. The pointer is divided by the scale on input; the painter maps the
logical canvas onto the surface and snaps font pixels to whole physical
pixels. The default window is 1280x800 logical, so it also scales with the
monitor. The UI smoke test runs with an empty `PUPPYCAD_CONFIG_DIR` and
`ui_scale` 1.0, so its click positions match the 1280x800 layout.

## Code layout

Library crate (`rust/src/lib.rs`):

- `model/` — the sketch `Document`: `mod.rs` (document state, snapping,
  selection, drafting, undo), `shapes.rs` (`Point`, `Shape` and per-shape
  geometry), `relations.rs` (fixed anchors and relation toggles),
  `symmetry.rs` (mirror geometry, symmetry relations, the Mirror tool),
  `offset.rs` (single-shape and mitered chain offsets, the Offset tool),
  `pattern.rs` (linked linear/grid and circular patterns),
  `corner.rs` (line-line fillet and chamfer, endpoint relation bookkeeping
  shared with split/trim), `tangent.rs` (tangent arc, smooth join gesture,
  endpoint tangents),
  `dimensions.rs` (`set_dimension`), `annotations.rs` (dimension objects:
  labels and hit-testing, point dimensions, reference dimensions,
  expressions and variables), `selection.rs` (multi-selection,
  `SelectionFilter`, box and toggle selection, `for_each_selected`),
  `edit.rs` (delete, split, trim, extend,
  copy, move, scale, rotate), `drag.rs` (whole-shape and single-anchor
  dragging), `solve.rs` (`build_problem`, `solve_once`, `analyze_constraints`),
  `geometry.rs` (curve intersections and boundaries), `glyphs.rs` (relation
  glyph layout, placement inference, automatic H/V), `tests.rs`,
  `dimension_tests.rs`, `glyph_tests.rs`.
- `project.rs` (`.pcad` load/save, parts, extrusion profiles and end
  conditions; dimension round-trip tests in `project/dimension_tests.rs`,
  extrude option tests in `project/extrude_option_tests.rs`),
  `project/features.rs` (feature tree order, suppression, rollback, error
  states and the per-feature evaluation cache; tests in
  `project/features_tests.rs`), `project/face.rs`
  (plane frames, web face descriptors, face lookup, face sketches and their
  support edges; tests in `project/face_tests.rs`), `project/features.rs`
  (revolve, loft and edge finish meshes, sharp-edge detection and the native
  extension nodes; tests in `project/feature_tests.rs`), `solver.rs`,
  `sketch_value.rs` (dimension expressions and sketch variables),
  `spline.rs`, `stl.rs` (body triangles, ASCII/binary STL, numbered
  writes), `threemf.rs` (3MF package). `lib.rs` also hosts `roundtrip` and
  `export` for the CLI.

Binary (`rust/src/main.rs` parses arguments and starts the event loop):

- `app/` — `mod.rs` (`App` state, layout, colors, painting entry point),
  `commands.rs` (command registry, key resolution and the actions behind
  keyboard/MORE/palette), `palette.rs` (Ctrl+K command palette),
  `shortcut_overlay.rs` (? / F1 cheat sheet),
  `input.rs` (mouse/keyboard handling; the `winit` handler forwards to
  `App::handle_window_event` and `App::handle_key`, which tests drive
  headlessly),
  `panels.rs` (toolbar and properties panels, dimension and variable
  editing),
  `corner.rs` (fillet/chamfer gestures and prompts), `tangent.rs` (tangent
  arc tool and smooth join key),
  `pattern.rs` (linear/circular pattern prompts and center pick),
  `project_panel.rs` (breadcrumbs and the projects page), `solid.rs` (3D
  preview projection, dragging, face and body picking and painting), `extrude.rs`
  (extrude prompt with ghost preview, end condition/flip/target edits and
  their undo), `feature_tree.rs` (feature rows, reorder/suppress/rollback
  keys and drag), `features.rs` (revolve axis pick and ANGLE prompt, loft
  picks, edge hover/pick and size prompt, extension feature Properties),
  `view_sketch.rs` (sketch grid
  and shape painting, dimension labels, hover highlight, relation glyphs and
  inference hints), `settings.rs` (settings file, units, theme palettes,
  SETTINGS panel, UI scale), `status.rs` (status bar fields, tooltips,
  toolbar glyphs), `tests.rs`, `input_tests.rs` (headless keyboard/mouse
  tests), `settings_tests.rs`, `status_tests.rs`.
- `renderer.rs` (CPU `Painter` that builds the triangle list, and the
  `wgpu` `Renderer` that uploads it; `Painter` tests in
  `renderer_tests.rs`), `view3d.rs` (orbit camera).

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
  A second web project with length (expression and label position),
  reference distance, X and Y distance relations and a sketch variable must
  come back with identical relations and variables. These parts are skipped,
  with a warning, when `cargo` is not installed. `rust/fixtures/dimensions.pcad`
  holds every dimension kind the native app writes, so both loaders check it.
  `rust/fixtures/face-sketch.pcad` has a pocket cut from a top-face sketch and
  a boss joined from a side-face sketch; the TypeScript test checks the web
  resolves the same face frames as `project/face_tests.rs`, and a web-built
  face sketch keeps its face target through `--roundtrip`.

## Isolated UI check

The UI smoke test uses a rootless Docker container, Xvfb, and software Vulkan.
It runs Rust unit tests, opens the native app in a virtual display, draws a
rectangle, center rectangle, both three-point rectangle modes, circle, line, ellipse, three-point arc, center arc, point, polygon, and slot with simulated clicks, edits dimensions, polygon sides, and slot width,
copies, moves, scales, and rotates selected shapes, switches units to inches and the theme to light in the SETTINGS panel, adds Front and Right sketches and switches between them, saves a v4 project, opens a single-extrusion fixture and toggles its view, then authors and saves an extrusion. It writes
screenshots and a project file to `workdir/`. It also extrudes a concave line
loop, line-drawn through-hole, slot, arc-and-line profile, and rectangle with two circular through-holes,
and exports their STLs (ASCII via Ctrl+Shift+E so facet counts can be grepped).
The existing-solid scenario also checks a binary STL (header and 684-byte size)
and a 3MF (ZIP parts, 12 triangles, millimeter unit). The authored extrusion scenario also checks a separate
open line and point beside its closed profile. A web-compatible fixture checks
an ellipse with 16 segments and a circular hole with 12 segments, plus a slot
with 12 segments per end cap. Another scenario extrudes both disconnected
regions from one sketch and checks their saved profile IDs.
A mirror scenario draws a line and a circle, mirrors the circle across the line
with Shift+I (`rust-native-mirror.png`: both circle rows show `[SYM]`), and
checks that the saved project holds a web `mirror` relation.
A pattern scenario box-selects a circle and makes a 3x1 linear pattern with
Shift+Z (`rust-native-pattern-linear.png`), then rotates a line six times about
a sketch point with ; (`rust-native-pattern-circular.png`: a six-spoke
star around the point, every row `[PAT]`), and checks the saved
`linearPattern`, `circularPattern` and `centerAnchor` fields. The
`rust/fixtures/patterns.pcad` fixture (a 3x2 circle grid and a four-way line
pattern about a referenced point) is opened by both the Rust round-trip test
and `test/rust-roundtrip.test.ts`.
A palette scenario draws a line, presses Ctrl+K and types `mirr`
(`rust-native-palette.png`: Mirror highlighted first), presses F1
(`rust-native-shortcuts.png`: the three-column cheat sheet), then runs
Circle from the palette with `circ` + Enter and checks the saved circle.
It also creates and renames a second part, switches between parts, renames a
sketch, and saves geometry in both parts. A two-body scenario creates extrusions
on Top and Front sketches, saves their graph, and exports both STLs.
It also clicks each extrusion row and edits the selected body's depth.
An axis-constraint scenario snaps one line horizontally and vertically, then
undoes the second constraint before saving.
A dimension scenario adds a point and a line, uses the D tool to add a
point-to-line-start distance and types 150 into its label
(`rust-native-dimension-edit.png`), adds the sketch variable `w = 40 mm` in
Properties, adds an absolute X dimension with Shift+origin driven by
`#w*-2`, and saves; the file must contain the `distance` relations, the X
axis, the expression and the variable (`rust-native-dimensions.png`).
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
A fillet scenario draws two lines meeting at a corner, fillets it with Shift+W
(`rust-native-fillet.png`), chamfers a second corner with Shift+H in
distance-angle mode (`rust-native-chamfer.png`), and checks the saved
`endpointTangent` and `chamfer` relations.
An offset scenario draws an L of two lines, offsets the chain by 20 mm with
Shift+U (`rust-native-offset.png`), and checks the saved `offsetChain`
relation.
A tangent arc scenario draws a line, presses `,`, clicks the line's end and
captures the live preview (`rust-native-tangent-arc-preview.png`), places the
arc, then smooth-joins a second line to the first line's start with `.`
(`rust-native-tangent-arc.png`) and checks the two saved `smoothJoin`
relations.
A face sketch scenario extrudes a rectangle, hovers its top face
(`rust-native-face-hover.png`), presses 4, draws a circle over the projected
block outline (`rust-native-face-sketch.png`), cuts it with Shift+X
(`rust-native-face-cut.png`) and checks the saved `face` node and cut.
A viewport feedback scenario draws two lines, makes them parallel with
Shift+P, hovers one in Select mode (`rust-native-glyphs-hover.png`: hover
highlight plus `//` glyphs on both lines), places a near-horizontal line
(`rust-native-glyphs-hint.png`: dashed guide and `H` hint), commits it
(`rust-native-glyphs-auto.png`: the line is horizontal with an `H` glyph) and
checks the saved `parallel` and `horizontal` relations.
A feature tree scenario opens `fixtures/feature-tree.pcad`, selects the
suppressed cut (`rust-native-tree-suppressed.png`), unsuppresses it with
Ctrl+H (`rust-native-tree-unsuppressed.png`), moves the join above the cut
with Ctrl+Up (`rust-native-tree-reordered.png`), moves the rollback bar with
Ctrl+[ (`rust-native-tree-rollback.png`), drags the ROLLBACK row above the
join (`rust-native-tree-rollback-drag.png`) and checks the saved
`rollbackIndex`.
A revolve scenario draws a rectangle and a vertical line beside it, presses
8 (`rust-native-revolve-axis.png`), clicks the line as the axis
(`rust-native-revolve-preview.png`) and commits a 360° ring
(`rust-native-revolve.png`). An edge chamfer scenario extrudes a block,
presses 0, hovers a top edge (`rust-native-edge-chamfer-hover.png`), clicks
it, types 10 and commits (`rust-native-edge-chamfer.png`); both check the
saved `revolve` / `edgeFinish` nodes.
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
