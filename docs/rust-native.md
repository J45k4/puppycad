# Native Rust app

The native rewrite lives in [`rust/`](../rust/): a `winit` window with a
custom `wgpu` renderer. Package and executable are named `puppycad`. The web
app remains the parity reference ([sketch-parity.md](sketch-parity.md)); the
plan is in [rust-roadmap.md](rust-roadmap.md).

## Overview and running

```sh
cd rust
cargo run                         # empty untitled sketch
cargo run -- my-project.pcad      # open a v4 project, or start a new one there
```

| Invocation | Effect |
| --- | --- |
| `puppycad [PROJECT.pcad]` | Open the editor. |
| `puppycad --roundtrip IN.pcad OUT.pcad` | Load and write back exactly as Ctrl+S would (no revision bump, `IN` untouched); non-zero exit on any error. |
| `puppycad --export IN.pcad OUT.stl` / `OUT.3mf` | Export the active part bodies: binary STL (`OUT-2.stl`… per extra body) or one 3MF. Prints the paths, overwrites, exits non-zero when the input is missing, has no solid, or the extension is unknown. Assemblies are not exported. |
| `puppycad --print-shortcuts` | Print the [Shortcuts](#shortcuts) tables as Markdown. |
| `puppycad --help` | Usage. |

`PUPPYCAD_CONFIG_DIR` sets where `settings.json` lives (else
`$XDG_CONFIG_HOME/puppycad`, else `~/.config/puppycad`).

## Interface

**Layout.** Header with breadcrumbs, toolbar, Project panel (left), viewport,
Properties (right) and status bar. Default window 1280×800 logical pixels.

**Pixel font.** The 8×8 ASCII pixel font is an intentional visual direction
for the native interface (hence `//`, `T` and `TAN` glyphs instead of
Unicode symbols).

**Header.** Breadcrumbs Projects / project (file name) / part / sketch /
selected shape; in the 3D view the trail ends at the selected feature. The
project crumb clears the selection, the part crumb opens the 3D view (or
clears the selection when the part has no solid), the sketch crumb returns
to the sketch; the last crumb is not a link. The right edge shows the DOF
state in the sketch and **PERSPECTIVE / ORTHO** in the 3D view.

**Projects page** (click **Projects**). Lists `..`, subfolders and `.pcad`
files (part count, revision) of the open project's folder, or the working
directory. Click or Up/Down + Enter opens, Backspace goes up, PgUp/PgDn and
the wheel scroll, Esc returns. With unsaved changes the first open only
warns: Ctrl+S saves, opening again discards.

**Project panel.** Part rows (click to switch), the active part's sketches,
solid features plus a ROLLBACK row, an `ASM` row per assembly, and the active
sketch's entities with relation tags (`[H]`, `[//]`, `[SYM]`, `[PAT]`, `[!]`,
…). Long rows are clipped with `...`. The wheel scrolls one row per notch;
when a key changes the selected feature or active sketch its row scrolls into
view. Shift+click toggles a shape in the selection.

**Properties.** Fields of the primary selection (with a count for
multi-selection), click-to-edit values (Enter applies, Esc cancels), the
Move / Scale / Rotate tabs, prompts of the active tool, and **SKETCH
VARIABLES** when nothing is selected.

**Quick actions (NEXT).** A band at the bottom of the Properties column
lists the most likely next commands for the current state
(`app/quick_actions.rs`): label, shortcut chip and a dimmed reason. Click a
row or press **Ctrl+1..7** (registered commands "Run quick action n") to run
row n. The band has a fixed height: seven rows when the column is at least
760 px tall, otherwise four (the default 1280x800 window). Properties ends
above it instead of being covered: its lower part scrolls (see
*Properties scrolling* below). Every row is a registry command whose
availability predicate passes right now; the list is rebuilt only when the
sketch revision, feature count, view, drafts, selection, hover target (kind
and id: sketch shape, 3D face / edge, not every pointer move) or assembly
state change. Sketch closed-profile detection is cached per revision and
shared with the viewport paint. **Ctrl+Shift+N** toggles the band; the
choice is saved as `quick_actions` in `settings.json`. Labels use " - "
(the pixel font has no em dash). Rules, in priority order:

- read-only project: **Open project** (reason `Read only: <why>`), then the
  rules below;
- assembly: Insert part, Mate, Export STL, Close assembly; instance selected:
  Mate, Fasten to world, Delete instance;
- extrude / revolve / edge finish prompt open: **Commit** (Enter), Flip (F),
  End condition (E), Cancel (Esc); Flip and End condition only for extrude;
- 3D, edges selected: Chamfer, Fillet ("n edges selected"), Measure; face
  selected: Sketch on face, Measure, Project into sketch; open face sketch
  with a profile: Extrude face sketch; nothing selected: Chamfer when an
  edge is hovered ("1 edge hovered"), Sketch on face (reason
  "Sketch on face - <face name>" over a face, else "hover a face first"),
  New sketch on TOP, **Edit depth** / **Edit value** (command "Edit feature
  value": opens the Properties prompt for the selected feature's depth, or
  a revolve angle / edge finish size), **Reorder feature** (two or more
  features; Ctrl+Up / Ctrl+Down, the row runs Ctrl+Up), Suppress, Roll back
  here, Export STL, Assembly view, Back to sketch;
- sketch extruded (locked): **Finish sketch**, Tab to 3D, New sketch;
- dimension selected: **Edit dimension value** (opens its value prompt),
  Make reference / driving, Delete dimension, Deselect;
