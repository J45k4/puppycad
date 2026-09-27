# Sketch workspace

Choose **New → Part**, select **Sketch plane**, then **New sketch**. Sketch mode keeps the project layout, hierarchy, and panel sizes in place. Sketch tools and the drawing canvas use the existing viewport, and sketch properties use the existing properties panel. Finish or Cancel restores the part controls without recreating the 3D view. The workspace supports drawing lines, rectangles, circles, and native three-point arcs. Finish keeps the sketch as an independent entry in the project hierarchy. **Apply changes** saves the part. A sketch does not need a solid or a closed region to be saved.

Select the sketch and choose **Extrude sketch**, a closed region, and its depth. The extrusion references that sketch; reopening the sketch and changing a driving dimension rebuilds dependent geometry after Finish and Apply. Existing extrusion sketches also have **Edit sketch**.

## Drawing and constraints

- **3-point arc**: click start, end, and a point on the desired curve. Minor and major arcs retain their signed sweep. Use **Add driving dimension** for radius and **Dimension arc sweep** for angle. Endpoints and center are selectable and draggable.
- Circle: click its center and radius. Rectangle: click opposite corners. Line: click consecutive vertices; Escape ends the chain. Clicks within eight screen pixels snap to existing entity anchors. Shared line/arc endpoints receive coincidence constraints. Axis-aligned lines drawn with grid snapping receive horizontal or vertical constraints.
- Construction geometry stays visible as dashed lines and is excluded from filled regions and extrusion profiles.
- Select geometry on the canvas or through the Entities list. Shift-click selects multiple entities. Select endpoint handles for point constraints; dragging a point solves its connected geometry.
- **Dimension** or **Add driving dimension** creates a circle diameter, line length, or primitive width. Rectangles also expose height. Click a dimension label or constraint-list entry to edit its value; drag the label to reposition it.
- Horizontal, vertical, coincident, concentric, equal, parallel, perpendicular, tangent, fixed point, distance, X/Y distance, angle, and radial distance are supported. Radial distance is the first selected circle's radius minus the second's. Fix anchors the selected point at its current position.
- The status shows remaining degrees of freedom or conflicting constraints. Conflicts prevent Finish. Constraints can be deleted without deleting geometry. Underconstrained sketches can be finished.
- Local Undo/Redo and Cancel preserve the accepted sketch until Finish. Middle-drag pans, wheel zooms, and Fit sketch frames the geometry.

Relations, construction flags, and label positions persist through project JSON and PCad conversion. The nonlinear numerical solver applies all relations together; its rank determines the displayed remaining degrees of freedom. This is a bounded initial editor, not full Onshape feature parity: splines, tangent arc creation, mixed-curve offsets, expression-based dimensions, and finite-arc tangency are not implemented. Circle-circle tangency supports external and internal contact; arcs currently use their supporting circle for external tangency. Radius, equal-radius, concentric, and endpoint relations support arcs. New standalone sketches use principal planes; existing face-attached extrusion sketches retain their support. Existing additional filled-region recipes remain separate from the main extrusion sketch workspace.

## Verification

Tests cover coupled annulus dimensions, solver conflicts/redundancy, geometry rebuild, JSON/PCad round trips, canvas drawing, construction, local undo/cancel, chained-line constraints, and standalone sketch-to-extrusion ownership.

The integrated-browser test project is `4b6480fc-489c-4323-823a-87e4e792350d`. A construction circle and two boundaries were drawn through the UI, constrained, saved, reloaded, and extruded. Changing its diameter drives both boundaries while retaining a 16 mm ring width. Screenshots: `workdir/sketch-ring-constraints.png` and `workdir/sketch-ring-extrusion.png`.


Native arc browser proof is in **Part 2** of the same test project: draw a 50 mm radius arc with a closing chord, increase the radius to 60 mm, constrain its -180 degree sweep, fix its center, and constrain the chord horizontally. Finish, Apply, Extrude sketch, and save/reload preserve one native arc, one line, and all six constraints. Screenshots: `workdir/sketch-arc-native.png` and `workdir/sketch-arc-extrusion.png`. Unit tests also verify major arcs, open arcs without accidental chord closure, construction exclusion, solver coupling, and feature/entity ID collisions during serialization.

The broader parity effort remains open; see [Sketch parity tracking](sketch-parity.md).


## Trim, split, and extend

Select **Trim** and click the portion of a line, arc, or circle to remove. Intersections divide the selected curve; a curve with no dividing intersection is removed entirely. Select **Split** and click inside a line or arc. For a circle, choose two points. Select **Extend** and click near the line or arc endpoint to extend to the next finite sketch boundary. Closed circles have no endpoint to extend. These tools currently edit lines/arcs/circles; rectangle and slot outlines can act as boundaries but are not themselves trimmed or split.

Split circular pieces remain concentric with equal radius; split lines remain collinear. Unchanged endpoint references follow their surviving piece. Changed endpoint, driving length, driving sweep, and legacy length dimensions are removed and reported in the status; radius/diameter and compatible supporting-curve constraints remain. Undo restores both geometry and constraints. Newly trimmed intersection endpoints are not yet linked parametrically to their cutting boundary.

Automated coverage includes line and circle intersections, finite arc bounds, circular wraparound, extension of both arc endpoints, constraint reference remapping, and canvas trim/Undo/Redo. Live trim/split/extend verification is still pending: the test sketch in Part 3 was saved, but integrated-browser input stopped reaching the page after a save dialog. A narrow-layout ResizeObserver loop observed during that run was fixed by deferring redraws to animation frames and allowing the canvas grid track to shrink; compact-layout visual re-verification is also pending.

### Linked offsets

Select one entity and click **Offset** to create a native copy at 5 mm. Edit its **Dimension (mm)** field to change the signed distance. Positive distances expand circles, arcs, rectangles and slots; lines move to the left of their start-to-end direction. Negative distances inset shapes or move lines right. Construction status is inherited. The persistent offset relation links both geometry and dimensions, and supports Undo/Redo. Trimming, splitting or extending either entity removes the offset relation and reports it because the original whole-entity correspondence no longer applies.

Shift-select a connected chain of lines and click **Offset** to create mitered joins controlled by one distance. Closed chains expand for positive distances; open chains offset left along their ordered direction. Reversed stored edges and shuffled selections are supported. Disconnected selections, branches, reversing corners, collapsed edges and crossing offset edges are rejected. Trimming, splitting, extending or deleting any participating entity removes the group relation; dimension edits retain it. Mixed line/arc chains, rounded joins and offset previews remain unimplemented. Automated geometry, solver, UI-harness and PCad persistence checks cover single-entity and line-chain offsets; live browser proof remains pending because integrated-browser clicks are not reaching the current test page.

### Midpoint and point on curve

Select a point (or an entity to use its default anchor), then Shift-select a different curve. **Midpoint** places that point at a line midpoint or halfway along an arc. **Point on curve** keeps the point on a finite line, circle or arc while allowing it to slide. It does not use invisible line or arc extensions. A fixed point outside the curve produces a conflict. Both relations persist through PCad, support Undo/Redo, and are removed when their target curve is trimmed, split, extended or deleted. Unsupported curves show an error. These controls have solver and UI-harness coverage; live integrated-browser verification remains pending.

### Drawing slots

Choose **Slot**, click the two end-cap centers, then click away from their centerline to set the width. A dashed outline previews the slot before the third click. Coincident centers and zero width are rejected; choose a new point or press Escape to cancel. Native slots retain center anchors, construction status and snapped coincidence constraints. Use **Add driving dimension** for width and **Dimension slot center distance** for the distance between cap centers. The overall length equals that center distance plus the width. Width/length changes, Undo/Redo, closed-profile materialization and PCad persistence are covered by tests; live browser verification remains pending.

