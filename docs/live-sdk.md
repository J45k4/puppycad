# Live TypeScript SDK

The SDK edits a running PuppyCAD project through the same command API used by the GUI. The server saves each accepted operation in `workdir/saved-projects/<project-id>.json` and broadcasts the resulting project to connected viewers. The server project is the source of truth for this workflow.

Start the server with `bun dev`. In another terminal:

```sh
bun examples/flower-holder-live.ts
```

The script prints a viewer URL and project ID. Open that URL while designing. Rerun against the same project:

```sh
bun examples/flower-holder-live.ts <project-id>
```

The example reconstructs the original `bowl_with_flowerholder.gltf`: a tapered bowl, upper ring with four integral legs, lower channel ring, offset three-arm centre holder, and perforated 250 mm stem (264 mm including its connector). Five reusable parts form five instances with nine fixed constraints. It preserves the reference file's partially exploded pose, including the offset bowl. Each part exports individually with its lowest point at local Z=0.

See [reference measurements and verification](flower-holder-reference.md) for dimensions, tolerances, and the original fixture. The model is authored with revolves, fillets, extrusions and boolean cuts; it does not import the reference triangles as its output.

The implementation contract and its remaining limits are maintained in `spec/puppycad.tex`. Render its PDF with `bash spec/render.sh`.

## Editing in the UI

Open any reconstructed part to use the **Solid features** panel. Profiles, holes, dimensions, operations, fillets, chamfers, and feature order are editable without TypeScript. The assembly panel edits instances, connector frames, and fixed connections. Use **Apply changes** or **Apply assembly changes** to validate and save one undoable edit; **Discard** abandons the draft.

See [UI editing](ui-editing.md) for the workflow. UI edits change the saved project, not the example source file. Rerunning the example replaces those part definitions with the source dimensions.

## Parts and assembly instances

```ts
import { PuppyCad, circle, rectangle, v2 } from "puppycad/sdk"

const cad = new PuppyCad({ serverUrl: "http://localhost:5337" })
const project = await cad.createProject()
// For subsequent runs: const project = await cad.openProject("existing-id")
console.log(project.viewerUrl)

const base = await project.part("base", {
  name: "Base",
  outline: rectangle(v2(0, 0), 80, 40),
  depth: 5
})
const post = await project.part("post", {
  name: "Post",
  outline: circle(v2(0, 0), 4),
  depth: 60
})

await project.assembly("fixture", (assembly) => {
  const plate = assembly.instance("base", base)
  const upright = assembly.instance("upright", post)
  const seat = assembly.connector("seat", plate, { x: 20, y: 0, z: 5 })
  const foot = assembly.connector("foot", upright, { x: 0, y: 0, z: 0 })
  assembly.fasten("post-on-base", seat, foot)
}, "Fixture")

await Bun.write("post.stl", await project.exportStl(post))
```

Part geometry stays in its local coordinate system. Instances reference parts by ID and carry their own translation, rotation, and scale. Changing a part updates every instance that references it. STL export uses the part coordinates, so assembly placement does not affect printing.

Dimensions are millimetres. Instance and connector rotations are Euler XYZ angles in degrees. Explicit placement is also supported:

```ts
assembly.instance("second-post", post, {
  translation: { x: -20, y: 0, z: 5 },
  rotation: { x: 0, y: 0, z: 90 }
})
```

`fasten` aligns complete connector frames and records a fixed constraint. Mate order is independent: the solver traverses the graph, including consistent closed loops. Conflicting loops reject the update. World connectors ground a component; otherwise its root pose is retained. Fixed constraints require unscaled instances.

Persisted fixed mates are solved on load and display. Changing connector coordinates updates the assembled pose. Connectors are explicit coordinates, so rerun the script when part dimensions change their intended locations. `solveFixedAssembly(assembly)` is also exported for direct evaluation. Revolute motion and mixed-joint/dynamics solving are not implemented.

## Holes and feature sequences

A single part can contain multiple sketch/extrusion features:

```ts
await project.part("cup", (part) => {
  const top = part.extrude("base", {
    outline: circle(v2(0, 0), 30),
    depth: 3
  })
  part.extrude("wall", {
    outline: circle(v2(0, 0), 30),
    holes: [circle(v2(0, 0), 27)],
    depth: 40,
    on: top
  })
})
```

