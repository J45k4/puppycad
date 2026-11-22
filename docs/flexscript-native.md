# Native Flexscript editor

The editor opens a single `.pcad` file in PuppyCAD v4 or Biscuit standalone schema version 1 per window on Linux.
Use `./run.sh drawing.pcad` to build and open a file, or `./run.sh` to open
`untitled.pcad` in the current directory. Save writes back to that path.
If a v4 drawing contains several top-level parts or assemblies, **Next item** or F6
switches between them inside the same window. Save remembers the selection.

**New part** (Shift+N) adds a part with a blank XY sketch to the open file.
**New assembly** (Shift+A) adds an empty assembly to that file. In an
assembly, click **Part** or press P to choose a part from the drawing.
**Insert** or I places an instance. Repeated insertions reference the same
part and receive distinct instance IDs. Undo removes one insertion at a time.

## Workspaces

- **Assembly** shows occurrences and mates. Select an assembly in the Design
  tree and choose **Open assembly**. Use **Insert** to place parts and
  **Mate** to constrain how they move.
- **Part** shows the solid and its features. Select a part in the tree and
  choose **Open part**. Use **Sketch** in the toolbar to open its current
  sketch, or select a specific sketch under that part.
- **Sketch** provides the 2D drawing and dimension tools. Choose **Done
  sketch** or click the underlined **Part** breadcrumb to return to the
  owning part, even if the sketch has no solid.
  F12 switches between Part and Sketch. Extrude and Revolve create a feature
  and return to Part.

From Part, click **New sketch** or press Shift+S. Choose **Top (XY)**,
**Front (XZ)**, or **Right (YZ)**. Choose **On face** to attach the sketch
to an existing solid, then click its face. Keys 1, 2, and 3 select a plane
while the chooser is open. Escape cancels.

Save writes all of the items and their sketches to the same PCAD file.
The curved arrows in the toolbar undo and redo edits. A dim arrow marks
an unavailable action. Ctrl+Z and Ctrl+Y are available from the keyboard.

The shared Flexscript CAD library evaluates the selected part or assembly.
The native editor draws the resulting
triangles through Flexscript's OpenGL renderer and uses its native font stack
for the toolbar.

## Design tree

The **Design** sidebar opens with the file, including a new empty sketch.
The default part appears as **Part 1** when its name comes from the filename.
Click a disclosure triangle to expand a part, sketch or group. Search by
name, type or ID to reveal matching records and their parents. Click a
part, assembly or sketch row to open its workspace and update the
breadcrumb. Other rows show their ID and available action.
Right-click a sketch to choose
**Edit sketch**, or right-click another row to see its available actions.
Press Escape to dismiss the menu. With the tree focused, Up and Down
select rows, Left and Right navigate branches, and Enter opens the selected
action. Collapse the panel with **‹** or
F3 and reopen it with **Design ›** or F3.

In a v4 part, expand a sketch to select its geometry or inspect its
constraints and dimensions. **Edit sketch** opens that sketch in its
owning part. Click the eye beside a sketch, or use **Show sketch** and
**Hide sketch** in its context menu, to control whether it appears in
Part mode. Unused sketches appear by default. Sketches used by an active
feature start hidden. Visibility changes support Undo and are saved in
the drawing. **Fit** includes visible sketches, even in a part with no solid.

Select a solid feature and choose **Feature history** to
inspect its rebuild state. **Open item** switches directly to another
part or assembly. In an assembly, selecting an occurrence highlights it.

In a standalone Biscuit file, expand **Parts**, **Features**, **Assembly**,
**Drawing sheets** or **Electrical** to inspect the saved design. Select
a sheet and choose **Open sheet**. The viewport shows the projected edges,
dimensions and callouts for the current model. Open another sheet from
the tree, or press **Escape** to return to the model. **Export PDF**
(Ctrl+P) writes the complete set beside the PCAD file. A repeated export
uses the next available name. The CLI also accepts
`--export INPUT.pcad OUTPUT.pdf`.