### Point symmetry

Select two points (or two entities to use their default anchors), then Shift-select a separate line and click **Symmetric**. The line may be construction geometry. The relation keeps the points reflected across the axis as dimensions or the axis change; it does not create a mirrored copy. Two endpoints of one entity can also be selected before the axis. Degenerate axes are rejected, and incompatible fixed points report conflicts. Undo/Redo and PCad persistence retain all three references. Deleting or trimming/splitting/extending the axis removes the relation. Symmetry between existing whole entities remains a separate gap; the Mirror tool creates linked copies. Automated tests cover the relation and toolbar workflow; live browser verification remains pending.

### Mirror copies

Select one or more source entities, Shift-select the mirror line **last**, then click **Mirror**. The tool creates native copies linked to both the source geometry and the axis. Source dimensions and axis edits update the copies through the solver. Lines, circles, arcs, slots and rectangles are supported. Arcs reverse their sweep; corner rectangles become rotated native rectangles to support arbitrary axes. Construction flags are inherited when copies are created.

Undo/Redo restores the copies and relations together. Deleting the source or axis leaves the copy in place and removes the broken mirror link. Trimming, splitting or extending either participant also removes the link and reports it. A zero-length axis is rejected. Tests cover geometry, closed profiles, driving dimensions, PCad persistence, toolbar selection and deletion; live browser proof is still pending.

### Internal circle tangency

Select two circles and click **Internal tangent**. The initially larger circle becomes the container, regardless of selection order. The relation persists that containment direction and requires the center distance plus the inner radius to equal the outer radius. With fixed radii and one fixed center, the inner circle can slide around the inside. Equal coincident circles and impossible containment report conflicts. Splitting or trimming either circle removes the relation. Tests cover the solver, toolbar, Undo/Redo and PCad persistence; internal arc contact and live browser proof remain pending.

### Center-point arcs

Choose **Center arc**, click the center, click the start to set the radius, then click the end direction. The third point is projected onto that radius. The default sweep is counterclockwise; toggle **Clockwise arc** to choose the other direction, including major arcs. A center marker and dashed radii guide the preview. Coincident directions and zero radius are rejected; use Circle for a full revolution. Center snaps become persistent coincidence relations. Radius and sweep dimensions, construction geometry and Undo/Redo use the same native arc model as three-point arcs. Automated tests cover both sweep directions and the toolbar workflow; live browser verification remains pending.

### Center and three-point rectangles

**Center rectangle** takes a center and a corner, and preserves a snapped center as a coincidence constraint. **3-point rectangle** takes the two ends of its first edge, then a point setting the perpendicular height. Both tools preview and store a native rectangle. Select it to add **Dimension width**, **Dimension height** and **Dimension rotation**; rotation is entered in degrees. The existing **Rectangle** tool remains the axis-aligned corner rectangle.

Tests cover rotated geometry, both sides of the first edge, invalid sizes, editable dimensions, Undo/Redo and PCad persistence. Native rectangles expose all four corner anchors (`p0` through `p3`, in outline order) plus their center. Corner handles support dragging, fixed-point and coincidence constraints, distances and symmetry. Snapped rectangle corners acquire persistent coincidence relations, and attached geometry follows size and rotation edits. Live browser verification remains pending.

### Session ownership and keyboard history

The part's feature panel owns at most one sketch workspace. Opening another sketch closes the previous session without committing it. Discarding part changes or disposing the part editor closes its workspace; detached Finish buttons cannot write into a later session. Apply is unavailable until the active sketch is finished or cancelled.

The drawing canvas receives focus when opened. Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z, plus Ctrl+Y, operate on sketch-local history without also changing project history. Text-field undo remains native to the field. Tests cover keyboard event propagation, owner disposal, replacement, discard and stale callbacks; live browser verification remains pending.

### Sketch fillets

Select two lines sharing an endpoint, set **Fillet radius (mm)** in the properties, then click **Sketch fillet**. Both lines must have matching construction status. The tool shortens them and inserts a native arc with a driving radius, endpoint coincidence and oriented endpoint-tangency constraints. Select the radius constraint to edit it later. Obsolete corner and line-length constraints are removed and listed in the status; Undo restores them with the sharp corner.

Creation rejects zero, oversized and collinear cases. Driving edits that collapse or reverse the trimmed edges report conflicts. Tests cover precise radius edits, closed profiles, PCad persistence and the toolbar workflow. Fillets between other curve types, direct native-rectangle corner fillets, and live browser proof remain pending.

### Saving without a blocking dialog

**Save to Server** displays progress and completion inline. The button is disabled while its request runs, and duplicate requests are ignored. A failed save leaves the browser recovery copy available, shows an inline retry message, and re-enables the button. Saving no longer opens a native alert. Existing browser dialogs from an earlier build are unaffected by this source change. Automated tests cover pending, success, failure and retry states; live browser confirmation remains pending.

### Linear patterns

Select source entities and click **Linear pattern**. Edit **Pattern columns** and **Pattern rows** (including the source cell), **Pattern step X (mm)** and **Pattern step Y (mm)** in the pattern properties. Copies retain native geometry and stay linked to source dimensions. Row step X/Y controls the second direction, including skew grids. Count changes retain surviving cell IDs; shrinking removes dependent constraints on deleted copies and reports them. Undo/Redo restores the entire group.

The current numerical solver limits a pattern to 2–32 total instances and 64 copied entities. Larger-capacity solving remains a gap. Deleting, trimming, splitting or filleting a participating entity removes its pattern relation; remaining copies become independent geometry. Tests cover driving dimensions, count changes, native arc/line profiles, reference remapping, PCad persistence and the toolbar workflow. Live browser proof remains pending.

### Circular patterns

Select source entities and click **Circular pattern**. The default creates four instances around the origin. Edit **Pattern count**, **Pattern center X/Y (mm)** and **Pattern angle (degrees)** in the properties. A full ±360° pattern spaces instances without duplicating its first position; an open pattern includes both angular endpoints and stays open when its count changes. Negative angles run clockwise. Drag the selected pattern's center handle to reposition the center; the source geometry stays linked, and the whole drag is one Undo/Redo action.

Copies preserve native circles, arcs, lines, slots and rotated rectangles. Axis-aligned corner rectangles produce native rotated rectangle copies. Source dimensions drive all copies. Resizing retains surviving instance IDs and reports removed constraints on deleted copies. Circular pattern metadata and profiles survive PCad save/load and reference remapping.