- selected line + hovered line: Parallel, Perpendicular, Equal;
- empty sketch: Draw rectangle / circle / line, New sketch on another plane;
- conflicting constraints: "Fix conflict - see red rows" (Undo) first;
- closed profile: **Finish sketch**, Extrude, Add dimension ("n DOF
  remaining" while under-constrained), Revolve;
- open geometry only: Line (continue or close), Add dimension, Fit, Finish
  sketch (Select tool with nothing selected, which is what Esc leaves);
- padding to at least three rows: Search commands, Keyboard shortcuts, Save.

**Properties scrolling.** The Properties rows down to 340 px below the
column top (name, dimensions, point / spline coordinates, angle / offset
fields) never move. Rows below that (variables past the fifth, Move / Copy,
Scale / Rotate) scroll inside the space above the NEXT band: the mouse wheel
over Properties scrolls them, starting to edit a field there scrolls it
into view, and selecting another entity scrolls back to the top. With NEXT
hidden Properties uses the whole column.

**Finish sketch** (Ctrl+Enter, sketch view): with a body in the part it
opens the 3D view; with a closed profile and no body yet it opens the
extrude prompt; otherwise it stays and says "Sketch has no closed profile to
extrude - draw a closed shape or press X to extrude". **Ctrl+O** opens the
Projects page.

**Status bar.** Message on the left (truncated with `...`; long messages get
more room), GEOMETRY / CONSTR / POINTS selection filters, `AUTO` and `GLY`
toggles, a two-line block with cursor coordinates in display units, the
tool, DOF and snap state (`SNAP GEOM+GRID`, `GEOM`, `GRID`, `OFF`), and
zoom. It is rebuilt only when a field changes. Toolbar buttons have 16×16
pixel-art glyphs; hovering a button or MORE row for 400 ms shows a tooltip
with the command and shortcut.

**Settings** (Ctrl+, or MORE → SETTINGS). Clicking a row cycles it; changes
apply live and are written to `settings.json`:

```json
{ "units": "mm", "grid_mm": 1.0, "theme": "dark", "ui_scale": null,
  "quick_actions": true,
  "window": { "width": 1280, "height": 800, "x": 0, "y": 0 } }
```

- `units` (mm, cm, in): display unit for Properties, labels, prompts and the
  status bar; files stay in millimeters. Plain numbers typed into length
  fields use the display unit. Pattern prompts still say mm.
- `grid_mm` (0.5, 1, 2, 5, 10, 25.4): grid snap step.
- `theme` (dark, light): `ThemePalette::DARK` / `LIGHT`.
- `ui_scale` (`null` = monitor factor rounded to 1.0/1.5/2.0, or 1.0, 1.5,
  2.0): `Layout::scaled` divides the surface by it, so layout coordinates are
  logical units; font pixels snap to whole physical pixels.
- `quick_actions`: show the NEXT band (Ctrl+Shift+N toggles).
- `window`: logical size and position, saved on close.

Invalid fields fall back to the defaults.

**Command palette.** Ctrl+K or Ctrl+Shift+P. Filters by label, category and
shortcut text (prefix, word start, substring, subsequence: `mirr`,
`ctrl+s`). Up/Down, Enter runs, Esc or an outside click closes. Commands
that cannot run now are dimmed and skipped. All commands come from one
registry (`app/commands.rs`) shared by keys, toolbar, MORE menu, palette and
overlay.

**Shortcut overlay.** `?` or F1: a full-window cheat sheet by category,
balanced over as many columns as fit (three at 1280×800).

**Undo history.** Every step has a label ("Draw line", "Set length 40 mm",
"Extrude 50 mm", "Suppress newBody", …), shown as `Undo: <label>`. Sketch
snapshots and solid steps are separate stacks stamped by one global counter,
so Ctrl+Z undoes the newest step of either (switching views as needed) and
Ctrl+Y / Ctrl+Shift+Z redoes. Sketch undo restores the selection. Steps of a
locked sketch are skipped. **Ctrl+Shift+H** shows the HISTORY panel over the
Project panel: newest first, SKETCH/SOLID tags, `>` marks the position,
undone rows dimmed, a `Start` row at the bottom; click a row to undo or redo
to it. The assembly view has its own undo stack.

## Sketching

### Tools

| Category | Tool | Gesture |
| --- | --- | --- |
| Basic | Select `V`, Line `L`, Point `P` | Two clicks for a line, one for a point. |
| Rectangles | Corner `R`, center `Shift+R`, 3-point `T`, 3-point center `Shift+T` | Two corners; center + corner; edge + height; center + side midpoint + side. 3-point modes save rotated rectangles. |
| Circles | Center `C`, 3-point `Shift+C` | Center + rim; three rim points. |
| Arcs | 3-point `A`, center `Shift+A` (`W` flips sweep), tangent `,` | Start, end, point on arc; center, start, end direction; see [Tangent arc](#tangent-arc-and-smooth-join). |
| Ellipse | `E` | Center, first axis end, height point. |
| Polygon, slot | `G`, `S` | Center + vertex (6 sides, edit 3–64); two cap centers + wall point. |
| Splines | MORE → FIT SPLINE / CTRL SPLINE | Click points; Enter finishes, Shift+Enter closes a fit spline; control splines need 3n + 1 points. |

Esc cancels placement. All shapes save as editable web nodes (circle, arc,
rectangle and rotated-rectangle primitives, `sketchSpline` for both spline
modes, including periodic fit splines). **K** toggles construction (dimmed,
not part of profiles, kept by copies). **F** fits the sketch to the view;
middle-drag pans, the wheel zooms about the pointer.

### Placement, snapping and inference

Placement snaps within 8 px to anchors, midpoints and finite intersections
of lines, circles, arcs, ellipses, elliptic arcs, splines and
rectangle/polygon/slot boundaries (projected entities included), the ends,
midpoints and crossings with sketch geometry of a face sketch's dashed
context edges, then to the origin or grid. A square marks
the target. **N** toggles geometry snapping, **Shift+G** grid snapping.
Snapping a new line or open spline endpoint onto a sketch point adds a
persistent coincidence; other snaps only place a coordinate.

While placing a segment (line, 3-point rectangle edge, arc chord, slot axis)
a hint with a dashed guide shows `H`/`V` (within 2°), `//`/`T` to a line
within 30 px, or `TAN`. Hints are visual; with automatic constraints on
(**7**, `AUTO`, default on) a line committed under an `H`/`V` hint snaps to
the axis and gets the relation in the same undo step.

### Projecting body edges

In a face sketch, **O** (MORE → USE EDGES) projects body geometry into the
sketch as ordinary entities: with nothing picked it takes every boundary
loop of the sketch face (outer boundary, holes, notches); edges and faces
selected in the 3D view before **4** created the sketch are projected
instead. While Use edges is armed (until Esc or a click elsewhere), a click
on a dashed context edge projects just that body edge. In the 3D view the
context menu of an edge or face offers **Project into sketch**: into the
open face sketch, or into a new sketch on the clicked face (with its loops).

Straight edges become lines, closed circular loops circles, circular runs
arcs, and any other curve a polyline of lines (no splines are fitted).
Every projected entity carries a native `projected` relation (see
[Files](#files)); the solver locks it, so it adds no degrees of freedom and
cannot be dragged, moved or rotated. Projected entities are profile
geometry by default; **O** with projected entities selected toggles them
between construction and geometry. Delete removes the entity and its
relation; every projection is one undo step.

Projections follow the body: they are re-projected when a sketch opens,
when returning from the 3D view, after undo and after depth, end-condition
or feature value edits (in every face sketch, the same rule that lets face
sketches follow their source). Face loops are matched to their entities by
kind and position, edges by their end points, else (after the body changed)
by the two face planes they lie between. When a source is gone the entities
stay as plain geometry and the status bar says so. A sketch only sees the
features before the first feature built from it, so its own cuts never feed
back into its projections.

In a sketch with projected geometry, drawn curves may branch from or cross
the projected outline: the bounded regions of that arrangement are the
profiles (a line across a projected face gives two profiles, a projected
hole stays a hole of the region around it). Plain sketches keep the web's
simple-loop rule.

### Selection

Click a shape or its row. In Select, drag from empty canvas: left-to-right
(blue) selects enclosed shapes, right-to-left (green) touched shapes; Shift
on release adds. Shift+click toggles. Esc or an empty click clears. The last
selected shape is the primary. The status-bar filters limit what clicks and
boxes pick (filtered shapes still render). Hover pre-highlights a shape and
its anchors. Move, copy, scale, rotate, delete, **K** and dragging act on the
whole selection as one undo step.

### Dragging and transforms

Left-drag a selected shape to move it. A selected shape shows squares on its
anchors (line ends, points, circle/arc centers, rectangle and
rotated-rectangle corners, ellipse axis points, polygon vertices, slot cap
centers; spline handles are circles). Dragging an anchor (7 px pick) moves
only that anchor as a strong soft target re-solved every frame with every
relation, so it slides along what they allow (a line end with a driving
length moves on a circle). Release commits, Esc cancels. Fixed anchors do
not drag. With several shapes selected, anchors are not dragged
individually.

Properties: Move X/Y with **Move selected** / **Copy selected** (Ctrl+D
copies with those values); Scale (positive factor) and Rotate (signed
degrees) about an editable center CX/CY, each with a copy variant. Copies
keep driving dimensions. Rotating an axis-aligned rectangle makes it a
rotated rectangle. Lines with H/V rotate only by quarter turns (odd turns
swap H and V). Scaling clears expressions of dimensions it changes; moved
label positions move with the shape.

Spline handles and standalone points have exact X/Y fields. **Delete**
removes the selection.

### Split, trim, extend

Select the shape, hover and press:

- **I** split: lines, arcs, elliptic arcs and open splines at the cursor;
  circles and ellipses at two rim points (two arcs; ellipse pieces get
  `sameEllipse`, `[SE]`); a closed fit spline opens there. Splines use exact
  cubic subdivision.
- **Q** trim the span between crossings (one crossing keeps the far side,
  none deletes the curve); circles become arcs, ellipses elliptic arcs,
  splines control splines.
- **U** extend a line, arc or elliptic arc to the nearest boundary.

Retained pieces keep construction, H/V, fixed ends, coincidences, smooth
joins and point dimensions at surviving anchors; driving lengths and sweeps
of the cut shape are dropped and a circle's diameter becomes the arc's
radius. Lines carrying Equal, and offset members, are refused. Extend drops
length and coincidence on the moved end and refuses fixed ends.

### Fillet and chamfer

Select a line, hover a second line meeting it at a corner and press
**Shift+W** (fillet) or **Shift+H** (chamfer), or pick both lines via MORE →
FILLET / CHAMFER. Fillet asks for a radius (empty = last, default 5 mm),
trims both lines and inserts an arc with `coincident` and `endpointTangent`
relations and a driving `radius`. Chamfer asks for two setbacks; **Tab**
switches to distance-angle. It saves the web `chamfer` relation, which the
solver keeps driving; Shift+H on a chamfer line with nothing hovered edits
it. Relations at the cut corner are removed. Line-line corners only, as on
the web.

### Offset

**Shift+U** (MORE → OFFSET) on a line, circle, arc, rectangle, rotated
rectangle, polygon or slot; type a signed distance (empty = last, default
5 mm). Positive goes left of a line and grows closed shapes. A line joined
end to end with others offsets the whole chain with mitered joins
(`offsetChain`); single shapes use `offset`. The distance stays driving
(**OFFSET … MM** in Properties) and the copy follows its source through the
solver. Rows show `[OFS]`. Collapsing, reversing or self-intersecting
results are rejected.

### Mirror and symmetry

**Shift+I** (MORE → MIRROR) mirrors the selection across the next clicked
line as a linked `mirror` copy (a corner rectangle's copy is a rotated
rectangle). **Shift+S** with a mirror line selected, then click two shapes:
same-kind shapes become whole-shape `mirror` symmetric, otherwise the
nearest anchors become `symmetric`. The same gesture removes it. Rows show
`[SYM]`.

### Patterns

**Shift+Z** (MORE → LIN PATTERN): COUNT, SPACING X/Y, ROWS and ROW SPACING
(`linearPattern`). **;** (MORE → CIRC PATTERN): COUNT and ANGLE (default
360), then a center click; a sketch point, line end or curve center becomes
a referenced `centerAnchor`. Full circles step angle/count, open sweeps
angle/(count − 1). 2–32 cells, at most 64 copies. Copies follow their sources
and are not independently editable; with a member selected the same key
edits the pattern. Deleting a source, copy or center removes the relation
and frees the copies. Rows show `[PAT]`.

### Tangent arc and smooth join

**,** (MORE → TANGENT ARC), click an endpoint of a line, arc or open spline,
then the far end: the arc continues the source tangent (live preview) and
saves a `smoothJoin`. **.** with one curve selected and another hovered
joins their nearest endpoints (G1, `[TJ]`); again removes it. Zero-tangent
spline ends are rejected.

## Constraints

**Solver** (`solver.rs`, structure of `typescript/src/sketch-solver.ts`). Parameters:
anchor coordinates, radii, ellipse axes and rotation, arc start and sweep,
shape variables of rectangles, rotated rectangles, polygons and slots
(`solver::ShapeVar`), and auxiliary curve parameters for point-on-spline and
Normal. Each relation adds residuals. Levenberg–Marquardt with numeric
Jacobians solves with soft "stay close" weights, then the relations alone.
Every edit (move, drag, dimension, transform, new relation) is apply → solve
→ commit as one undo step, or is rolled back. The edited geometry is held;
relation sources stay put before targets move. Curves keep size and angles
unless a relation may change them. Pattern copies are regenerated from their
sources after every solve instead of being solved.

**Relations.** Select the target, hover the source, press the key; the same
gesture removes it. All save as web sketch relations.

| Relation | Gesture | Web relation | Row tag |
| --- | --- | --- | --- |
| Horizontal / vertical | `H` / `J` on a line | `horizontal` / `vertical` | `[H]` `[V]` |
| Parallel / perpendicular | `Shift+P` / `Shift+N` | `parallel` / `perpendicular` | `[//]` `[T]` |
| Collinear | `Shift+L` | `collinear` | `[CL]` |
| Angle | `Shift+B`, value in Properties | `angle` | `[A]` |
| Equal (lengths, radii, ellipse axes) | `Shift+E` | `equal` | |
| Radial distance | `Shift+D` on two circles | `radiusDifference` | `[RD]` |
| Concentric | `Shift+O` | `concentric` | `[O]` |
| Tangent (circle/arc pairs; line to circle, arc, ellipse, elliptic arc) | `Shift+Q` | `tangent` | `[T]` |
| Internal tangent (circles/arcs) | `Shift+Y` | `internalTangent` | `[IT]` |
| Midpoint (point on line/arc/elliptic arc) | `Shift+M` | `midpoint` | `[MID]` |
| Point on curve (any finite curve, splines included) | `Shift+V` | `pointOnCurve` | `[POC]` |
| Normal (line end to a curve) | `Shift+F` | `normal` | `[NORM]` |
| Fix point, endpoint, center, axis point, corner, vertex, handle | `Shift+K` over the anchor | `fixed` | |
| Coincident | snap onto a sketch point | `coincident` | |
| Smooth join / endpoint tangent | `.`, tangent arc, fillet | `smoothJoin` / `endpointTangent` | `[TJ]` |
| Symmetry / mirror | `Shift+S` / `Shift+I` | `symmetric` / `mirror` | `[SYM]` |
| Shared ellipse | split an ellipse | `sameEllipse` | `[SE]` |
| Offset | `Shift+U` | `offset` / `offsetChain` | `[OFS]` |
| Pattern | `Shift+Z` / `;` | `linearPattern` / `circularPattern` | `[PAT]` |
| Chamfer | `Shift+H` | `chamfer` | |

Fixed anchors block edits, drags, transforms and dimension changes that
would move them. Web graph `sketchConstraint` nodes (length, width, height,
horizontal, vertical) are edited in place and move to sketch relations once
they gain a label position, reference flag or expression.

**DOF and diagnostics.** `Problem::analyze` ports `typescript/src/sketch-redundancy.ts`
(rank of the relation Jacobian by two-pass Gram–Schmidt). Free DOF: line 4,
point 2, circle 3, arc 5, ellipse 5, elliptic arc 7, spline 2 per handle,
corner rectangle 4, rotated rectangle 5, polygon 4, slot 5; pattern copies
add none. The header shows **DOF n** (green at 0), **OVER-CONSTRAINED**
(purple, redundant relation) or **CONFLICT** (red, unmet relation); rows get
`[R]`/`[!]` and matching colors, fully constrained shapes draw green. The
analysis is cached per document revision and never runs during a drag.

**Glyphs** (`model/glyphs.rs`). Each relation draws a glyph beside every
entity it involves: `H`/`V`, `//`, `T`, `COL`, `ANG`, `=`, `O`, `TAN`,
`MID`, `POC`, `NORM`, `TJ`, `SE`, `RD`, `SYM`, `OFS`, `PAT`, and `FIX` with
a lock square per fixed anchor. Glyphs stack outward along the entity normal
in its DOF tint. Hovering one highlights the relation's partners; clicking
selects its entity. **6** toggles glyphs (`GLY`).

## Dimensions

Dimensions are sketch objects (`model/annotations.rs`): per-shape fields,
line angles and point dimensions.

- **Canvas labels.** Every dimension is drawn in the viewport (lengths
  parallel to the line, widths/heights along edges, diameter and radius
  leaders, sweep inside the arc, angles at the vertex, point distances on
  aligned/horizontal/vertical dimension lines). In Select, click a label to
  edit inline or drag it; a moved label saves as web `labelPosition` and gets
  a leader. Clicking selects it: Properties shows kind, value and **MAKE
  REFERENCE / DRIVING**; Delete removes it.
- **Properties.** Click line length, rectangle/ellipse/elliptic-arc width or
  height, circle diameter, arc or polygon radius (`radius`), SWEEP
  (`arcSweep`, signed, ±360 excluding 0), ANGLE (`rotation`), polygon sides
  or slot width to edit.
- **D tool.** Click an entity to add its next missing driving dimension, or
  two anchors for an aligned distance (Shift on the second click: X, Ctrl:
  Y). Picking the origin second gives an absolute dimension from a fixed
  construction point created at the origin. Saved as web `distance` (`a`,
  `b`, `value`, `axis?`). No line-to-point distance (absent on the web).
- **Reference.** **R** toggles driving/reference. A reference shows its
  measured value in parentheses, adds no constraint and saves
  `reference: true`. Angles cannot be references.
- **Expressions.** Values accept `+ - * / ( )`, `pi`, units `mm cm m um in
  inch ft deg rad` and `#name` variables (`sketch_value.rs`, port of
  `typescript/src/sketch-value.ts`). Labels show `expression = value`; saved as
  `expression` beside the evaluated `value`.
- **Variables.** With nothing selected, **SKETCH VARIABLES** lists
  `name = expression` rows (empty deletes an unused one, renaming rewrites
  references) and **+ ADD VARIABLE**. A change re-evaluates all dependents in
  one undo step or rolls everything back. Saved in the sketch's `variables`.

## Solids

**Sketch planes.** **1 / 2 / 3** (or + TOP / FRONT / RIGHT) add a sketch on
the XY / XZ / YZ reference plane using the web frames. Click a sketch row to
switch; unsaved edits of every sketch and part stay in memory. **B** adds a
part, Ctrl+PgUp/PgDn switch parts, **F2** / **Shift+F2** rename part /
sketch.

**Face sketches.** In the 3D view a hovered planar face is outlined yellow
and named (**TOP FACE**); faces not belonging to an extrusion prism are grey.
**4** (MORE → SKETCH ON FACE) adds a sketch on it, saved like the web: a
graph `face` node `face-<extrudeId>-<faceId>` targeted by the sketch.
Frames follow `getExtrudedFaceDescriptors` and are recomputed on every
evaluation, so the sketch follows depth changes; symmetric and flipped
blind prisms are placed where the native evaluation puts them (the web
treats them as blind from the sketch plane). The body context comes from
the evaluated bodies the sketch sees (the features before the first one
built from it): every loop of the face, including holes and notches from
later cuts, is drawn dashed and snaps; the rest of the body's edges,
projected along the normal, are drawn dashed and dimmer. **5** toggles the
context; **F** frames it.

**Profiles.** Closed rectangles, rotated rectangles, circles, ellipses,
polygons, slots, closed fit splines and simple loops of lines and arcs
(concave, either direction) are profiles; nested closed shapes become holes
and islands inside holes become new profiles. Open geometry, points and
construction are ignored; branching, self-intersecting and touching
boundaries are rejected. With several regions, **[ / ]** choose one
(outlined cyan; saved as `profileId`). Web segment counts are preserved.

**Extrude.** **X** new body, **Shift+X** cut, **Shift+J** join (cuts/joins
apply to the nearest earlier new body). The 3D view opens with a translucent
ghost and a DEPTH prompt: **E** cycles the end condition, **F** flips, **O**
cycles the operation (new body / cut / join; cut and join need an earlier
body), Enter commits (empty = 50 mm blind), Esc cancels. End conditions: Blind; Symmetric
(total depth centred, no flip); Through all and Up to body (cut/join only,
target an earlier new body, **U** then click). A selected extrusion shows
DEPTH, END, FLIP and TARGET in Properties, each edit one undo step; edits
that fail to evaluate are rejected. Face-sketch extrusions run along the
face normal: bodies and joins grow outward, cuts go inward (a native rule;
flip inverts it). The extruded sketch locks; **Tab** returns to it to
extrude another profile. Delete removes the selected extrusion and unlocks
its sketch.

**Preview state.** While an extrude, revolve or edge finish prompt is open
nothing exists yet, and the UI says so: the status bar reads "Extrusion
preview - not created yet | Enter commit | Esc cancel" (Revolve / Chamfer /
Fillet likewise, plus the reason when the ghost cannot be built), the last
breadcrumb is "New extrusion (preview)", a "PREVIEW" tag in the ghost color
sits right of the DEPTH / ANGLE prompt, the ghost's sharp edges (same 20°
rule as bodies) are outlined in the ghost color, and the Project panel shows
a dimmed, inert "NEW EXTRUSION (preview)" row after the feature tree.
Feature tree keys and row clicks do nothing until the draft ends.
The draft's Properties block (NEW EXTRUSION / REVOLVE / CHAMFER / FILLET)
is mouse-editable: clicking DEPTH (ANGLE, DISTANCE, RADIUS) or the prompt
box focuses the prompt (caret shown), END cycles the end condition, FLIP
toggles, OP cycles new body / cut / join like **O**, the TARGET row (Up to
body) arms a body click, and **[ COMMIT ENTER ]** / **[ CANCEL ESC ]** end
the draft; each change rebuilds the ghost. The prompt reads e.g.
"DEPTH 50_ mm  BLIND  NEWBODY". For blind and symmetric extrusions,
left-dragging the ghost's end cap along the extrusion normal sets the depth
live (1 mm steps, Shift 0.1 mm); the draft stays open for Enter, and right
drag still orbits. Enter
creates the feature as one undo step ("Created … | Ctrl+Z to undo"); Esc
reports "Extrusion cancelled" and returns to the sketch.

**Revolve** (**8**, MORE → REVOLVE). With a profile selected, click a sketch
line as the axis or press **X** / **Y**; an ANGLE prompt (empty = 360°) with
ghost, **O** cycles new body / cut / join. 32 segments per full turn; the
profile may touch but not cross the axis.

**Loft** (**9**, MORE → LOFT). Pick a profile, open another sketch, pick its
profile and press **9** (new body) or **Shift+9** (MORE → LOFT JOIN). Outer
loops are resampled and aligned; holes, edge-on profiles and
self-intersecting results are rejected.

**Edge finishes** (**0** chamfer, **Shift+0** fillet, in the 3D view).
Hover highlights the nearest visible sharp edge (normals > 20° apart); click
edges to toggle, type DISTANCE / RADIUS (empty = 2 mm), Enter. Only straight
edges between planar faces; tessellated facets are grey and rejected.
Edges are re-identified by endpoints, then by plane pair, on each
evaluation.

**Feature tree.** Features (extrusions, revolves, lofts, edge finishes) are
rows in tree order plus ROLLBACK. Selecting one shows its value in
Properties (editable). **Ctrl+Up/Down** reorder (refused when a dependency
would come later or a cut would change body), **Ctrl+H** suppresses (`[S]`,
dim), **Ctrl+[ / ]** or dragging ROLLBACK moves the bar. Features with
missing inputs or empty results are `[!]` (click for the reason) and
skipped; the rest still evaluates. Results are cached per feature; sketch
edits re-evaluate when returning to the 3D view. Delete is refused while a
later feature depends on the feature.

**3D view.** **Tab** toggles sketch / 3D. Right-drag orbits about the
surface point under the cursor (free, can roll), middle-drag pans, the wheel
zooms (web sensitivities and default angle pitch π/5, yaw π/4).
**PERSPECTIVE / ORTHO** in the header (ortho default, reset per project).
**[ / ]** select bodies. Shading: per-face normals, key light from the upper
right plus ambient and a weak fill; sharp edges (> 20°) drawn dark with
hidden parts removed. Booleans use `manifold-rust`.

### Selecting vertices, edges and faces

The 3D view (and the assembly view) derives a topology from the evaluated
mesh (`app/topology.rs`): **faces** are triangles joined across smooth
(< 20°) edges, so a planar face or a whole cylinder side is one face;
**edges** are the sharp edges, straight runs kept whole and the facets of a
tessellated curve chained into one curved edge or closed loop; **vertices**
are the edge ends (a closed loop has none). Ids are stable for one evaluated
result and re-derived (cached per scene hash) when it changes.

Hover priority: a visible vertex within **6 px** (filled square), then a
visible edge within **4 px** (thick line), then the face under the ray
(outline; grey when it has no web face id). Hidden elements are never
picked. The status line reads `Vertex (x, y, z)`, `Edge L=… mm` or
`Face <name> A=… mm²`. Left-click selects, **Shift+click** adds or
removes, a click on empty space or **Esc** clears; the selection is drawn in
the accent color and survives orbit, pan and zoom. Properties lists the
selected elements (vertex coordinates; edge endpoints, length and angles to
X/Y/Z; face normal, area and web face id), with the count and total
length/area for several. With exactly two selected it also **measures**
(read-only): the distance vertex–vertex/edge/face, between parallel edges,
parallel planar faces or an edge parallel to a face, and the angle between
two edges, two faces (between normals) or an edge and a face.

The selection drives the gestures: **0 / Shift+0** start from the selected
straight edges and go straight to the size prompt (without a selection they
arm click-to-pick as before); **4** sketches on the selected face, else the
hovered one. **Delete** with only a topology selection removes nothing
("Select a feature row to delete"); clicking a feature row clears it.

**Context menu.** A right press and release within 4 px (no orbit) selects
the element under the cursor, keeping the selection when it already
contains it, and opens a menu at the cursor: face → **Sketch on face**,
Suppress / unsuppress, Delete feature (both act on the selected feature
row), Measure; edge → **Chamfer…**, **Fillet…**, Measure; vertex → Measure;
empty space → Fit all, Toggle perspective, Export STL, Export 3MF. Up/Down
move, Enter runs, Esc or a click elsewhere closes; unavailable items are
dimmed (Measure needs two selected elements). Every item is a registry
command, so it is also in the palette.

## Assemblies

Top-level `"type": "assembly"` items are web `Assembly` documents
(`typescript/src/contract.ts`): `instances` (`partId`, `transform` with translation,
Euler XYZ degrees, scale), `connectors`, `mates` (`fasten`, `revolute`,
`prismatic`, `planar`, `ball`) and `actuators` (servos). `actuators`,
`variables` and mate `params` round-trip verbatim.

**Solver** (`assembly/solver.rs`). Fasten mates are solved exactly like
`solveFixedAssembly` (`typescript/src/assembly-solver.ts`). Natively each fastened group
is a rigid body and damped Gauss–Newton moves free bodies until revolute
(shared origin and Z), slider (same orientation, origin on Z), planar
(parallel Z, coplanar) and ball (shared origin) mates hold; a revolute servo
fixes its angle (`homeDeg`). The first instance's group and world-fastened
groups are grounded. The web stores non-fasten mates without solving them.

**View.** **Ctrl+B** (MORE → ASSEMBLY, or an `ASM` row) opens the first
assembly, creating one if needed; Ctrl+B, Esc with nothing selected, a part
row or Ctrl+PgUp/PgDn leave. Instances are drawn with their part's evaluated
bodies in per-instance colors; orbit, pan and zoom as in the 3D view. Click
an instance or row: Properties shows name, part, editable X/Y/Z and RX/RY/RZ
and its mates. **I** then a digit inserts a part instance; **Delete** removes
an instance with its connectors, mates and servos. Mate: click a vertex,
edge or face of A (it becomes the selection; with several the last one
counts), hover a vertex, edge or face of B, **J**, then **1** fasten, **2** revolute, **3** slider,
**4** planar, **5** ball (connectors at the picked points, Z along the face
normals; a vertex uses the normal of an adjacent planar face, a straight
edge its midpoint and direction, a curved edge loop its centre and axis).
Picking uses the part view's vertex > edge > face priority. Right-click an
element for Mate… / Fasten to world / Delete instance / Measure. Edits re-solve and are undoable; conflicting edits are rolled
back. Ctrl+E / Ctrl+Shift+E write one STL per instance
(`<project>-<instance>.stl`), Ctrl+Alt+E one 3MF with an object per
instance.

Instances are drawn with the same dark sharp-edge lines as the part view;
edges are computed once per part and moved by each instance's transform. The
built scene is cached per assembly revision, so hovering only re-picks.

The Properties panel lists the selected instance's mates. Click a mate row to
select it; **Delete** then removes the mate, its servo and any connectors no
other mate uses. A revolute mate shows its servo angle in a DEG box at the
right of its row: click it, type degrees and press Enter to drive the joint
and re-solve (a mate without a servo shows `-- DEG` and gets one). Every
change is one undo step.

Press **J** with an instance selected and nothing under the cursor to fasten
it to the world (ground) where it currently sits. Hovering a sharp edge
highlights it; clicking an edge, or pressing **J** while one is hovered, uses
the edge as the mate axis (its midpoint and direction) instead of the face
normal, which is the natural pick for revolute and slider mates. The edge
direction is flipped if needed so the mate does not turn the part over.

CLI: `puppycad --export IN.pcad OUT.stl|OUT.3mf [--assembly NAME]` exports an
assembly when the project's selected document is one, or when `--assembly`
names one by name or id. Mates are solved first. STL writes one file per
instance, `<out-stem>-<instance>.stl`; 3MF writes one package with one object
per instance.

## Files

**Compatibility.** The native app reads and writes the web `.pcad` v4
format. A project opens editable when every sketch is on a reference plane
or an existing extrusion face, every entity and relation is supported, and
every solid feature resolves. Otherwise it opens read-only with the reason
shown, for example:

- unsupported relations, relation targets or graph constraint kinds
  (`"unsupported … relation"`), and `distance` relations with a `direction`;
- legacy sketch `dimensions` arrays;
- sketches on planes other than XY/XZ/YZ or on missing faces;
- web `chamfer` graph nodes (they reference topology `edge` nodes the native
  app cannot resolve), legacy `solidSteps`/`solids` that do not resolve to
  native features, and features that fail to evaluate;
- unknown extrusion end conditions, a non-boolean `flip`, a missing or later
  up-to-body target, through-all/up-to-body on a new body;
- assemblies with unknown mate types, dangling connectors or non-positive
  scale (only that assembly is read-only).

**Native extensions the web loader ignores.**

| Field | Meaning | Web behavior |
| --- | --- | --- |
| `extrude.endCondition` (`symmetric`, `through-all`, `up-to-body` + `targetId`), `extrude.flip` | Extrude options | Evaluated as blind with the stored depth. |
| `"suppressed": true` on feature nodes, `data.tree.rollbackIndex` | Suppression, rollback bar | All features active; `rollbackIndex` dropped on re-save. |
| `revolve`, `loft`, `edgeFinish` graph nodes (`native-…` ids) | Native features | Dropped when the graph is normalized. |
| Assembly instance `name` | Instance label | Dropped. |
| Sketch relation `{"type":"projected", "entityId", "sketchId", "sourceFaceId"}` or `{…, "sourceEdge": {"a", "b", "via", "normals"}}` | Projected entity (Use edges): the entity is an ordinary `sketchLine` / `sketchArc` / circle primitive; `sourceFaceId` names a web face whose loops it follows, `sourceEdge` a body edge by its ends, a middle point and its two face normals (zero for curved edges). A source that no longer resolves leaves a plain entity. | Ignored; the entities load as plain geometry. |

Feature order is the order of feature nodes in `data.cad.nodes`, mirrored in
`rootNodeIds` and `data.tree.orderedNodeIds`. Defaults are omitted, so plain
blind extrusions are byte-identical to web output.

**Saving.** Ctrl+S saves every edited part and sketch together, records the
active part in `selectedPath`, preserves other project documents, and
refuses when the file changed on disk or the project is read-only. The
selected top-level part opens first.

**Export** (3D or assembly view). **Ctrl+E** binary STL per body beside the
project (80-byte header `PuppyCAD <part> / <body>`), **Ctrl+Shift+E** ASCII
STL, **Ctrl+Alt+E** one `name.3mf` (core 2015/02, millimeters, one object
per body). Existing files are kept: new files get numbered names
(`name-2.stl`). Exports use the evaluated Boolean bodies of active features
(respecting suppression and rollback). `--export` covers parts only.

## Shortcuts

Generated by `cargo run --manifest-path rust/Cargo.toml -- --print-shortcuts`
("editable": not in read-only sketches; "selection": needs a selected shape
or dimension). Context-dependent keys: **R** toggles a selected dimension
instead of drawing a rectangle, **E / F / U** edit the selected extrusion in
the 3D view, **[ / ]** cycle profiles in the sketch and solids in 3D, **I /
J / Delete** act on instances in the assembly view. In the 3D views
**right-click** (press and release without moving) opens the context menu
and **Shift+click** adds to the topology selection. A unit test rejects
duplicate bindings within a context.

### Draw

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
| `O` | Use edges: project the face loops / selection (again: construction) | sketch, editable |

### Edit

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
| `Ctrl+Z` | Undo | any view |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo | any view |
| `Esc` | Cancel / clear selection | any view |

### Constrain

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

### Dimension

| Shortcut | Command | When |
| --- | --- | --- |
| `D` | Dimension tool | sketch |
| `R` | Toggle driving / reference | dimension selected, selection |
| `Shift+B` | Angle to hovered line | sketch, editable, selection |
| `Shift+D` | Radial distance to hovered circle | sketch, editable, selection |
| palette / MORE | Edit dimension value | dimension selected, selection |

### Solid

| Shortcut | Command | When |
| --- | --- | --- |
| `X` | Extrude new body | any view |
| `Shift+J` | Extrude join | any view |
| `Shift+X` | Extrude cut | any view |
| `4` | Sketch on hovered face | any view |
| `5` | Toggle face body context | any view |
| palette / MORE | Project selected edges / faces into a face sketch | 3D preview |
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
| palette / MORE | Measure two selected elements | any view |
| palette / MORE | Delete selected feature | 3D preview |
| `Ctrl+Enter` | Finish sketch | sketch |
| palette / MORE | Edit feature value | 3D preview |
| `Enter` | Commit | extrude / revolve prompt open |
| `F` | Flip direction | extrude / revolve prompt open |
| `E` | Cycle end condition | extrude / revolve prompt open |
| `Esc` | Cancel | extrude / revolve prompt open |

### View

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
| `Ctrl+Shift+H` | Undo history panel | any view |
| `Ctrl+,` | Settings | any view |
| palette / MORE | Fit all (3D view) | any view |
| palette / MORE | Toggle perspective / orthographic | any view |
| `Ctrl+Shift+N` | Toggle quick actions | any view |
| `Ctrl+1` | Run quick action 1 | any view |
| `Ctrl+2` | Run quick action 2 | any view |
| `Ctrl+3` | Run quick action 3 | any view |
| `Ctrl+4` | Run quick action 4 | any view |
| `Ctrl+5` | Run quick action 5 | any view |
| `Ctrl+6` | Run quick action 6 | any view |
| `Ctrl+7` | Run quick action 7 | any view |

### Project

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

### Assembly

| Shortcut | Command | When |
| --- | --- | --- |
| `Ctrl+B` | Open / close assembly view | any view |
| `I` | Insert part instance | assembly view |
| `Delete` / `Backspace` | Delete instance / selected mate | assembly view |
| `J` | Mate selected face/edge to hovered face/edge (empty space: fasten to world) | assembly view |
| palette / MORE | Mate: hover a face/edge/vertex of another instance, then J | assembly view |
| palette / MORE | Fasten selected instance to world | assembly view |
| `Ctrl+Z` | Undo assembly edit | assembly view |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo assembly edit | assembly view |
| `Escape` | Clear selection / leave assembly | assembly view |

### File

| Shortcut | Command | When |
| --- | --- | --- |
| `Ctrl+S` | Save project | any view |
| `Ctrl+E` | Export binary STL | any view |
| `Ctrl+Shift+E` | Export ASCII STL | any view |
| `Ctrl+Alt+E` | Export 3MF | any view |
| `Ctrl+O` | Open project | any view |

## Code layout

Library crate (`rust/src/lib.rs`, also hosts `roundtrip` and `export`):

- `model/` — the sketch `Document`:
  - `mod.rs` state, snapping, drafting, undo; `history.rs` undo labels and
    stamps; `shapes.rs` `Point`, `Shape` and per-shape geometry
  - `relations.rs` fixed anchors and relation toggles; `symmetry.rs` mirror
    and symmetry; `offset.rs` offsets; `pattern.rs` patterns; `corner.rs`
    fillet/chamfer; `tangent.rs` tangent arc and smooth join
  - `dimensions.rs` `set_dimension`; `annotations.rs` dimension objects,
    references, expressions, variables
  - `selection.rs` multi-selection and filters; `edit.rs` delete, split,
    trim, extend, copy, move, scale, rotate; `drag.rs` shape and anchor drags
  - `solve.rs` problem building and analysis; `geometry.rs` intersections and
    boundaries; `glyphs.rs` glyph layout, inference, automatic H/V
- `solver.rs` least-squares solver and DOF analysis; `sketch_value.rs`
  expressions; `spline.rs` cubic splines; `stl.rs` STL writing; `threemf.rs`
  3MF package
- `model/projection.rs` projected entities (Use edges) and polyline
  recognition
- `project.rs` `.pcad` load/save, parts, profiles, extrusions;
  `project/face.rs` plane frames, face descriptors, face sketches;
  `project/context.rs` face sketch body context; `project/regions.rs`
  arrangement profiles;
  `project/features.rs` feature tree, evaluation cache, revolve, loft, edge
  finishes; `project/assemblies.rs` assembly items
- `assembly.rs` assembly documents and poses; `assembly/solver.rs` mate
  solver

Binary (`rust/src/main.rs` parses arguments and runs the event loop):

- `app/mod.rs` `App` state, layout, painting; `input.rs` mouse/keyboard
  (`App::handle_window_event`, `App::handle_key`); `commands.rs` command
  registry; `palette.rs`; `quick_actions.rs` NEXT band and Finish sketch; `shortcut_overlay.rs`; `history.rs` cross-view
  undo and HISTORY panel
- `app/panels.rs` toolbar and Properties; `project_panel.rs` breadcrumbs and
  projects page; `status.rs` status bar, tooltips, toolbar glyphs;
  `settings.rs` settings, units, themes, UI scale
- `app/view_sketch.rs` sketch painting, labels, glyphs, hints; `corner.rs`,
  `tangent.rs`, `pattern.rs` tool prompts
- `app/topology.rs` vertices/edges/faces, picking and measurement; `topology_view.rs` hover, selection, Properties; `context_menu.rs` right-click menu; `projection.rs` Use edges, re-projection.
- `app/solid.rs` 3D projection, picking, painting; `extrude.rs` extrude
  prompt and edits; `feature_tree.rs` feature rows; `features.rs` revolve,
  loft and edge finish interaction; `assembly.rs` assembly view
- `renderer.rs` CPU `Painter` and `wgpu` `Renderer`; `view3d.rs` orbit
  camera; `shader.wgsl`

Tests: `model/tests.rs`, `model/dimension_tests.rs`, `model/glyph_tests.rs`,
`project/dimension_tests.rs`, `project/extrude_option_tests.rs`,
`project/face_tests.rs`, `project/feature_tests.rs`,
`project/features_tests.rs`, `assembly/tests.rs`, `app/tests.rs`,
`app/input_tests.rs`, `app/history_tests.rs`, `app/settings_tests.rs`,
`app/status_tests.rs`, `app/assembly_tests.rs`, `app/projection_tests.rs`,
`renderer_tests.rs`;
integration tests `rust/tests/roundtrip.rs` and `rust/tests/export.rs`.

## Testing

**Unit tests.** `cd rust && cargo test --workspace` (the app tests drive
`App` headlessly). CI (`.github/workflows/rust.yml`, pushes and PRs to
`master` and `rust`) runs `cargo fmt --check`, build, test and
`cargo clippy --workspace --all-targets -- -D warnings`, then the UI smoke
job, uploading screenshots as the `rust-ui-smoke` artifact.

**Round trip.** `rust/tests/roundtrip.rs` loads every `rust/fixtures/*.pcad`,
saves, reloads and saves again; both saves must match.
`typescript/test/rust-roundtrip.test.ts` (run `bun run test` inside `typescript/`) loads every fixture in the
TypeScript loader, passes web-built projects (geometry, dimensions with
expressions and variables, face sketches) through `--roundtrip` and checks
they come back identical; it checks web face frames against
`project/face_tests.rs` and the assembly fixture's solved transforms against
`assembly/tests.rs` (1e-3). `rust/tests/export.rs` and
`typescript/test/rust-export.test.ts` cover `--export`. The cargo-dependent parts are
skipped with a warning when `cargo` is missing.

**UI smoke.** A rootless Docker container with Xvfb and software Vulkan
runs the unit tests, then drives the app with simulated input
(`rust/ui-smoke.sh`), checking saved files and exports and writing
`rust-native-*.png` screenshots to `workdir/`. Settings come from an empty
`PUPPYCAD_CONFIG_DIR` at UI scale 1.0, so clicks match the 1280×800 layout.

```sh
docker --context rootless build -f rust/Dockerfile.ui-test -t puppycad-ui-test rust
docker --context rootless run --rm -v "$PWD/workdir:/out" puppycad-ui-test
# projection-only check (rust-projection-*.png):
docker --context rootless run --rm -v "$PWD/workdir:/out" \
  -v "$PWD/rust/ui-projection-smoke.sh:/app/ui-projection-smoke.sh:ro" \
  puppycad-ui-test sh /app/ui-projection-smoke.sh
```

| Area | Scenarios | Screenshots (`rust-native-…`) |
| --- | --- | --- |
| Drawing | all tools, 3-point circle, center arc, rectangles, splines, fit view | `container`, `three-point-circle`, `center-arc`, `center-rectangle`, `three-point-rectangles`, `author-splines`, `splines`, `fit-before/after`, `tools-menu` |
| Editing | copy, move, scale, rotate, construction, box select, anchor drag | `copy`, `scale`, `rotate`, `construction`, `construction-dimmed`, `box-select*`, `anchor-drag` |
| Split / trim / extend | lines, arcs, circles, ellipses, splines | `split-*`, `trim-*`, `extend-*`, `*-trim-spline` |
| Snapping | anchor, intersections, linked move | `snap-target`, `intersection-snap`, `*-intersection`, `snap-linked-move` |
| Spline edits | handle drag, coordinates, fix | `spline-handle`, `spline-coordinates`, `fixed-spline`, `point-coordinates` |
| Relations | fixed anchors, Equal, Concentric, Parallel/Perp/Collinear, Angle, radial distance, tangents, Midpoint, POC, Normal, H/V, graph constraints | `fixed-*`, `equal-*`, `concentric`, `line-angle`, `radius-difference`, `internal-tangent`, `external-tangent`, `line-*-tangent`, `midpoint`, `point-on-curve`, `normal`, `horizontal-line`, `vertical-line`, `graph-constraint-*`, `dof-header` |
| Dimensions | Properties edits, D tool, variables, radius, sweep/rotation | `dimension-edit`, `dimensions`, `dimension-copy`, `curve-radius`, `angular-dimensions` |
| Sketch tools | mirror, fillet/chamfer, offset, patterns, tangent arc | `mirror`, `fillet`, `chamfer`, `offset`, `pattern-*`, `tangent-arc`, `tangent-arc-preview` |
| Feedback / UI | glyphs and hints, palette, cheat sheet, history, settings | `glyphs-hover/hint/auto`, `palette`, `shortcuts`, `history`, `settings-*` |
| Parts and files | sketches per plane, parts, rename, save failure | `two-parts`, `part-switch`, `part-list`, `created-part`, `renamed-part`, `save-failed` |
| Solids | extrude profiles, holes, two bodies, cut, join, depth, delete, STL/3MF, orbit, options | `solid*`, `*-solid`, `two-bodies`, `two-profiles-two-bodies`, `profile-selection`, `cut-extrusion`, `join-extrusion`, `deleted-solid`, `stl-export`, `extrude-preview`, `extrude-symmetric*` |
| Face sketches | hover, sketch, cut | `face-hover`, `face-sketch`, `face-cut` |
| Use edges | extrude-options top face: context, projected loops, split line, cut | `use-edges-context`, `use-edges-projected`, `use-edges-split`, `use-edges-cut` |
| Feature tree | suppress, reorder, rollback | `tree-*` |
| Revolve / loft / finish | revolve, edge chamfer, loft join | `revolve*`, `edge-chamfer*`, `loft*` |
| Topology | vertex hover, vertex–edge measure, two-edge chamfer, right-click → sketch on face | `topology-vertex-hover`, `topology-measure`, `topology-edges`, `topology-chamfer`, `topology-context-menu`, `topology-face-sketch` |
| Assembly | open, select, export, delete, mate row, servo edit | `assembly`, `assembly-selected`, `assembly-deleted`, `assembly-mate-selected`, `assembly-servo-60` |

## Known gaps

- STEP export.
- Assemblies: mate kinds cannot be changed and mates cannot be renamed after
  creation; servos only on revolute mates; no mate limits; instance scale is
  ignored for edge directions and connector axes.
- The web app does not read the native extensions (extrude options,
  suppression, rollback, revolve, loft, edge finish, instance names) and
  does not solve non-fasten mates.
- Web `chamfer` graph nodes (topology edges) open read-only.
- Pattern copies are regenerated after each solve, not solved; skipped
  instances and more than 64 copies are unsupported.
- Snaps infer only coincidence with sketch points; midpoint and
  intersection snaps add no relation (the web infers both).
- Lines carrying Equal, and offset members, cannot be split, trimmed or
  extended; H/V lines rotate only by quarter turns.
- No diameter-circle tool, conics or text.
- Fillet and chamfer are line-line only; no ellipse, elliptic-arc or spline
  offset, mixed line/arc chains or rounded joins (all open on the web too).
- Splines: no closed control splines, curvature or handle-continuity
  constraints; midpoint on a spline is rejected (as on the web).
- Edge finishes accept only straight edges between planar faces; curved
  edges can be selected and measured but not finished.
- Topology faces merge across tangent (smooth) edges, so a face blended
  into a tangent fillet is one face; non-parallel edge–edge and
  curved-face distances are not measured. The context menu has no "Edit
  feature", "Extrude from face sketch" or "Hide body" items yet.
- Use edges projects only into face sketches; other curves become line
  polylines (no fitted splines); projections are re-linked after body
  changes only on the listed events, and a sketch's stored geometry is
  stale until then (it is never re-projected while saving).
- Faces of cut features cannot carry sketches; face-sketch cuts run inward
  by a native rule the web graph cannot express.
- Pattern prompts ignore the display unit.
