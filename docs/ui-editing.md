# Project tree, viewer, and properties

PuppyCAD uses three columns: the full project hierarchy on the left, the active 3D viewer in the middle, and selection properties on the right. Solid parts expand into features, sketches, outlines/holes, and their line entities. Assemblies expand into instances, connectors, and connections; each instance links to its reusable part definition. The properties panel has no separate feature or instance list.

Select a tree row to inspect it. Enter selects a focused row; Left/Right collapse and expand folders. Clicking a supported border in the viewer reveals and selects its source entity in the project tree. Clicking an assembly part selects its instance. Selection does not save geometry or create an undo step.

## Part geometry

1. Select a feature in the project tree.
2. Change its dimensions, operation, translation, or sketch support. Revolves expose angle and segment count; extrusions expose depth and top scale.
3. Select its sketch, outline, or hole in the project tree to resize or move it. Drag profile vertices or select a line entity in the tree to edit its start/end coordinates. Revolve profile X is radius and Y is height; other profiles use the selected sketch plane. Circle and rectangle replacements, additional holes, and individual point insertion/removal are available.
4. Edit profile fillets/chamfers, including vertex indices (zero-based), radius/distance, and segment count. Extrusion source-edge finishes also expose their edge and distances.
5. Click **Apply changes**. A valid edit rebuilds the preview and saves the entire part as one undo step. Invalid geometry shows an error and leaves the last accepted part intact. **Discard changes** restores accepted values.

**Add extrusion**, **Add revolve**, **Move up/down**, and **Delete feature** change the feature sequence. A cut must follow a joined solid; a profile must remain closed. References to a removed source may need to be reattached before Apply. The preview shows the accepted solid while controls contain a draft. Use **Fit part** after orbiting, panning, or zooming; it recenters the geometry and fits the current viewport width and height.

The profile editor preserves the original polygon coordinates until they are edited. Circle replacement constructs a newly tessellated circle; resizing a revolve preserves its minimum radius and height. Apply verifies the complete solid with the same evaluator used for STL export. This does not expand the kernel's supported fillet or boolean cases.

## Assembly

Open the assembly to edit **Instances**, **Connectors**, and **Fixed connections**. Click a part in the 3D viewport to highlight its instance, reveal it in the project tree, and show its properties on the right. Instances can also be selected with the keyboard in the project tree. Click empty space or press Escape in the viewport to clear selection. Use **Edit part geometry** in the selected instance to open its part definition. Hold the **right mouse button and drag** in the 3D viewport to rotate; hold the **middle mouse button and drag** to pan; use the wheel to zoom toward the cursor. Zoom supports close detail inspection and distant overviews, with only broad numerical safety bounds. **Fit assembly** (F while the viewport is focused) restores the whole model; **Fit selected part** (Shift+F) frames the selected instance. Both preserve the viewing angle. The +/− keys zoom around the viewport center. Right-drag suppresses the browser context menu. Primary touch/pen dragging also rotates.

- Instances choose a reusable part and carry position, rotation, and scale. **Edit part geometry** opens its definition. A change to that definition affects every instance.
- Connectors attach to an instance or to **World (ground)** and expose their position and rotation.
- Fixed connections join two connector frames. Connected instance poses are solved from those frames: change connector positions to adjust relative placement. Edit all affected connections in a closed loop before applying.
- Add/remove instances, connectors, and connections using the corresponding buttons. Removing an instance or connector also removes its dependent connections. Names used by references are updated when renamed.

Click **Apply assembly changes** to validate and save. Conflicting fixed constraints, missing references, and scaled fixed-connected instances produce an error without changing the accepted assembly. Only fixed connections are currently solved; motion joints remain outside this implementation.

## Saving and undo

Apply writes through the same project command API as the SDK. Server-backed projects save to disk and broadcast updates. **Undo** and **Redo** restore applied geometry and assembly edits; history lasts for the server process lifetime. Selection is local UI state and does not add a geometry undo step. Standalone/local projects use project snapshots for undo and local persistence.

UI editing changes the saved `.pcad` project, not `examples/flower-holder-parts.ts`. Rerunning the example will replace the corresponding definitions with its source dimensions. Use the assembly's part download buttons to export the currently accepted geometry as STL.

Rotation picks a pivot under the cursor once at the start of each drag. A surface hit supplies the pivot; an empty-space miss uses a point along the cursor ray at the default view distance (the part preview base distance, or three times the assembly bounding-box diagonal). The pivot stays at its screen position without recentering the view.

## Projection

Both part previews and assembly views have a **Projection** selector. **Perspective** shows depth with nearer features appearing larger; **Orthographic** keeps parallel edges parallel. Orthographic close-ups change magnification while keeping the whole part inside the camera depth range, so zoom does not slice through it. Switching preserves the viewing direction and scale at the view target. Orbit, pan, zoom, selection, and Fit work in both modes. Part projection is saved with its local view state; assembly projection is remembered on this browser.

## Selecting a sketch from a solid

Hover over a selectable border to preview its highlight without changing the sketch panel. Click to keep the selection and open its sketch. Moving away clears the hover preview while preserving a clicked selection. The orange highlight follows the nearest rim edges of the evaluated mesh, including when the click lands just inside a hole wall. Click an extrusion outline or hole border in the part preview to highlight its source profile in orange and reveal the creating feature’s sketch entity in the project tree and show its controls on the right. A hole made by a boolean cut selects the cutting feature’s outline. The panel identifies the sketch and source line segment, highlights that segment in the profile preview, and preserves pending draft edits. Current picking supports straight, untapered extrusion profiles, including translated and face-attached features; revolved and finished edges do not yet have source picking. Tessellated circles expose their actual line segments.

Part orbit, pan, and wheel zoom batch viewport draws to one per animation frame. Camera state is saved after 150 ms of inactivity or when a drag ends or the editor closes. Camera movement does not rebuild the project tree; visibility and model changes still refresh it.