The current limit is 2–32 total instances and 64 copied entities. The **Pattern center reference** selector attaches the center to an existing endpoint, circle/arc center, slot endpoint or rectangle corner. Source geometry dimensions then drive the pattern center. Choose **Free center** to detach at its current position. Dragging a linked center moves the referenced point through the solver and respects its constraints. Own-copy references are rejected. Splitting a reference curve preserves surviving anchors; removing its anchor removes the pattern relation. Free-center coordinates and angle are fixed pattern parameters. Skipped instances and larger-capacity solving remain gaps. Deleting or trimming a participant currently removes the pattern relation rather than leaving a skipped instance. These differences remain tracked against [Onshape's circular sketch pattern workflow](https://cad.onshape.com/help/Content/Sketch/sketch_circular_pattern.htm).

Automated geometry, solver, persistence and UI-harness tests cover circular patterns, including center dragging and local history. Live browser verification and screenshots remain pending because focusing the integrated PuppyCAD tab still times out.

The selected circular pattern shows a dashed sweep guide, a numeric angle label and an angle handle. Drag the handle to open or close the pattern; the pointer tracks continuously across the angular wraparound. Hold **Shift** for 15° increments. Near a complete revolution the handle snaps to ±360°, and near zero it keeps a nonzero 0.1° sweep. The numeric angle field remains available for exact values. One drag is one Undo step. **Escape** or a cancelled pointer gesture restores the pre-drag sketch without adding a history entry. These behaviors are covered by UI-harness tests; integrated-browser visual verification remains pending.

A selected linear pattern shows a column-spacing handle and guide; a two-direction grid also shows a row-spacing handle. Drag either handle to change that step's X/Y vector. Hold **Shift** to preserve the step's original direction while changing its length. Canvas snapping follows the existing snap setting. A zero step or a spacing change conflicting with existing constraints is rejected. Each accepted drag is one Undo step; Escape or pointer cancellation restores the pre-drag state. Single-row or single-column layouts show only their active direction's handle. Unit/UI-harness verification covers independent grid steps, direction locking, cancellation and rejected edits; live visual verification remains pending.

### Tangent arcs

Choose **Tangent arc**, click an existing line or arc endpoint, then click the new arc's endpoint. The first click must be within eight screen pixels of an eligible endpoint. The preview chooses the clockwise/counterclockwise and minor/major arc that continues the source's tangent direction. A point on the tangent line is rejected because it cannot define a finite-radius tangent arc. Construction mode and endpoint snapping apply to the new arc.

A persistent **smoothJoin** keeps the two endpoints coincident and their outward tangent directions opposite. Radius and sweep dimensions can reshape the new arc while retaining this join. Either endpoint of clockwise or counterclockwise source arcs is supported. Joining the free end to another existing endpoint can close an extrudable profile. Undo/Redo, PCad persistence and endpoint remapping after splitting are covered by tests. This adds finite endpoint-to-endpoint smooth joins; general tangency to arbitrary locations on bounded arcs remains a separate gap. Live integrated-browser verification is pending.

### Three-point circles

Choose **3-point circle** and click three distinct points on the desired circumference. The circle preview appears after the second point as the pointer moves. Collinear or repeated points are rejected, and the tool stays active so the third point can be corrected. The result is a native circle with the existing diameter controls, construction mode, profile creation and local Undo/Redo.

When a placement point snaps to an existing entity anchor, the new circle receives a point-on-curve constraint for that anchor. These references persist through PCad and keep the circle passing through the anchors when their dimensions change. Free placement points do not create separate point entities. Tests cover input order, small and large geometry, invalid inputs, preview/retry/history, reference inference and persisted dependency updates. Live browser verification remains pending.

### Native sketch points

Choose **Point** and click to place a point. Points appear as small crosses and can be selected, dragged, fixed, constrained to curves or other anchors, and used for distance dimensions or circular-pattern centers. Drawing other geometry at a point creates the usual endpoint coincidence. Point placement itself also retains a coincidence when it snaps to an existing entity anchor.

Points have two coordinate degrees of freedom and never create loops or solid profiles, regardless of construction mode. They remain native point entities through project and PCad persistence, linear/circular patterns and mirror operations. Offset is unavailable because a point has no offset curve. The SDK exports `sketchPoint(center)`. The part preview renders point markers, including point-only sketches. Unit and UI-harness tests cover points; live screenshot verification remains pending.

### Regular polygons

Choose **Polygon**, set **Polygon sides** (3–64), click the center and then a vertex. Enable **Circumscribed polygon** to use the second click as an edge midpoint instead; the polygon encloses a circle with that apothem. Both modes preview before placement and create a native regular polygon.

Select the polygon to edit **Polygon side count**, add a driving circumradius dimension or **Dimension rotation**. Every vertex has a selectable `vertex0`, `vertex1`, … anchor, so points and geometry can remain attached to polygon corners. Increasing or decreasing the count preserves vertex indices that still exist; removed-vertex constraints are removed and reported. Linked pattern, mirror and offset copies follow source side-count changes. Edit the source to change a linked copy's count.

Polygons form closed profiles and support construction mode, native patterns, mirror and parallel-edge offset. Their vertex references and dimensions persist through PCad. The SDK exports `polygon(center, radius, sides, rotation)`. Native polygon edges currently serve as trim boundaries; trimming/splitting the polygon itself is not implemented. Unit/UI-harness tests cover polygon authoring and dependencies; integrated-browser visual verification remains pending.

### Ellipses

Choose **Ellipse** and click the center, the first axis endpoint, and the second axis extent. The third point is projected perpendicular to the first axis. A live outline and axis guides show the resulting ellipse before placement. Zero-length axes are rejected. Select the ellipse to add **Dimension width**, **Dimension height**, or **Dimension rotation**; width and height are the full lengths of the first and second axes.

The center and four axis endpoints (`p0` through `p3`) support selection, dragging, coincidence and fixed-point constraints. Snapped center and axis placements retain matching entity-anchor references. Ellipses remain native geometry through profiles, patterns, mirrors and PCad persistence. The SDK exports `ellipse(center, width, height, rotation, segments)`.

Ellipse tangency to other curved entities, exact ellipse intersections, and trimming/splitting an ellipse remain gaps. Existing trim boundaries use the sampled ellipse outline. Ellipse offsets are rejected rather than substituting another ellipse for a true offset curve. Geometry and UI-harness tests pass; live integrated-browser visual verification remains pending.

**Point on curve** supports native ellipses through their analytic implicit equation. Select a point or entity anchor, Shift-select the ellipse, and apply the constraint. A free point retains one degree of freedom along a fixed ellipse, including when it begins at the ellipse center.

**Tangent** supports a line and a native ellipse in either selection order. It uses the ellipse's analytic support function, preserving tangency when either axis length or rotation changes. As with existing line/circle tangency, it constrains the line's supporting infinite line; contact may lie outside the drawn segment. Tests cover rotated ellipses, center-start singularities, both tangent sides, PCad persistence, axis edits, and toolbar Undo/Redo. Tangency between ellipses and other curved entities remains unimplemented.

### Box selection

With **Select** active, drag empty canvas left-to-right to select entities fully enclosed by the blue window. Drag right-to-left for a green crossing box that selects entities whose curves touch the box. Hold **Shift** to add the result to the current selection. Escape or pointer cancellation restores the previous selection. Box selection does not add a geometry Undo step; edits applied to the selected group use the normal sketch history.

Window bounds and crossing tests use analytic lines, circular arcs, ellipses and slot caps, plus native polygon/rectangle edges and points. A box wholly inside a closed curve does not select that curve unless its boundary is touched. Tests cover finite arcs, curved intersections between display samples, additive selection, cancellation and grouped deletion/Undo. Live integrated-browser visual verification is still pending.


## Reference dimensions

Select an entity and choose **Add reference dimension** to measure it without constraining its geometry. Select any supported dimension and toggle **Reference dimension** to switch between measuring and driving. Reference dimensions appear in parentheses on the canvas and show a read-only live value in the properties panel; converting one to driving uses its current measurement.

Reference dimensions do not consume degrees of freedom or add solver equations. They follow geometry changes and persist through PCad save/load. Length and sweep references remain on a surviving trimmed or split fragment and measure that fragment. Unit and UI-harness tests cover live updates, conversion, undo, persistence and unchanged degrees of freedom. Integrated-browser visual verification remains pending.


## Edit dimensions on the canvas

Double-click a driving dimension label, or focus it with Tab and press Enter, to edit its numeric value beside the label. Enter or **Apply dimension** commits one undoable change. Escape or **Cancel dimension** discards the draft. Invalid numbers and values that conflict with existing constraints remain editable with an error; they do not change geometry or create history. Reference dimensions remain read-only. Panning, zooming or another canvas redraw dismisses an uncommitted draft.

The editor accepts values in millimeters or degrees by default, explicit units and arithmetic as described below. Persistent formulas, named variables, document display units and dimension reuse remain gaps. UI-harness tests cover keyboard activation, confirmation, cancellation, rejected values, geometry changes and undo/redo. Live integrated-browser screenshots remain pending because tab focus is timing out.


## Dimension units and arithmetic

Canvas dimension editors and sketch property fields accept arithmetic with `+`, `-`, `*`, `/`, parentheses and `pi`. Length fields accept `mm`, `cm`, `m`, `um` (also `µm` or `μm`), `in`/`inch` and `ft`; angle fields accept `deg`, `°` and `rad`. Examples: `2 in`, `2 * (1 in + 5 mm)`, `(pi / 2) rad`. A unit suffix applies to the preceding number or parenthesized expression. Values without units use the field's displayed units. Count fields accept only dimensionless results.

Addition and subtraction require matching dimensions: write `1 in + 5 mm`, not `1 in + 5`. Products require a dimensionless factor; division by a matching dimension produces a dimensionless ratio. Incompatible units, malformed expressions, excessive nesting and non-finite results are rejected without applying a value.

Expressions are evaluated at entry and persisted as numeric millimeters/degrees. They are not retained as formulas and do not create dependencies on other dimensions. Parser tests and UI-harness tests cover conversion, validation, canvas/property entry and PCad persistence. Integrated-browser verification remains pending.


## Snap target feedback

While placing geometry, a green square and label identify the next snap target: an entity anchor, the origin, or the rounded grid position. Feedback appears before the first click and uses the same target calculation as placement. Geometry anchors are chosen by nearest distance within eight screen pixels, so the tolerance stays consistent across zoom levels. Tangent-arc starts show only eligible line or arc endpoints.

**Snap to grid** and **Snap to geometry** independently control ordinary placement snapping; disabling both clears the feedback. Tangent arcs still require an existing endpoint to begin. Feedback clears on leaving the canvas or switching to selection. Tests cover nearest-target selection, zoom tolerance, tangent endpoint filtering, preview/placement agreement and the saved coincident relation. Live screenshots remain pending because integrated-browser tab focus is unavailable.


## Automatic constraint controls

**Automatic constraints** controls new inferred coincidences, horizontal/vertical line relations and three-point-circle point-on-curve relations. It does not change existing constraints. Snapping remains available independently, allowing a point to be placed exactly on another anchor without creating a dependency. Grid snapping can be disabled while geometry snapping remains enabled, preserving off-grid anchor coordinates.

Explicit constraint buttons and the defining relations of tools such as Tangent arc, Mirror, Offset and patterns remain active regardless of the automatic-constraint setting. Tangent arc still requires an existing endpoint and creates its smooth join. These controls apply to the current editor session and default to enabled when a new session opens. UI-harness tests cover independent placement, suppressed inference, explicit constraints and tangent-arc semantics; live visual verification remains pending.


## Sketch chamfer

Select two lines sharing one endpoint, set **First chamfer setback (mm)** and **Second chamfer setback (mm)**, and choose **Sketch chamfer**. Setbacks follow the selection order and may differ. The operation trims both lines and inserts a native line, retaining a closed profile when applied to a closed outline. The selected chamfer relation exposes its first value as **Dimension (mm)** and its second as **Second setback (mm)**; the canvas label shows both. Units and arithmetic are accepted in the fields.

The driving relation measures setbacks from the virtual intersection of the two supporting lines and keeps the inserted edge joined to them. Existing corner-endpoint and driving-length constraints that no longer apply are removed and reported; compatible constraints and reference lengths survive. Undo restores geometry and removed constraints. Setbacks must be positive and leave a nonzero portion of each edge. Collinear, disconnected and mixed-construction corners are rejected.

Chamfers currently support two native lines. Native rectangle corners and curved edges remain gaps. Deleting the chamfer edge removes its operation; trimming or splitting that edge dissolves the driving chamfer relation. Tests cover oblique/reversed corners, unequal setback edits, closed profiles, invalid sizes, PCad persistence, reference remapping and the toolbar workflow. Live integrated-browser visual proof remains pending.


### Distance-and-angle chamfers

Enable **Chamfer distance and angle** while two lines are selected to create a chamfer using the first setback and an angle. The angle is measured at the first cut endpoint, between the direction back toward the original corner and the new chamfer edge. For a right-angle corner, 45 degrees produces equal setbacks; other corner angles use the actual corner triangle.

On an existing chamfer, the same control switches between distance/angle and two distances by measuring current geometry, preserving the shape. The second field changes between millimeters and degrees and accepts the corresponding units. The mode and value persist through PCad. Angles must be positive, fit inside the corner triangle, and leave both supporting edges nonzero. An invalid edit restores the previous value and allows correction. Tests cover conversion, oblique corners, driving edits, rejected angles, persistence and toolbar history; live browser verification remains pending.


## Normal constraint

Select a line endpoint, Shift-select a line, circle, arc or ellipse, then choose **Normal**. Selecting whole entities uses the line's first endpoint (`p0`); either entity selection order is accepted for curved targets. With two lines, select the source line first and the target second. The endpoint stays on the curve while the line follows its normal there. Circles and arcs use the radial normal, and rotated ellipses use their analytic surface gradient. An arc's finite sweep is respected. A straight target keeps the selected endpoint on its finite segment and makes the source line perpendicular. A free normal line can slide along the curve and change length unless other constraints prevent it.

The relation persists through PCad and follows a surviving endpoint when its line is split. Removing the line or target curve removes the relation. Trimming or splitting the target remaps the Normal relation to the surviving fragment containing its contact point; deleting that contact removes the relation. Tests cover analytic ellipse normals, circle and straight-target degrees of freedom, finite arc and line bounds, invalid geometry, persistence, remapping and toolbar Undo/Redo. Normal constraints involving splines or other native curve types remain unsupported. Live verification of curved targets remains pending.


## Selection filters

The **Selection filter** menu offers **All geometry**, **Construction**, **Non-construction**, **Points**, **Lines** and **Curves**. Curves includes circles, arcs, ellipses and slots; Lines includes native line entities. Filters apply to the entity list, canvas hit targets, directional box selection and curve-edit tool targets. Excluded geometry remains visible, and a new filter clears any excluded selected entities or handles. Explicit constraint labels remain selectable.

Selection filtering is independent of grid/geometry snapping and does not change sketch geometry or create undo entries. After selecting a filtered group, Delete affects only that selection. The filter is local to the current workspace session. UI-harness tests cover all filter choices, canvas click rejection, box selection and saved deletion results; live integrated-browser screenshots remain pending.


## Redundant constraint feedback

The status bar lists **Locally redundant** relations, and the constraint list marks them in amber with **redundant**. Selecting one explains the diagnostic and exposes the existing **Delete constraint** action; deletion remains an explicit, undoable choice. Constraints are considered in their stored order, so a later duplicate is marked while the earlier relation is retained as the basis.

The diagnostic uses the solved Jacobian. A relation is marked only when its active rows add no local rank beyond earlier relations; partially independent relations and inactive zero rows are not labeled as wholly redundant. Reference measurements are excluded. Conflicting sketches receive conflict diagnostics instead. If the independent rank analyses disagree, redundancy labels are withheld. Local redundancy is not proof that a nonlinear relation can be removed without affecting all future edits, so removal is not automatic.

Solver and UI-harness tests cover duplicate dimensions, partial independence, inactive rows, conflicts, reference dimensions and explicit removal with undo. Live integrated-browser visual verification remains pending.


## Per-entity constraint state

Geometry now uses its own local constraint state even when the whole sketch remains underconstrained: dark gray for fully constrained entities, blue for entities with remaining freedom, red for entities participating in conflicts, and gray for uncertain states. Selection retains its orange highlight. The entity list also names the state, so color is not the only indicator.

The solver checks whether every parameter of an entity lies in the constraint Jacobian's row space. Coupled relations count: a point coincident with a fixed center can be fully constrained even if unrelated geometry remains free. Fixing only one endpoint does not mark an entire line fully constrained. Reference dimensions do not remove freedom. When a sketch conflicts, uninvolved entities are marked uncertain; disagreement between rank analyses also prevents a fully-constrained label. These are local numerical diagnostics rather than a guarantee about every finite deformation.

Tests cover mixed sketches, coupled points, partial line constraints, reference dimensions, conflict isolation and UI updates after deleting/undoing a constraint. Live integrated-browser screenshot verification remains pending.


## Analytic line–ellipse boundaries

Trimming or extending a native line against an ellipse now computes intersections from the rotated ellipse equation, independent of its display segment count. The same calculation handles finite box edges during ellipse crossing selection. Tangencies produce one contact; segment filtering excludes contacts beyond the segment. Reconstructing contacts from ellipse coordinates avoids subtracting very large line coordinates to recover small intersection positions.

Tests cover low-resolution ellipse outlines, rotated tangency, very long supporting lines, finite segments, exact trim/extend endpoints, and canvas trim with Undo/Redo. Circle/arc–ellipse contacts now use the polynomial intersection method described below; trimming/splitting the ellipse itself remains unimplemented. Live integrated-browser verification is pending.


### Circle and arc intersections with ellipses

Circle trimming and arc trim/extension now use the analytic ellipse and supporting-circle equations. A quartic polynomial is solved in two bounded half-angle intervals, including repeated roots for tangencies. This avoids both sampled boundary edges and roots at infinity. Contacts are filtered against finite arc sweeps by the editing operation; coincident circular ellipses provide no isolated cut points.

Tests cover four contacts, tangencies at axes and chart boundaries, offset/rotated geometry, disjoint and coincident curves, circle trim, finite arc extension and canvas Undo/Redo. Ellipse–ellipse intersections and editing the ellipse itself remain gaps. Live integrated-browser verification remains pending.


## Move and copy selected geometry

Select one or more entities, enter **Move X (mm)** and **Move Y (mm)** under **Move / copy**, then choose **Move selected** or **Copy selected**. Values accept units and arithmetic. Each operation is one undoable edit. Copies receive independent entity, constraint and legacy-dimension IDs; editing a copied dimension does not change the source group.

Relations wholly inside the group are preserved. Absolute fixed positions, circular-pattern centers and dimension-label positions translate with the geometry. A move detaches and reports relations crossing from selected to unselected geometry. A copy includes only internal relations and leaves all original relations intact. Construction flags and native entity types survive. Moving a complete linked pattern preserves its pattern relation; moving only part of it detaches the external pattern dependency.

Tests cover fixed/dimensioned geometry, external dependencies, independent copied dimensions, circular patterns, PCad persistence and the property workflow with Undo/Redo. Live transform-handle verification remains pending. Live integrated-browser verification remains pending.


### Uniform scale

With a group selected, set **Scale factor** and **Scale center X/Y (mm)**, then choose **Scale selected** or **Scale copy**. Positive factors enlarge or shrink about the chosen center. Geometry, linear dimensions, fixed positions, label positions and pattern spacing scale together. Angles, rotations, arc sweeps and a distance-angle chamfer's angular parameter retain their values. Two-distance chamfers scale both setbacks. Native entity types and construction flags are preserved.

Scaling uses the same group-dependency rules as translation: internal constraints survive, a scaled copy is independent, and an in-place operation reports detached constraints to unselected geometry. It is one undoable edit. Nonuniform scaling and reflection through a negative factor are not part of this control. Tests cover dimensioned copies, chamfer angle preservation, complete grid patterns, PCad persistence and UI history. Live integrated-browser proof remains pending.


### Group rotation

Set **Rotate angle (degrees)** and **Rotate center X/Y (mm)**, then choose **Rotate selected** or **Rotate copy**. Positive angles rotate counterclockwise in the sketch plane. Copies are independent, and each operation has one undo step. Fixed positions, labels, pattern centers and linear-pattern directions rotate with the group. Relative angles and lengths remain unchanged.

Horizontal/vertical line relations become driving orientation dimensions. Existing orientation dimensions change by the rotation angle. X/Y distance dimensions become signed distances along a rotated unit direction, retaining their value; repeated rotations rotate that direction again. Axis-aligned rectangles become native rotated rectangles, with their two corner references remapped to the matching native corners and an orientation relation preserving their original axis alignment.

The group dependency rules for move/scale also apply to rotation. Tests cover line orientations, signed coordinate dimensions, rectangle corner identities, PCad persistence, pattern directions, and UI Undo/Redo. Live integrated-browser verification remains pending.


### Canvas group-move handle

In Select mode, a **Move** handle appears above the selected geometry. Drag it to preview a rigid group translation. Hold Shift to constrain the delta to the dominant X or Y axis; **Snap to grid** rounds the delta to millimeters. This handle does not snap its offset position to geometry anchors. The preview lists external constraints that will be detached.

Pointer release commits the entire drag as one undo step. Escape or pointer cancellation restores the original geometry and constraints. Each preview is calculated from the drag-start snapshot, preventing accumulated motion. Tab to the handle and press Enter to focus the numeric Move X control. Live transform-handle verification remains pending.

The UI-harness test covers multi-entity movement, axis locking, cancellation, detached dependencies and Undo/Redo. Live integrated-browser screenshots remain pending.


### Canvas group-rotation handle

The purple **Rotate** handle rotates the selection around the visible pivot. Set the pivot with **Rotate center X/Y (mm)**. Dragging previews from the original geometry; Shift snaps to 15-degree increments. The handle follows the preview, and crossing the angular seam does not reverse the reported direction. A drag supports up to one full revolution in either direction; numeric rotation accepts other finite angles.

Release commits one undo step. Escape or pointer cancellation restores the original geometry and relations. External dependencies that would be detached are listed during preview. Tab to the handle and press Enter to focus the numeric angle field. Tests cover snapping, cancellation, undo/redo, transformed driving orientation and angular seam crossing. Live integrated-browser screenshots remain pending.


### Canvas uniform-scale handle

The green **Scale** handle previews uniform scaling around the visible square center marker, configured by **Scale center X/Y (mm)**. Movement is projected along the initial center-to-handle direction. Shift snaps the factor to tenths. Factors at or below 0.0001 are rejected while retaining the last valid preview; numeric scaling remains available for other positive factors.

Release commits one undo step. Escape or pointer cancellation restores original geometry and constraints. The preview uses the drag-start snapshot and transforms linear dimensions with geometry. Tab to the handle and press Enter to focus **Scale factor**. Tests cover snapped factors, rejected center crossings, cancellation, dimension preservation, keyboard access and Undo/Redo. Live integrated-browser screenshots remain pending.

The **Search tools** button (or **S** outside text fields) opens a searchable list of current enabled toolbar actions. Search matches words in tool names; Up/Down selects a result, Enter runs it, and Escape closes the list and returns focus to the canvas. Results also accept pointer clicks. Finish and Cancel remain separate explicit controls. This workflow has DOM-harness coverage; integrated-browser visual verification remains pending.

External **Tangent** constraints involving native arcs now require the contact to lie within each arc's sweep. This covers line–arc, circle–arc, and arc–arc pairs, including clockwise arcs and endpoint contacts. A free arc may adjust to satisfy the contact; a fixed arc reports a conflict when only its supporting circle touches. Lines retain supporting-line tangency semantics. Internal tangency also accepts circle–arc and arc–arc pairs, with the larger radius defining the outer curve; each arc must contain the shared contact. Equal-radius coincident curves remain invalid. Solver and PCad round-trip tests cover these cases; live browser verification is pending.

Geometry snapping now recognizes analytic ellipse–ellipse crossings and tangent contacts, with an **Intersection** preview label. Placement uses the same exact target, independent of tessellation. With automatic constraints enabled, eligible newly inserted anchors at exact intersection snaps receive point-on-curve relations to both source curves. Coincident ellipses do not produce isolated snap targets. Ellipse trim/split still requires native elliptical-arc support. Numerical and DOM-harness tests cover this workflow; live visual verification is pending.

Intersection snaps also cover native line, circle, arc and ellipse pairs. Line and arc contacts must lie within their finite extent; overlapping collinear lines do not yield isolated intersections. Conservative bounds exclude distant curves before pairwise intersection calculations. Endpoint-only tools continue to use endpoints, and disabling geometry snapping disables intersection snaps. Automatic constraints retain the intersection using two point-on-curve relations for eligible inserted anchors, including point and circle centers and line endpoints. Disabling automatic constraints keeps placement snapping without adding these relations. Undo/redo and PCad persistence are covered by UI-harness tests, including a point following a changed source radius.

Trim, Split and Extend preserve point-on-curve dependencies whose contact survives the edit. When an edit creates multiple fragments, each dependent anchor is reassigned to the fragment containing its current contact. A contact on a removed portion is detached and reported in the removed-constraints list. This also preserves inferred intersection dependencies. Midpoint and other operation-specific relations retain their existing cleanup rules. Geometry, PCad reload, changed-radius and UI history tests cover the new point-on-curve behavior; live verification remains pending.

Geometry snapping includes line and arc midpoints with a **Midpoint** preview. Arc targets use the midpoint of the signed sweep, including clockwise arcs. Eligible inserted anchors at these targets receive a midpoint relation when automatic constraints are enabled, so they follow later source edits. Disabling geometry snapping or using endpoint-only tools excludes midpoint targets. Numerical and UI tests cover placement, history, PCad reload and source edits; integrated-browser visual verification remains pending.

Live verification update: the integrated browser can again select the existing Project 3 tab, open Part 3 / Sketch 1, render the current full workspace, switch to Point, and cancel the edit. A screenshot was inspected in the browser tool. Midpoint placement is not yet live-proven: locator clicks on the horizontal SVG path fail with a zero-height bounding box, including a forced click. The test edit was cancelled without saving. A screenshot file under workdir and completed live midpoint placement remain outstanding.

Subsequent live evidence in the integrated browser: Search tools → `circle` → ArrowDown → Enter selected 3-point circle. In the existing Project 3 / Part 3 / Sketch 1 draft, numeric rotation entered through native keyboard events rotated the chord to a diagonal. Clicking its midpoint with Point active created `point-1` and `midpoint · constraint-2`; the live canvas showed the green Midpoint target. Undo removed the point, Redo restored the midpoint relation, and Cancel sketch closed the draft without saving. A full workspace screenshot was visually inspected in the browser tool. This proves live line-midpoint placement/inference/history and palette keyboard activation; arc-midpoint interaction, durable screenshot files under workdir, and the other pending workflows remain unverified. The horizontal zero-height SVG locator limitation remains specific to browser-tool targeting; the diagonal native path was clickable.

New sketch's **Sketch plane** selector now includes top, bottom and side face descriptors from valid existing extrusion steps, in addition to XY/XZ/YZ. Selecting one creates a face-referenced sketch rather than a fixed world-plane copy; an extrusion made from that sketch follows changes to its source face. Unsupported/invalid extrusion sources supply no choices. The existing Attached face label identifies this support inside the workspace. Direct viewport face picking and in-context model visibility are not yet implemented.

Live evidence: in the integrated browser's existing Project 3 / Part 1, selecting `Extrude 1: Top Face` and clicking New sketch opened `Sketch 2` with `Attached face` displayed. The empty draft was cancelled; the saved model was unchanged. Top/bottom faces are listed before the potentially numerous side-face descriptors. Full live creation/extrusion/save/reload of a face-attached sketch is still pending.

The face-attachment regression also serializes the complete part through PCad, restores its features, and confirms the attached extrusion resolves to the updated 30 mm support plane. Face choices are not computed while the New sketch controls are hidden in feature-navigation mode, avoiding unnecessary extrusion evaluation during unrelated feature edits.

Live persistence evidence: Project 3 (`4b6480fc-489c-4323-823a-87e4e792350d`) / Part 1 now contains Sketch 2, created on Extrude 1: Top Face with one point. Finish sketch, Apply changes and Save to Server completed with `Project saved on server.` After a full page reload, Sketch 2 reopened with Attached face and point-1 intact. The reopened editing session was cancelled. This isolated fixture is intentionally saved for continued testing; the original Onshape document and Flowergirl reconstruction were not modified. Live extrusion from the attached sketch remains pending.

Selecting a standalone sketch now exposes **Sketch support**, allowing its existing geometry to be reattached to XY/XZ/YZ or an available extrusion face. The selection is part of the normal draft and Apply/Discard workflow. Faces from the sketch's own extrusion, or from an extrusion depending on it through another face-attached sketch, are excluded to avoid cycles. An unresolved existing support is shown as Current face (unavailable), rather than being silently replaced with a world plane. Geometry coordinates remain in the sketch's local frame when the support changes.

Live support-editor check: selecting the saved Sketch 2 displayed the new Sketch support control and available faces. Subsequent browser selector/evaluation commands timed out before any support change was dispatched. Live reattachment remains unverified; the saved face reference was not changed by this check.

The extrusion feature editor now uses the same support picker as standalone sketches. Its earlier-step choices are additionally checked for dependency cycles, so reordering a dependent extrusion before its source does not make that dependent face a valid support. This also removes the older delimiter-encoded face-reference selection path and preserves unresolved support labels consistently in both editors. The UI regression includes a reordered source/dependent pair.

Live reattachment evidence: using the direct labeled select locator in Project 3, Sketch 2's support was changed to YZ and Edit sketch displayed YZ plane. Cancel sketch followed by Discard changes restored the saved face reference; reopening displayed Attached face. The final editing session was cancelled, with no server save. This verifies live draft support changes and discard recovery; cycle exclusion remains covered by automated regression tests.

Face-attached workspaces display the supporting extrusion's projected edges as a dashed, non-interactive reference. **Show support** toggles this view, and Fit sketch includes visible support geometry. The reference is projected into the actual top/bottom/side sketch frame, removes collapsed and duplicate edges, and is not added to sketch entities, snapping, constraints or profiles. It is a wireframe view of the source extrusion, not a shaded view of the final Boolean model; downstream features, occlusion and full assembly context remain unfinished. Unresolved support geometry does not prevent opening the sketch for repair.

Live support-context evidence: Project 3 / Part 1 / Sketch 2 displayed the supporting annulus as 192 projected edge segments while retaining its single point entity. Show support reduced the displayed reference count to zero, then restored all 192 segments. A full screenshot was visually inspected through the integrated browser. The sketch was cancelled without saving changes. A durable screenshot file under workdir remains outstanding.

The workspace performs its initial fit when ResizeObserver first reports a real, nonzero viewport, rather than retaining the constructor's detached-element fallback fit. Subsequent resizes preserve the chosen zoom; Fit sketch explicitly refits to current dimensions. The regression uses two viewport sizes and verifies initial fit, resize preservation, explicit refit and observer disposal.

Live initial-fit evidence: reopening Project 3 / Part 1 / Sketch 2 showed the supporting annulus expanded to the actual viewport's fit extent. The integrated-browser screenshot was visually inspected and the unchanged sketch cancelled. The prior fallback-scale display was no longer present; durable screenshot export under workdir remains outstanding.

When a face support cannot resolve, the workspace now displays a persistent Sketch support unavailable message containing the resolution error and instructions to close the sketch and choose a valid support. Geometry stays editable in its local coordinates. Reattaching to a valid plane clears the message on reopening; the original document remains untouched until Apply. The UI regression covers a missing extrusion, message persistence across tool changes, and recovery through the support selector.

**Symmetric entities** constrains existing geometry: select the two entities in order, then Shift-select a separate line as the symmetry axis. The entities retain their IDs and no copies are created. The existing mirror relation enforces reflected geometry, including circle radii and native shape parameters. Matching native types are supported; corner rectangles currently require recreation as native rectangles. **Symmetric** remains the point/anchor symmetry action, and **Mirror** creates copies. UI history and PCad tests verify existing-circle symmetry; live verification remains pending.

Live whole-entity symmetry evidence: in Project 3 / Part 3's temporary sketch draft, a second circle was prepared using Copy selected. Selecting circle-1, copy-circle-1-1 and line-1, then Symmetric entities, added mirror · constraint-2 while keeping three entities. The solver reported six degrees of freedom instead of nine. The complete draft was cancelled, leaving the saved test sketch unchanged. Existing-circle symmetry is live-verified; other entity types retain automated geometry coverage from the mirror solver.

Whole-entity symmetry and mirror constraints now treat ellipse and polygon rotations separated by full turns as equivalent, matching the existing rectangle/arc handling. Fixed source and target dimensions no longer produce a false conflict solely because one rotation is represented with an extra 360 degrees. A regression failed before this correction and passes for both native shape types afterward.


## Live straight-target Normal verification

In the integrated browser, Project 3 / Part 3 / Sketch 1 was opened as a temporary draft. Selecting line-1 and using Rotate copy at 90 degrees created copy-line-1-1 with a rotation constraint. With the copy selected first, Shift-selecting line-1 and choosing Normal attached the copied line's first endpoint to the horizontal target. The displayed geometry showed the perpendicular contact; the solver changed from nine degrees of freedom and two constraints to eight degrees of freedom and three constraints. The orientation was already constrained, so the new independent restriction was contact with the target.

Undo removed normal · constraint-2 and restored nine degrees of freedom; Redo restored the relation and eight degrees of freedom. Cancel sketch discarded the entire temporary draft. The saved model was not modified. The screenshot was inspected inline; a durable screenshot under workdir is still outstanding. This proves the straight-target toolbar workflow and history in the live UI, not curved-target behavior or persistence after saving/reloading.

When a constraint is selected, the creation settings for fillets and chamfers are hidden. They remain available when selecting two lines without a selected constraint; an existing chamfer retains its own constraint editing fields.

Selecting a constraint also hides geometry dimensions, construction toggles and transform controls, including canvas transform handles. Select an entity again to return to geometry editing or add another dimension. Constraint selection retains the geometry highlight for context.

Delete and Backspace remove the focused constraint before considering highlighted geometry. This uses sketch Undo/Redo and leaves the associated entities intact. With geometry selected instead, the keys retain their existing geometry-deletion behavior.

Splitting or trimming a tangent join’s target line preserves endpoint tangency on the surviving segment containing the arc endpoint. The orientation signs are retained, and the companion coincident relation follows the same endpoint. Removing the contact removes the join. This behavior has geometry tests; live UI verification remains pending.

## Live Normal target split verification

In the integrated browser, a temporary Project 3 / Part 3 / Sketch 1 draft was prepared by rotating line-1 to 45 degrees, creating a 90-degree rotated copy, and adding Normal from the copy to line-1. Clicking the target with Split produced line-1-split-1, retained normal · constraint-2, and added coincident and collinear relations between the fragments. Status changed from three entities / three constraints / eight degrees of freedom to four entities / five constraints / nine degrees of freedom, consistent with the new split point remaining free along the line. Undo restored the previous counts; Redo restored the split and retained Normal. Cancel sketch discarded the entire draft. This verifies live line-target splitting and history; curved targets, trimming and tangent-join splitting still need live verification.

Live circle-Normal/source-line split evidence: in a temporary Project 3 / Part 3 draft, selecting line-1 then circle-1 and adding Normal produced two entities, two constraints and four degrees of freedom. A subsequent canvas Split click intended for the circle hit the overlapping source line instead. The actual result retained circle-1 and normal · constraint-2, added line-1-split-1 with coincident/collinear relations, and reported three entities, four constraints and five degrees of freedom. Undo and Redo preserved this behavior; the draft was cancelled. This verifies the circle Normal toolbar action and source-line splitting, not circle-target splitting. Circle-target split verification remains pending because the automated center click hit overlapping geometry.

Origin snapping now infers a fixed anchor at (0, 0) for eligible newly placed geometry when Automatic constraints and Snap to geometry are enabled. This preserves the origin attachment through solving, Undo/Redo and the existing fixed-relation persistence. Disabling either option prevents this inference. Existing geometry anchors take precedence when the origin overlaps another anchor.

Live origin inference evidence: in an empty temporary sketch opened with New sketch in Project 3 / Part 3, placing a Point at the canvas origin created fixed · constraint-1 and reported Fully constrained, one entity and one constraint. Undo returned to zero entities/constraints; Redo restored the fixed point. After Undo and disabling Automatic constraints, placing the point at the same origin reported Underconstrained, two degrees of freedom, one entity and zero constraints. Cancel sketch followed by Discard changes restored the saved hierarchy containing only Sketch 1. This verifies origin point placement, history and the automatic-constraints opt-out in the integrated browser.

Corner rectangles participate in automatic anchor inference at both defining corners, including origin attachment and coincidence with existing geometry. A rectangle started at the origin becomes fully constrained after driving width and height are set; UI and PCad tests verify its first corner remains at (0, 0) during those edits.

Disabling Snap to geometry also suppresses automatic coincidence inference when a newly placed anchor happens to have exactly the same coordinates as an existing anchor. Explicit constraint commands remain available.

Live geometry-snap opt-out evidence: an empty temporary Project 3 / Part 3 sketch received a fixed point at the origin. With Snap to geometry disabled, placing a second point at the same location produced two entities, one constraint and two degrees of freedom. Undoing that placement, enabling Snap to geometry and repeating it produced fixed · constraint-1 plus coincident · constraint-2, with two fully constrained entities. Cancel sketch and Discard changes restored the saved hierarchy. This verifies the inference toggle against overlapping anchors through the integrated browser.

The Construction geometry checkbox applies to all selected entities in one Undo step. Mixed construction/non-construction selections display an indeterminate checkbox; checking it makes the selection construction geometry, and unchecking makes it normal geometry. Unselected entities retain their state.

Ctrl/Cmd+A selects every entity allowed by the active selection filter and returns to Select mode, cancelling unfinished placement. It does not add an Undo step. In text and numeric fields, the shortcut retains normal text-selection behavior.

Live Select All evidence: in Project 3 / Part 3 / Sketch 1, Ctrl/Cmd+A selected both circle-1 and line-1 under All geometry. Switching Selection filter to Lines and repeating the shortcut selected only line-1. Toggling Construction geometry changed the line to construction while the circle remained normal geometry. The draft was cancelled. This verifies filter-aware keyboard selection feeding a real editing operation in the integrated browser.

Escape cancels unfinished drawing or dragging while retaining the previous selection. When no gesture is active, Escape clears geometry, anchor and constraint selection without modifying the sketch or adding history. Delete after clearing selection has nothing to remove.

Live Escape evidence: in Project 3 / Part 3 / Sketch 1, selecting line-1 and horizontal · constraint-1 then pressing Escape closed the constraint editor and cleared the line's pressed selection state. Pressing Delete afterward retained both entities and the horizontal relation, with the same six degrees of freedom. The draft was cancelled. This verifies idle selection clearing and safe no-selection deletion in the integrated browser; active drag cancellation retains automated coverage.

Equal applies to every selected entity in a single Undo step. Select two or more lines to equalize lengths, or circles/arcs to equalize radii. Each additional entity is constrained to the first selected entity. Mixing lines with circles/arcs is rejected before changes are made.

Live multi-entity Equal evidence: in a temporary Project 3 / Part 3 / Sketch 1 draft, two successive Scale copy actions created circles at twice and four times the original radius. With the Curves filter and Ctrl/Cmd+A, Equal added two relations for the three circles. Status changed from twelve to ten degrees of freedom with four entities unchanged (three circles and the existing line). One Undo removed both Equal relations; one Redo restored both. The draft was cancelled. This verifies grouped Equal creation and history through the integrated browser; exact radii are covered by the UI solver test.

Horizontal and Vertical apply to every selected line in one Undo step. Selections containing non-line geometry are rejected before adding any relations.

Live group alignment evidence: a temporary copy of line-1 was made in Project 3 / Part 3 / Sketch 1. With the Lines filter and Select All, Horizontal added a relation for each line; because the copied lines already carried Horizontal, the new relations were correctly flagged redundant. That operation was undone, and the two existing Horizontal constraints were removed. Vertical then added two constraints in one action, reducing degrees of freedom from eleven to nine. One Undo removed both; one Redo restored both. The temporary draft was cancelled.

Reapplying Horizontal or Vertical skips existing matching driving constraints. Only missing constraints are added, and an already-covered selection adds no Undo step. This avoids exact duplicate axis relations while retaining broader redundancy diagnosis.

Live duplicate-axis prevention evidence: opening the saved Project 3 / Part 3 / Sketch 1 and reapplying Horizontal to line-1 kept exactly one horizontal relation and left the sketch Undo button disabled. Entity/constraint counts and six degrees of freedom were unchanged. The draft was cancelled. This verifies the no-op case through the integrated browser; partially covered groups have automated history coverage.

Parallel accepts two or more selected lines, constraining each additional line parallel to the first in one Undo step. Line lengths remain independent and retain their existing driving dimensions. Non-line selections are rejected.

A live group-Parallel check exposed stationary solver starts when free lines were exactly perpendicular. The solver now slightly perturbs the initial target direction for Parallel/Collinear at right angles and Perpendicular at parallel directions, before normal constraint solving. Focused tests verify the two direction constraints converge from those starts. Live re-verification remains pending.

Live stationary-direction regression verified: after reloading the solver fix, the original failure was repeated in Project 3 / Part 3 / Sketch 1 by deleting Horizontal, making two successive 90-degree Rotate copies, selecting all lines and choosing Parallel. It now solved without conflict, with four entities, two constraints and thirteen degrees of freedom. Undo returned to fifteen degrees of freedom and zero constraints; Redo restored the solved state. The temporary draft was cancelled. This confirms the previously failing three-line browser workflow now succeeds.

Concentric accepts multiple selected circles and arcs. It aligns each additional center with the first selected center in one Undo step while preserving independent radii and arc sweeps. Other entity types are rejected.

Live group Concentric evidence: in a temporary Project 3 / Part 3 / Sketch 1 draft, two Copy selected actions created circles offset by the default 10 mm translation. With Curves filtering and Select All, Concentric added two relations for the three circles, reducing degrees of freedom from twelve to eight. One Undo removed both relations; one Redo restored both and the solved state. The original line and its Horizontal relation remained present. The draft was cancelled.

Concentric also supports ellipses, including mixed circle/arc/ellipse groups. Only center coordinates are constrained; ellipse axis sizes and rotation remain independent.

Collinear is available in the toolbar and tool search. Select two or more lines to place them on the same supporting line in one Undo step. Segment endpoints and lengths remain free unless constrained separately; segments do not need to touch.

Live Collinear evidence: in Project 3 / Part 3 / Sketch 1, a line copy was offset by 10 mm X and 20 mm Y. With both lines selected using the Lines filter and Select All, searching Collinear and pressing Enter added collinear · constraint-2. Degrees of freedom decreased from nine to eight (the two lines already had horizontal directions). Undo removed the relation; Redo restored the solved state. The temporary draft was cancelled.

Equal, Parallel, Concentric and Collinear reuse existing driving relations of the same type, including reversed pairs and transitive connections through other entities. A group action adds one link per disconnected group; repeating a fully connected selection adds no relation or Undo step. This does not replace solver redundancy checks across different constraint types.

Live group-relation reuse evidence: three temporary circles in Project 3 / Part 3 / Sketch 1 were selected with the Curves filter and Select All. Applying Equal twice kept exactly two Equal relations. One Undo removed both, confirming the repeated command added no empty history step. The draft was cancelled. Transitive connections between separate existing groups retain automated coverage.

Horizontal and Vertical also align two explicitly selected anchors, or two selected point entities. They create a zero Y-distance or X-distance respectively, leaving spacing along the other axis free. Whole-line selections retain their orientation behavior.

Live point alignment evidence: an empty temporary sketch in Project 3 / Part 3 was given a point with Automatic constraints disabled, then a copy offset by 10 mm X and 20 mm Y. Selecting both and choosing Horizontal created a zero-distance relation and reduced degrees of freedom from four to three. Undo restored four degrees of freedom and no constraints; Redo restored the relation. Cancel sketch and Discard changes removed the temporary sketch. Vertical and explicit-anchor alignment retain automated coverage.

Point alignment accepts more than two point entities or explicitly selected anchors. Each additional anchor is aligned to the first in one Undo step, preserving spacing along the other axis.

Live group point alignment evidence: a temporary empty sketch in Project 3 / Part 3 received a free point and two successive copies offset by 10 mm X and 20 mm Y. Select All followed by Vertical created two zero-distance relations and reduced degrees of freedom from six to four. One Undo removed both; Redo restored both. Cancel sketch and Discard changes removed the temporary sketch.

## Native open splines

Choose **Fit spline** and click at least two fit points, or **Control spline** and click a start point, two handles and an end point (repeat two handles and an end for additional segments). Use **Finish spline** or Enter to commit the curve. Escape discards the unfinished placement. Select the spline to edit point coordinates or drag its point handles; local Undo/Redo covers creation and edits. Native points survive PCad serialization. Add connected geometry to close an extrusion region; an open spline alone produces no region. Spline offset, trim/split, curved constraints, closed/periodic modes remain pending.

Live verification (2026-09-09): in integrated-browser Project 3 / Part 3, Fit spline placement, middle-point coordinate editing, Undo/Redo and Finish with zero closed regions were verified. A fresh Control spline was then drawn and its endpoints connected with a line; Finish reported one closed region. The editable control handles and shaded region were inspected in [the captured workspace screenshot](../workdir/native-control-spline-browser.jpg). The control-spline sketch remains a draft: automatic approval review blocked saving the fixture to the server. Restoring the earlier local fit-spline browser copy was also blocked. PCad round-trip tests pass, but live server save/reload is unverified.

Selected control splines now show a dashed control polygon connecting the stored points. It cannot intercept clicks, and disappears on deselection. Coordinate-edit and Undo behavior are covered by the UI tests. Integrated-browser appearance is verified in [the control polygon screenshot](../workdir/spline-control-polygon-browser.jpg).

The compact toolbar groups Trim, Split, and Extend under Edit and Dimension under Constraints. The active tool appears on its corresponding menu label.

Driving dimensions retain arithmetic and units when reopened, including after local project serialization. Entering a plain number replaces the expression. Reference conversion and transforms that change the dimension value clear it; translation preserves it. Named variables and references to other dimensions are not yet supported.

Open Variables to add a definition, choose Length, Angle or Number, and enter an expression. Reference it in a driving dimension with `#name`, for example `#radius * 2`. Variable changes update geometry and support Undo/Redo. Invalid or conflicting changes are rejected. Renaming a variable updates its dependent expressions automatically. Deleting a referenced variable is rejected until its dependent expressions are changed.
