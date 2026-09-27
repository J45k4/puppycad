# Flowergirl reconstruction through the UI

The project **Flowergirl — built through UI** (`54eed525-6fb5-4e01-85d4-135f71a92ce2`) was created with the integrated browser's native PuppyCAD controls. Its five part definitions, five assembly instances, 18 connectors and nine fixed connections were entered through the UI. Saved model files were read only for comparison; they were not used to populate the reconstruction.

Reference project: `30fbfab4-72f1-4696-8a0c-e7194cad084d`, revision 39. Reconstruction verified at revision 38. The reference remains unchanged.

## Observed Onshape workflow

- Bowl: 170 mm circle, plane offset 100 mm, 225 mm circle, loft, then a 2.5 mm shell with the top face removed.
- Ring and holder: annulus extrusion, four legs, through-all cuts; three capsule arms in one sketch, hub, pins and bore.
- Base: 25 mm annular blank, through-all channel cut, four regions extruded 5 mm to form bridges, socket holes using Up to next, Boolean Union, then a symmetric 100 mm access cut.
- Stem: an imported part only; no original sketch/feature history is available.
- Assembly: fastened mates between the parts.

The original was inspected in document `3d75cb6335dab14aac3c04a3`, workspace `e8907bdb781971b71eb1b7af`. Feature dialogs were closed without modifying dimensions.

## PuppyCAD workflow and differences

The bowl uses separate loft and shell operations. The circular sections and plane offset are properties of the loft, rather than independent sketch/plane documents. The holder uses multiple filled capsule and circle regions in shared extrusions. Ring legs and holes also use shared sketches.

The lower ring follows blank, channel, bridges, socket holes and access-cut order. Its four polygon bridges currently require four extrusions. Their ends are trimmed by annular intersection; PuppyCAD combines the resulting solids as the steps are evaluated rather than exposing the original two-body Boolean Union step. Through-all cuts replace the socket Up to next condition. These are equivalent for this part, but are not identical feature histories.

The imported stem is reconstructed as a nine-vertex revolve with a tip fillet and one six-region transverse hole cut. The assembly reproduces the saved reference's connector frames and nine fixed constraints. Its bowl retains the reference layout's separate offset position.

## Verification

All five parts and the assembly were reopened in the browser after an explicit server save. All connector and mate records match the reference exactly; the fixed-constraint solver reproduces all five reference positions with zero difference. Part bounds differ by less than 0.0002 mm. Boolean triangulation and grouping produce small volume differences; the largest observed net volume difference is 0.562 mm³ on the centre holder. This is a software geometry comparison, not physical print validation.

Evidence in `workdir/`: `ui-final-verification.json`, `ui-holder-comparison.json`, `ui-upper-ring-comparison.json`, `ui-lower-ring-comparison.json`, `ui-stem-comparison.json`, and `ui-complete-flowergirl-assembly.png`. Individual part screenshots are also saved there.

An interrupted development server exposed silent synchronization failures during assembly entry. The UI now warns when commands fail to reach the server, checkpoints command edits locally, and retains a differing browser copy for explicit review and recovery. See [UI editing](ui-editing.md) for the recovery flow.

## Live sketch-support recheck

The integrated browser reconfirmed the original Base - three-arm version workflow: Sketch 1 uses the Top plane and shows a Ø210 construction circle with a 16 mm ring-width dimension. Extrude 1 references Face of Sketch 1 and has a 25 mm blind depth. Sketch 2 and Sketch 3 both use Face of Extrude 1 as their sketch plane. Extrude 2 references Face of Sketch 2, uses Through all, and lists Part of Extrude 1 in its merge scope. The operation tab was not independently confirmed during this recheck. All inspection dialogs were cancelled without changing the original.

This identified a concrete new-sketch UI gap: the existing face-reference machinery was exposed for editing extrusion support but not for creating a standalone sketch. The New sketch plane selector now also offers face descriptors from existing extrusion steps. A UI test verifies creation on a top face and downstream frame movement when the source depth changes from 25 to 30 mm. Direct viewport face picking and in-context model display remain separate unfinished requirements.

### Current implementation boundary for viewport face picking

The current solid preview selects source profile edges through `PartEditor.selectSolidSourceAt` and `pickSolidSketchSource`; this does not identify a face support. New sketch face choices are maintained separately in `SolidFeaturePanel`. The older `PartEditor.enterSketchMode` face-selection path must not be treated as proof of the new workspace workflow. The required connection is an evaluated planar-face hit to a validated sketch target in the solid-panel lifecycle, followed by source-depth and reload verification.