Expand **Drawing sheets** and select **New sheet** to create a top view.
Open **Settings** (F2) on a sheet. Use Left and Right to choose a field
and Enter to edit it. You can change the sheet name, orientation, scale,
occurrence, edge visibility, and section depth. Set a scale denominator
of 0 for automatic fit, or clear the section depth to restore a projection.
On a detail sheet, you can change its name and magnification. Applied
settings update the preview and support Undo and Redo. Press Ctrl+S or
click **Save** to write them to the PCAD file.

## Finding a part in the drawing

Click **Objects** or press T in the solid viewport. Search for a body name, an
occurrence path or a driving field. Click an occurrence to highlight it and
inspect its full ID and world-space dimensions. Expand a subassembly to find
individual copies, or type a path such as `right/upper` to find one directly.
Selecting a driving field makes it the target of **Set value**, **+** and
**−**. For a v4 part, search for a feature by name or filter by
`depth`, `angle`, `width` or `height`. Rectangular profiles expose width and height
alongside their extrusion depth or revolve angle.

Press F2 or click **Set value** to edit the selected field. Lengths use
millimetres. Revolve and joint angles use degrees. Type a signed decimal with up to six fractional places.
Enter or **Apply** rebuilds the drawing and records one undo step. An invalid
value stays in the input for correction. Escape or **Cancel** discards the
pending entry. Ctrl+A selects the value, Ctrl+C copies the selection, and
Ctrl+V pastes at the cursor.

In an assembly, choose a row under **Part definitions** to highlight its
placed copies. The inspector shows the occurrence name and full path.
Use the arrow buttons or Left and Right to choose a copy, then click
**Isolate** or press Enter. Double-click a definition with one placed copy
to isolate it immediately.

Double-click a visible occurrence, or select it in **Objects** and click
**Isolate**. The camera fits that occurrence. Picking, orbiting and hinge
dragging then use its surfaces. Click **Show all** or press F11 to restore
the assembly. Export includes every body in the active drawing or v4 item.
Isolation, search and camera commands stay out of the document history.

Click a labelled cube face for its standard view. **Views** lists all six
faces and isometric. The cube follows orbit and roll. Keys 1–6 select front,
right, top, back, left and bottom. Press 0 for isometric. In windows narrower
than 800 logical pixels, the arrows at the ends of the toolbar page through
the commands available for the current drawing.

## Biscuit standalone drawings

A `puppycorp.pcad` schema-version-1 file opens directly in orthographic view. Try
`tests/fixtures/sensor-unit.pcad`, copied from Biscuit's standalone
studio at `ea4670d`. Tab or **Next field** cycles through independent joint
values, literal parameters and editable feature dimensions. Use **+** and
**−** to change the selected value by one degree or millimetre, or F2 to
enter the value directly.

Click an occurrence to highlight it. Drag a hinged occurrence or its fastened
child to preview the angle. Release to accept it or press Escape to cancel.
Right-click a visible occurrence and choose **Fix** to pin its current world
transform. Shift+F10 or the Menu key opens the same menu for the selection.
Use Up, Down, Home or End to choose **Fix**, **Unfix** or **Fit view**.
Enter or Space applies the action. Escape or Tab dismisses the menu.
The toolbar and P also toggle the selected occurrence’s pin. **Unfix**
restores movement with its parent. Click the projection button or
press O to switch between orthographic and perspective views. Choosing a
standard view refits the standalone drawing after pan or zoom. Ctrl+S saves the
authored graph, including electrical and routing records. Ctrl+E exports
one STL per occurrence and Ctrl+Alt+E creates a named multi-object 3MF.
Click **STEP** or press Ctrl+Shift+E to export the current pose as a faceted
STEP assembly. Repeated occurrences refer to the same part definition.

