# Editing the flower holder in PuppyCAD

Open the saved project and choose a part in the project tree. All five reconstructed parts are editable through the **Solid features** panel next to the 3D preview.

## Part geometry

1. Select a feature from the ordered list.
2. Change its dimensions, operation, translation, or sketch support. Revolves expose angle and segment count; extrusions expose depth and top scale.
3. Expand the outline or a hole to resize or move it. Drag profile vertices or expand **Vertices** to enter exact coordinates. Revolve profile X is radius and Y is height; other profiles use the selected sketch plane. Circle and rectangle replacements, additional holes, and individual point insertion/removal are available.
4. Edit profile fillets/chamfers, including vertex indices (zero-based), radius/distance, and segment count. Extrusion source-edge finishes also expose their edge and distances.
5. Click **Apply changes**. A valid edit rebuilds the preview and saves the entire part as one undo step. Invalid geometry shows an error and leaves the last accepted part intact. **Discard changes** restores accepted values.

**Add extrusion**, **Add revolve**, **Move up/down**, and **Delete feature** change the feature sequence. A cut must follow a joined solid; a profile must remain closed. References to a removed source may need to be reattached before Apply. The preview shows the accepted solid while controls contain a draft. Use **Fit part** after orbiting or zooming.

The profile editor preserves the original polygon coordinates until they are edited. Circle replacement constructs a newly tessellated circle; resizing a revolve preserves its minimum radius and height. Apply verifies the complete solid with the same evaluator used for STL export. This does not expand the kernel's supported fillet or boolean cases.

## Assembly

Open the assembly to edit **Instances**, **Connectors**, and **Fixed connections**. Hold the **right mouse button and drag** in the 3D viewport to rotate; hold the **middle mouse button and drag** to pan; use the wheel to zoom. Right-drag suppresses the browser context menu. Primary touch/pen dragging also rotates.

- Instances choose a reusable part and carry position, rotation, and scale. **Edit part geometry** opens its definition. A change to that definition affects every instance.
- Connectors attach to an instance or to **World (ground)** and expose their position and rotation.
- Fixed connections join two connector frames. Connected instance poses are solved from those frames: change connector positions to adjust relative placement. Edit all affected connections in a closed loop before applying.
- Add/remove instances, connectors, and connections using the corresponding buttons. Removing an instance or connector also removes its dependent connections. Names used by references are updated when renamed.

Click **Apply assembly changes** to validate and save. Conflicting fixed constraints, missing references, and scaled fixed-connected instances produce an error without changing the accepted assembly. Only fixed connections are currently solved; motion joints remain outside this implementation.

## Saving and undo

Apply writes through the same project command API as the SDK. Server-backed projects save to disk and broadcast updates. **Undo** and **Redo** restore applied geometry and assembly edits; history lasts for the server process lifetime. Selection is local UI state and does not add a geometry undo step. Standalone/local projects use project snapshots for undo and local persistence.

UI editing changes the saved `.pcad` project, not `examples/flower-holder-parts.ts`. Rerunning the example will replace the corresponding definitions with its source dimensions. Use the assembly's part download buttons to export the currently accepted geometry as STL.

Rotation picks a pivot under the cursor once at the start of each drag. A surface hit supplies the pivot; an empty-space miss uses a point along the cursor ray at the default view distance (the part preview base distance, or three times the assembly bounding-box diagonal). The pivot stays at its screen position without recentering the view.
