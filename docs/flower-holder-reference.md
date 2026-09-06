# Flower holder reference reconstruction

The authoritative source is `test/fixtures/bowl_with_flowerholder.gltf`, copied unchanged from the supplied `workdir/bowl_with_flowerholder.gltf`.

SHA-256: `ee97462f49afefe778329d85f93d643a0a96d5114ff87c7fb0b1e12f911e00ba`.

The glTF positions are in metres; the DSL uses millimetres. Five meshes correspond to five printable definitions, not the four-part simplified demonstration previously used. `examples/flower-holder-parts.ts` holds the measured dimensions and feature sequences. `examples/flower-holder-live.ts` builds the assembly.

| Part | Reconstructed features |
| --- | --- |
| Bowl | 100 mm height, outer base/rim radii 85/112.5 mm, 2.5 mm floor and conical-wall thickness. One revolved shell. |
| Upper ring | 97/113 mm radii, 5 mm thick, open quadrant, three radius-7 holder holes, four 60 mm integral legs of radius 4.95 mm at cardinal points on a 105 mm radius. |
| Lower ring | 25 mm high inner wall at radii 97–100 and outer wall at 110–113, four 5 mm bridge plates at Z=20, radius-5 leg sockets, measured opening and bridge edge planes. |
| Centre holder | Hub at `(45/√2, 45/√2)`, radius 16, radius-6.65 stem hole, three 20 mm wide capsule arms to three diagonal points on radius 105. Plate thickness 8, pin radius 6.475, pin length 10. |
| Stem | Shaft radius 8, shoulder radius 11, bottom connector radius 6.475, 0.8 mm connector chamfer, radius-3 rounded tip, five diameter-5 cross-holes and a diameter-3.5 connector hole. Total length 264 mm. |

The holder is deliberately offset from the rings' centre. Its arms are not three equally spaced radial spokes. The rings' openings are also not an idealized exact 90-degree sector: their original asymmetric cut planes are retained.

## Assembly and printing

The reference contains a partially exploded arrangement. Its bowl is translated down and sideways relative to the ring stack. The DSL preserves that arrangement instead of inventing a bowl-to-ring mate. The two rings, holder and stem are connected by nine fixed constraints, including a world anchor and consistent closed loops. Each part is translated within its definition so that its minimum Z is zero; assembly transforms compensate for that translation.

Run:

```sh
bun dev
bun examples/flower-holder-live.ts
```

Use the returned project ID on subsequent runs. The assembly viewer offers one STL for each part; print one of each. The upper ring's four legs and the holder's three pins are integral features, not additional assembly instances.

## Verification

`test/flower-holder-reference.test.ts` loads the original glTF and reconstructs each part from the DSL. It compares **all unique vertices plus 2,000 area-weighted samples on each surface in both directions**, using point-to-triangle distance through a bounding-volume tree. This catches missing holes, altered parts, and excess material rather than relying on bounding boxes alone.

| Part | Largest observed sampled surface difference |
| --- | ---: |
| Bowl | 0.114 mm |
| Lower ring | 0.119 mm |
| Upper ring | 0.118 mm |
| Centre holder | 0.024 mm |
| Stem | 0.017 mm |

The regression limit is 0.13 mm. The original glTF's coarse tessellation of large circles accounts for most of the deviation; the new model uses finer tessellation of the recovered analytic shapes. This is a sampled distance check, not a mathematical bound on continuous Hausdorff distance.

Tests additionally check closed edge incidence, positive signed volume, volume agreement within 0.6%, and assembly translations within 0.001 mm. The SDK integration test verifies persistence, separate closed STLs and deterministic reruns. Matched-camera reference/DSL renders are saved under `workdir/reference-analysis/`.

These are digital fidelity checks. Physical fit, material strength and watertight printing have not been tested. No new fit dimensions were substituted for the source design.