The viewport uses the part colors stored in the drawing. In a routed
assembly, orange lines show individual wires, blue lines show shared
cable jackets and purple crosses mark positioned splices. Red identifies
routes with reported issues or unknown or interfering clearance. The routes
use the same camera and depth as the parts. A route attached to a moving
occurrence follows its hinge preview. Isolating one part hides the assembly
wiring. **Show all** restores it.

## Build and run

Build from the repository root with the Flexscript bootstrap compiler on PATH.
The build reads `flexscript-sdk.rev` and takes that revision from the sibling
`../flexscript` Git checkout. Use `FLEXSCRIPT_DIR` for a different checkout.
For development against local CAD changes, set `FLEXSCRIPT_LIB` to the SDK's
`libs` directory. `FLEX_COMPILER` can select a prebuilt compiler.

```sh
./build.sh
cp rust/fixtures/extruded-rectangle.pcad workdir/flexscript/block.pcad
workdir/flexscript/puppycad workdir/flexscript/block.pcad
```

To start a part, launch the editor with an unused project path, for example
`workdir/flexscript/puppycad workdir/flexscript/new-part.pcad`. The new
part starts with an empty XY sketch. Choose **Rectangle** and click opposite
corners. To add a hole, choose **Circle** and click its centre and rim inside
the rectangle. Click **Extrude** or press X to open a 3D preview.
The default is a 10 mm blind extrusion. **Depth** (D) accepts a value
in millimetres. **End** (E) cycles Blind and Symmetric, with Through all
and Up to body available for cuts and joins. **Flip** (F) reverses
Blind or Up to body. **Operation** (O) cycles New body, Cut and Join
when an earlier body is available. For Up to body, click **Target** (U)
then click the earlier solid, or use Tab to cycle eligible bodies.
Apply (Enter) commits one undo step. Cancel (Escape) returns to the
sketch. Right-drag orbits the preview, middle-drag pans, and Home fits it.

Choose **Line** to draw a segment by clicking its two endpoints. Lines that
meet end to end can enclose a profile, including one with a concave outline.
In the sketch canvas, R, C and L select rectangle, circle and line. E
commits a quick 10 mm extrusion, P cycles closed
regions, and Tab selects another sketch in the part. The wheel changes the
sketch scale. Escape cancels an unfinished shape. Undo and redo work across
sketch creation and extrusion. The project file is created on its first save.

Press **F8** or choose **Dimensions** to edit a sketch made of lines, circles
and points. **Next sketch** cycles through the supported sketches in the part.
Click a line to select it, or use **Next line** (Tab). Its length and the
sketch's degrees of freedom appear above the viewport. **Set length** (F2)
edits the driving dimension while holding the start point. Drag either
endpoint to preview the motion permitted by the sketch constraints. The
opposite endpoint stays in place. Escape cancels a drag, and releasing
the pointer accepts it as one undo step. **Done** returns to the previous
view. Ctrl+S saves the drawing.

Open **Patterns** (F5) in a sketch. Click source geometry to select it,
then choose **Grid** (G) or **Circular** (C). Click a member to select an
existing pattern, or press P to cycle through patterns. Tab selects the
next field and F2 edits it. Grid fields cover columns, rows and both step
vectors in millimetres. Circular fields cover count, angle and center.
Choose **Pick center** (R) and click a sketch anchor to follow it, or click
an empty position for a fixed center. Drag a blue grid handle to change its
spacing vector. Shift keeps the original direction. Drag the circular sweep
handle to change the angle, or hold Shift to snap to 15°. Dragging the center
moves its referenced sketch anchor. Escape during a drag cancels the preview.
Releasing accepts it as one undo step.

Choose **More tools → Offset** (F4) in a sketch, then click a source outline
and enter a signed distance. One line selects its connected chain up to
the next fork. Positive distances expand closed chains, circles, rectangles,
polygons and slots. For an open line, the positive side is to its left when
traversed from its start to its end. Click an existing copy with the same
tool to change its offset. In **Dimensions**, **New offset** (F4) uses the
selected line, circle or circular arc. Each creation is one undo step.