The callback builds a complete part synchronously; it is sent as one command when the callback returns. An extrusion returns a reference to its top face for the next sketch. Through holes must be closed loops inside the outline.

The part and assembly viewers, CLI geometry queries/PNG rendering, and STL exporter share an evaluated solid boundary. SDK part previews frame the whole solid on first open and provide a **Fit part** button. Multi-feature parts export after union; cuts and intersections apply in order. Separate disconnected shells are allowed, so export success alone does not prove physical connectivity.

## Revolves, tapers, cuts, and edge finishes

```ts
await project.part("bowl", (part) => {
  // Profile x = radius, y = height; revolves about local Z.
  part.revolve("vessel", {
    outline: [v2(0, 0), v2(50, 0), v2(75, 55),
              v2(72, 55), v2(47, 4), v2(0, 4)],
    segments: 96
  })
  part.fillet("vessel", 0.8, [1, 2, 3, 4])
  part.extrude("socket", {
    outline: circle(v2(40, 0), 4.05, 32),
    depth: 3.1,
    translation: { x: 0, y: 0, z: 1 },
    operation: "cut"
  })
})
```

- `extrude` supports `operation: "join" | "cut" | "intersect"` (default `join`), positive `topScale` for a linear taper, and `translation` in part coordinates. Face attachment refers to the source extrusion, not a face selected from boolean output, and inherits the source chain's explicit translations.
- `revolve` accepts a closed radius/height `outline`, `angle` in `(0, 360]` degrees (default 360), `segments` from 8 to 1024 (default 96), an operation, and a translation. Partial turns have end caps.
- `fillet(featureId, radius, vertices?, segments = 8)` rounds selected profile corners with tangent arcs. It supports longitudinal extrusion edges and circular edges of revolved profiles. Omitting vertices selects all corners. Excessive radii and degenerate corners are rejected.
- `chamfer(featureId, distance, vertices?)` trims selected profile corners.
- `filletEdge(featureId, edgeId, radius, segments = 8)` rounds a convex straight source-extrusion edge using tangent cuts. Obtain source ids with `extrudeSolidFeature`. Existing GUI source-edge chamfers also render/export, including unequal distances.

These are tessellated solids. General concave/interacting edge fillets and selecting arbitrary boolean-output edges remain unsupported. STEP/GLB export and motion simulation are not implemented. Curvature segment counts control approximation quality; validate manufacturing dimensions and fit with a trial print.

The SDK persists its ordered `solidSteps` recipe alongside sketch/extrusion features. GUI saves retain it; advanced solid features are authored in TypeScript. These features have not yet been lowered into the general PCad graph vocabulary.

## Updates and transactions

- `part(id, ...)` creates or replaces that part definition. `assembly(id, ...)` creates or replaces that assembly definition.
- Stable IDs prevent duplicates when a script is rerun. Other documents are retained, including their folder placement and visibility.
- Replacing a definition replaces its contents, including manual edits within that same definition. Concurrent edits to the same definition use the last accepted command.
- Removing a part from a script does not delete it from the server. Use explicit project commands when deletion is intended.
- Each awaited call is one server revision, file save, undo step, and live event. `project.commands([...])` submits a batch atomically against the current server state. Invalid batches do not change the saved project.
- Whole-project snapshot saves reject stale revisions with HTTP 409, so an older browser snapshot cannot overwrite newer SDK edits. Reload before retrying a conflicting full snapshot save.
- `openProject` requires an existing project. `createProject` allocates a new server ID. `refresh()` reads current state; `snapshot` returns the last response seen by that SDK handle.
- Undo history remains in server memory and is lost on restart. Accepted project data persists on disk.

The SDK has no file watcher. Use Bun's watch mode for script reruns if wanted, always supplying the existing project ID so each restart targets the same project.

## Existing model DSL

Existing `defineModel` and `component` graphs still work:

```ts
import model from "./my-model.pcad.ts"
await project.model(model)
```

This compiles the model and sends all its part/assembly definitions as one atomic batch. It preserves the existing compiler's flattened body coordinates. For reusable parts with independent print coordinates and separate assembly placement, use `project.part` and `project.assembly` as above.