In a sketch with saved offsets, choose **Offsets** (O) in **Dimensions**.
The affected copies are highlighted together. Press **F2** to enter the
signed distance, or press **O** again to select the next offset. A line
chain keeps its mitered corners when the distance changes. An edit that
would collapse an edge or circle leaves the drawing at its previous size.

In **Dimensions**, choose **Mirror** (M), click a straight line as the axis,
then click the point, line or circle to copy. The new geometry stays linked.
Dragging either member moves its counterpart across the line. A dimension
on either mirrored circle drives both radii.

To reflect two existing anchors, choose **Mirror**, then **Pair anchors**
(P). Pick the axis, the anchor to keep, and its counterpart. Repeat the
same pair and axis to remove the relation. Escape cancels an unfinished
pick. Undo restores the geometry and its relation in one step.

Click a circle's rim to select its diameter. Tab switches between fields,
including the radius. The highlighted guide spans the diameter or runs
from the center to the rim. Press F2 to enter a length or a formula such
as `#bore + 6 mm`. **Set reference** (R) makes the selected field a
measurement. A driving diameter updates a reference radius when the sketch
variable changes. Delete removes the selected annotation. Circle centers
can also anchor distances or be dragged subject to their constraints.

For an ellipse, open **Dimensions** (F8) and click the rim to select
**Width**. Tab cycles through **Height** and **Rotation**. Both lengths
measure the full axis. Rotation uses degrees counterclockwise from sketch X.
F2 accepts a value or expression such as `2 * #span` or `-0.5 rad`.
On an elliptic arc, clicking the curve selects sweep first, followed by
the same three fields. **Set reference** releases the selected driver and
displays the measured value. **Set driving** fixes its current value.

Click a circular arc to select its radius. Tab selects its signed sweep.
Click an elliptic arc to select its sweep directly. F2 accepts an angle in
degrees or radians, including expressions such as `-2 * #turn`. The two
guide lines connect the center to the start and end of the selected sweep.
Reversing a semicircle from 180° to −180° keeps its chord and puts the
solid on the opposite side. An edit that opens an extruded profile reports
the failure and retains the previous solid.

To drive a line with a formula, choose **Variables** (F3) and enter
`w = 1 in`. Apply it, select the line, and enter `#w + 6.725 mm` in
**Set length**. The line measures 32.125 mm. F2 reopens the saved formula.
Changing `w` to `2 cm` rebuilds the line at 26.725 mm and updates connected
geometry in one undo step. In the variable editor, Tab cycles through
definitions and a blank entry for creating another variable. Renaming a
variable updates its references. Clear an existing definition to delete it.
A variable used by another definition or dimension must remain defined.

To place a point, choose **More tools** and then **Point** or **Datum**.
Click once per point. Period selects Point and Shift+Period selects Datum.
A filled square marks an ordinary point. A hollow square marks a construction
point. Both can anchor a dimension. A point adds no edge to a solid profile.

Press **W** or choose **More tools → Conics → Construction**. Click one
or more sketch entities. Click a selected entity again to deselect it.
**Apply** (Enter) converts the selection to construction geometry. If
every selected entity is already construction, Apply restores it to
ordinary geometry. **Escape** cancels the selection. Blue dashed curves
remain available as sketch guides and do not contribute to extrusion
profiles. Undo restores the entire selection in one step.

In the F8 sketch editor, drag a point marker to preview its motion. Driving
dimensions can move connected geometry or limit the point to an axis or
circle. A fixed point stays in place. Release to accept the move as one undo
step, or press Escape while dragging to cancel it.

Press **N** to add a distance. Pick two line
endpoints, circle centers or point markers in the viewport, including construction points.
The pick order sets the sign of an X or Y distance. During picking, **X**
and **Y** select an axis, **D** measures straight-line distance, **R**
switches between driving and reference, and Escape cancels.

Press **D** outside picking to cycle through existing distances. **F2** edits
a driving value or formula. **R** converts the selected dimension between
driving and reference. A reference dimension follows the geometry and leaves
the sketch free to move. **Delete** removes the selected dimension. Edits,
conversions and deletions can be undone.

These tools support up to 32 anchors per sketch. A line contributes two
endpoints and a point primitive contributes one. The solver also handles
horizontal, vertical, driving length, fixed endpoint and coincidence records.
For example, opening the Rust `graph-constrained-line.pcad` fixture and
setting its length to 125.125 mm leaves the start at (−50, 20) and the end
at (75.125, 20). The line retains two degrees of freedom for translation.
A drag that holds the opposite endpoint therefore leaves this line in place.

To revolve the selected profile, choose **Revolve** or press **8**. Pick two
points in the sketch to define a directed axis, or choose **X axis** or **Y axis**
to use a sketch axis through the origin. The profile may touch the axis along
an edge and must stay on one side of it. Escape cancels the axis selection.
The new solid starts at 360 degrees. Select its angle and press F2 to enter a
partial sweep, then save the drawing. On a supporting face, choose **join**,
**cut** or **newBody** before starting the revolve.

Choose **Loft** or press **9** to connect two closed profiles on different
sketches. The picker highlights the current profile. Use **Next sketch**
(Tab) and **Next profile** (P), then **Use profile** (Enter) to choose the
first section. Choose the second section and click **Create loft** for a
separate body or **Join loft** to add it to the active body. Enter and
Shift+Enter perform those actions. Escape returns to the previous view.
Both sections must have a single outer loop. If the surface would cross
itself, the picker stays open so you can choose another section.

Choose **Edges** or press **H** to chamfer a solid. Choose **Fillet** or
press **Shift+H** for a round. Click each visible straight edge to include
it. Selected edges turn amber, and clicking one again removes it. Choose
**Apply** or press Enter to create the feature with a 1 mm distance or
radius. Press F2 on its dimension to enter an exact value. Escape cancels
the selection. The saved file records the edges and the dimension so you
can reopen the drawing and edit the finish.

Open **History** or press **F7** to inspect a v4 part’s solid features.
Click a row or use Left and Right to select it. **Suppress** (S) toggles
a feature. **Roll before** (B) and **Roll after** (A) set where evaluation
stops. **Restore end** (E) evaluates the entire timeline again. Up and Down
move the selected feature earlier or later. The editor rejects moves that
break a dependency or change a boolean’s target body. Ctrl+Z and Ctrl+Y
undo and redo these edits. Ctrl+S saves the timeline to the open file.
Escape closes the panel.

A failed feature shows its reason in the toolbar when selected. The viewport
can still display independent bodies. Repair or suppress the failed feature
before exporting. Rolling back before the first feature leaves an empty
viewport. Reopen History to move the rollback position forward.

The **Profiles** toolbar provides these placement modes.

| Tool | Picks |
| --- | --- |
| **Diameter circle** (D) | Two opposite points on the rim |
| **Center rectangle** (Shift+R) | The center and a corner |
| **3-point rectangle** (T) | Two points on the first edge, then a point setting the height |
| **3-point centered** (Shift+T) | The center, a side midpoint, then a point setting the perpendicular half-height |
| **Polygon** (Shift+G) | A center and a vertex |
| **Slot** (Shift+S) | Both cap centers and a point on the side wall |

Open **Dimensions** (F8) to drive the rectangle’s width, height or
rotation. All three rectangle modes save an editable rectangle with its
center and axes. The diameter circle uses the ordinary circle dimensions.

A polygon starts with six sides. Point at its outline and press F2, or
select it in the Design tree and choose **Set sides**, to change the count
to 3–64. **Dimensions** drives its radius. Press F2 over a slot or choose
**Set width** in the tree to change the full width in millimetres while
keeping both cap centers in place.

Choose **More tools → Conics → Ellipse** (Shift+E). Click the center, the
endpoint of the first axis, then a point to set the perpendicular height.
For a **3-point circle** (Shift+C), click three points on the rim. The third
click commits the shape. Backspace retracts the last pending point. A
collinear third point leaves the first two picks available for correction.

Choose **More tools → Conics → Split ellipse** (Shift+X). Click two
distinct points on an ellipse’s rim. To split an existing elliptic arc,
click once inside its sweep. Each split is one undo step. After saving
and reopening, editing the width, height or rotation of any piece in
**Dimensions** (F8) still updates the whole ellipse.

In **More tools**, choose **Tangent** (Shift+J), click a free line, circular
arc or open spline endpoint, then click the new arc’s endpoint. **Smooth**
(J) joins the nearest endpoints of two curves. Select the curve to keep in
place first, then the curve to align. Selecting the same pair again removes
their smooth join. To reshape a joined chain, select a curve with **Smooth**
and drag one of its handles. Release to apply one undo step. Escape cancels
the drag. These tools support sketches whose driving relations are smooth
joins. An edit that requires deforming a closed chain is declined.

Choose **More tools** for arcs and splines. For a three-point arc (A), click
the start, a point on the arc and the end. A fit spline (S) passes through
the points you pick. A control spline (B) takes four points for its first
cubic segment and three more for each additional segment. A periodic fit
spline (K) closes smoothly through at least three points. Choose **Finish**
or press Enter to accept a spline. Backspace removes the last pending point
and Escape discards the pending curve. Each accepted curve is one undo step.
Home fits the sketch and its supporting face guides in the window.

To sketch on a solid, choose **Face sketch** or press F, then click the
visible face. The sketch opens with the supporting face outline and any
holes shown as guides. Draw in its local coordinates. Save and reopen to
continue drawing, or press V to inspect the solid and V again to return.
**Extrude** joins the new material by default. Click **join** or press O to
cycle through join, cut and new body. A face cut runs inward. Escape
cancels a pending face pick.

In a face sketch, **Use edges** in Conics or Shift+U projects the supporting
face's straight boundary lines. **Pick edge** or U lets you click a straight
body edge guide, and Escape leaves the picking tool. The blue projected lines
follow their source and can form a profile. Their endpoints are fixed by
the projection. The **Line** tool snaps to a projected edge within six
pixels. A line across a closed projected outline splits it into separate
profiles. Click **Profile** or press P to cycle through them. The amber
outline shows the region used by the next extrusion. **Construction**
turns selected lines into guides and excludes them from the profile.

| Control | Action |
| --- | --- |
| Click the dimension label or press Tab | Cycle through the selected extrusion’s editable dimensions, then select the next extrusion. All supported profiles expose depth. Corner rectangles also expose width and height. |
| Click + or − | Change the selected dimension by 1 mm. The keyboard also accepts keypad + and −. |
| **Extrusion options** in the Design tree or F4 in Part mode | Reopen the extrusion preview to edit its operation, depth, direction or end condition. |
| Sketch / Solid or V | Switch between the selected profile’s outer and inner outlines and the shaded solid. |
| Left-click a solid | Select and highlight the nearest body or assembly occurrence. Click empty space to clear the selection. |
| Middle-drag | Pan the 3D view. |
| Right-drag | Orbit around the surface under the pointer. A tangential drag at the viewport rim rolls the camera. |
| Arrow keys, mouse wheel | Orbit and zoom the solid view. |
| Fit or Home | Recenter the drawing and restore the opening zoom at the current orientation. |
| Undo / Redo | Move through the edit history. Ctrl+Z and Ctrl+Y also work. |
| Save or Ctrl+S | Save to the open `.pcad` file. External content changes prevent overwriting the file. |
| Export or Ctrl+E | Write a binary STL per body beside the project, choosing unused filenames. |
| Ctrl+Alt+E | Write all bodies in the selected part to one 3MF beside the project, choosing an unused filename. |
| STEP or Ctrl+Shift+E | Write a faceted AP214 `.step` file beside the drawing. A repeated export uses the next unused filename. |

The shared library evaluates corner and rotated rectangles, circles,
ellipses, regular polygons, capsules and connected arc, elliptic arc, line
and spline segments on XY, XZ and YZ reference planes
and on the cap or side faces of earlier extrusions. Editing a supporting
extrusion rebuilds its face sketches. A cut on a face runs inward by default.
It uses the saved segment counts and profile IDs, including holes, islands
and disconnected regions. The evaluator rejects touching or intersecting
contours outside projected subdivisions.
Extrusions support blind, symmetric, through-all or up-to-body extents and
cut/join operations. The evaluator respects feature order,
suppression and rollback. Width and height edits preserve the rectangle centre
or the fixed corner. Unsupported sketch dependencies produce an edit error.
General sketch solving, additional solid features and edge and vertex picks
for assembly mates are tracked in the [migration plan](flexscript-migration.md).

## File operations and tests

Open a `.step` or `.stp` file in PuppyCAD to make a new single-file PCAD
drawing from one of its faceted solids. **Previous** and **Next** select
a body by its STEP name and entity number. Right-drag to orbit the preview
and scroll to zoom. **Import** accepts the body in one undo step. Ctrl+S
saves beside the STEP file, choosing an unused filename.

For a file with assembly placements, choose **Assembly** or press A to
preview the whole assembly in a new drawing. **Import** accepts it as one
undoable edit. The drawing keeps reusable part and subassembly definitions,
their occurrence names and the rigid placements stored in the file.
**Previous** and **Next** return to individual body selection. Whole-assembly
import is also available for an empty standalone design or Biscuit's original
starter drawing.

In an existing standalone PCAD drawing, click **Import STEP** or press
Ctrl+I. Enter the STEP path. Relative paths start in the drawing's folder.
**Target** cycles between adding a part and replacing an existing imported
shape. Replacement preserves its node ID, part references and assembly
placements. Escape cancels and returns to the previous camera view.
Each imported shape records its source filename, entity number, body name
and the SHA-256 of the file it came from.

Use `--step-bodies` to list valid body IDs for a headless import. A STEP
file containing one solid can be imported without specifying its ID.

```sh
workdir/flexscript/puppycad --step-bodies sensor.step
workdir/flexscript/puppycad --import-step sensor.step support.pcad --body 5897
workdir/flexscript/puppycad --import-step sensor.step sensor.pcad --assembly
workdir/flexscript/puppycad --roundtrip input.pcad output.pcad
workdir/flexscript/puppycad --export input.pcad output.stl
workdir/flexscript/puppycad --export input.pcad output.3mf
workdir/flexscript/puppycad --export input.pcad assembly.step --assembly "Arm assembly"
./build.sh test
```

Opening a recognized drawing with an unsupported record shows **Read-only**
and identifies the record. The check includes other parts in the file,
suppressed features and features below the rollback bar. A v4 solid preview
can show the earlier geometry. A sketch with unsupported constraints can
show its saved curves in local sketch coordinates. **File source** displays
the original JSON.
Tab switches between source and preview when both are available. In the
source view, use the wheel, arrows, Page Up, Page Down, Home or End. In the
preview, the arrows rotate and the wheel zooms. Escape closes the file.
A newer schema opens directly to source. Removing an unsupported record in
a capable editor allows PuppyCAD to reassess the drawing on its next open.
When the source inspector identifies an unsupported record or schema,
`--roundtrip` copies the original bytes, including spacing and line endings.

Round-trip serializes the project graph and preserves unknown JSON fields.
Export evaluates the selected part or assembly in the file. For a part,
STL uses numbered filenames for additional bodies. For an assembly, STL
uses the instance names and 3MF stores one named object per instance. For a v4 file, use
`--export input.pcad output.3mf --assembly NAME` to select an assembly by
name or ID for that export.

STEP accepts `.step` and `.stp`. The writer retains v4 part and solid names,
assembly occurrence names and poses, and standalone nested subassembly
definitions. Scaled or reflected v4 occurrences get distinct local geometry.
The writer requires closed oriented shells and limits the file to 2 MiB.
It reports enclosed void shells as unsupported. A failed export preserves
the previous file.

Drag a servo-driven occurrence with the left mouse button to move its hinge.
The picked point follows a circle around the solved world axis and stops at
the servo limits. A fastened child can also serve as the handle. Release
to accept one edit, or press Escape before releasing to restore the pose.
A rigid subtree previews with a GPU transform. Other supported mechanisms
use the shared solver to evaluate their preview.

In a v4 assembly, click **Mate** and pick a vertex, edge or face on each of
two different instances. A visible corner has priority near the pointer,
followed by an edge, then the face underneath. You can pick just outside
a silhouette. **Isolate** restricts picking to the visible occurrence.

The two markers show where the connectors will meet. A straight edge
supplies its midpoint and direction. A curved edge highlights the entire
run and marks its center. Picking a circular hole or shaft rim places the
connector on its axis. A face supplies the surface point you clicked.
The second face points inward, and an edge axis can reverse to agree with
the other connector. Choose **Fasten**, **Revolute**, **Slider**, **Planar**
or **Ball** with the toolbar or keys 1–5. Before choosing the second
element, keys 1–6 switch views and right-drag orbits. Escape cancels.

For the keyboard shortcut, select the first element, hover over an element
on another instance and press J. To fasten an instance at its current world pose, select
it and press G, or press J over empty viewport space. Each accepted mate is
one undoable edit. Ctrl+S saves its connectors, mate and solved placements.

To move an individual instance, open **Objects**, select the occurrence
and click **Placement**. Click an X, Y or Z value to enter a translation
in millimetres, an XYZ Euler rotation in degrees or a scale factor. A
negative scale reflects the part along that local axis. Free copies of the same part
keep their placements. Enter applies the edit and solves the assembly.
If a mate holds the occurrence, the panel shows its resulting placement.
Ctrl+Z and Ctrl+Y undo and redo each accepted edit. **Back** returns to the
occurrence details.

To inspect a connection, open **Objects**, select an occurrence, and choose
a row under **Mates**. The row names the mating partner. The inspector shows
the mate type, ID and both connector IDs. For a revolute mate, click **Set
angle** or its current angle to enter degrees. Entering 0° also creates a
servo on an undriven revolute mate.

With a mate selected, **Remove mate** or Delete detaches it, removes its
drivers and releases endpoint connectors that no other mate uses. The freed
part keeps its displayed pose. With an occurrence selected, **Remove
instance** or Delete removes that occurrence and its attached mates. Other
copies still reference their original part definition. Each removal supports
undo and redo. An assembly saved with its last occurrence removed reopens
ready for **Insert**.

A selected assembly opens in its solved pose. Tab selects a servo. Click
**+** or **−** to adjust its home angle by one degree. Save records the angle
and the mated instances' world transforms. Fastened, revolute, prismatic
(slider), planar and ball mates are supported. The solver keeps the first
instance's rigid group and any world-fastened groups grounded. **Solve**
records the computed placements as one undoable edit. Save writes them back
to the open file. A set of conflicting mates produces an evaluation error.
The regression compiles the same native entry point as the editor, reads the
Rust fixtures, checks STL and 3MF topology, bounds and volume, and exercises rejected
edits and failed writes. It accepts the same `FLEX_COMPILER` and
`FLEXSCRIPT_LIB` environment variables as the build above.

For the native window regression, build both the Flexscript executable at the
path above and the Rust debug executable. With Xvfb, xauth, xprop, libXtst,
Mesa llvmpipe, ImageMagick and Pillow installed, run
`python3 tests/window.py`. The harness starts an authenticated headless X server,
removes inherited desktop display connections and uses two llvmpipe threads
at low scheduling priority. It edits and reopens temporary drawings, compares
exports with Rust and writes screenshots to `workdir/flexscript/`.
It finds Xvfb on PATH or at `workdir/flexscript/headless/Xvfb`.
