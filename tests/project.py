"""Run the native Flexscript editor's graph/geometry contract against real v4 files."""
import copy
import collections
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "rust/fixtures"


def triangulation(path):
    raw = path.read_bytes()
    count, = struct.unpack_from("<I", raw, 80)
    assert count > 0 and len(raw) == 84 + 50 * count
    faces = ([record[k:k+3] for k in (3, 6, 9)] for record in struct.iter_unpack("<12fH", raw[84:]))
    return solid(faces)


def solid(faces):
    points, volumes, edges = [], [], collections.Counter()
    for a, b, c in faces:
        points.extend((a, b, c))
        assert a != b and b != c and c != a
        for start, end in ((a, b), (b, c), (c, a)):
            edges[start, end] += 1
        volumes.append(sum(a[i] * (b[(i+1)%3]*c[(i+2)%3] - b[(i+2)%3]*c[(i+1)%3]) for i in range(3)) / 6)
    assert all(n == 1 and edges[end, start] == 1 for (start, end), n in edges.items())
    return [(min(k), max(k)) for k in zip(*points)], math.fsum(volumes)


def package_geometry(path):
    core = "{http://schemas.microsoft.com/3dmanufacturing/core/2015/02}"
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None
        types = ET.fromstring(archive.read("[Content_Types].xml"))
        defaults = {item.attrib["Extension"]: item.attrib["ContentType"] for item in types}
        assert defaults["model"] == "application/vnd.ms-package.3dmanufacturing-3dmodel+xml"
        assert defaults["rels"] == "application/vnd.openxmlformats-package.relationships+xml"
        links = ET.fromstring(archive.read("_rels/.rels"))
        targets = [link.attrib["Target"] for link in links if link.attrib["Type"].endswith("/3dmodel")]
        assert len(targets) == 1
        model = ET.fromstring(archive.read(targets[0].lstrip("/")))
    assert model.tag == core+"model" and model.attrib["unit"] == "millimeter"
    metadata = {entry.attrib["name"]: entry.text for entry in model.findall(core+"metadata")}
    bodies = {}
    for obj in model.find(core+"resources"):
        assert obj.attrib["type"] == "model" and obj.attrib["id"] not in bodies
        mesh = obj.find(core+"mesh")
        vertices = [tuple(float(v.attrib[k]) for k in "xyz") for v in mesh.find(core+"vertices")]
        faces = []
        used = set()
        for tri in mesh.find(core+"triangles"):
            indices = [int(tri.attrib[k]) for k in ("v1", "v2", "v3")]
            assert all(0 <= index < len(vertices) for index in indices)
            used.update(indices)
            faces.append([vertices[index] for index in indices])
        assert len(used) == len(vertices)
        bodies[obj.attrib["id"]] = (obj.attrib["name"], solid(faces))
    build = [entry.attrib["objectid"] for entry in model.find(core+"build")]
    assert len(build) == len(set(build)) == len(bodies)
    return metadata["Title"], [bodies[key] for key in build]


def check_profiles(editor, directory):
    input_file = directory / "profiles.pcad"
    output = directory / "profiles.stl"

    def check(model, expected_bounds, volume):
        subprocess.run([editor, "--export", model, output], check=True)
        actual_bounds, actual_volume = triangulation(output)
        for bounds, expected in zip(actual_bounds, expected_bounds):
            assert all(math.isclose(x, y, abs_tol=0.001) for x, y in zip(bounds, expected)), actual_bounds
        assert math.isclose(actual_volume, volume, rel_tol=0.00001), actual_volume

    # Polygon areas use the segment counts saved in the drawings.
    ellipse = 16 / 2 * 80 * 50 * math.sin(math.tau / 16)
    hole = 12 / 2 * 20**2 * math.sin(math.tau / 12)
    check(FIXTURES / "segmented-primitives.pcad", [(-80, 80), (-50, 50), (0, 40)], 40*(ellipse-hole))
    # Every sampled rim vertex must agree with Rust even for a highly eccentric
    # ellipse. A millidegree truncation shifts its tips by several micrometres.
    eccentric = json.loads((FIXTURES/"segmented-primitives.pcad").read_bytes())
    nodes = eccentric["items"][0]["data"]["cad"]["nodes"]
    outer, hole_node = [n for n in nodes if n["type"]=="sketchPrimitive"]
    nodes.remove(hole_node)
    for rotation, segments in ((31.291983,128),(-47.123783,43),(122.4359,128),(1.00095,33)):
        outer["primitive"].update(width=420,height=18,rotation=rotation,segments=segments)
        input_file.write_text(json.dumps(eccentric))
        samples = []
        for exe in (editor, ROOT/"rust/target/debug/puppycad"):
            subprocess.run([exe,"--export",input_file,output],check=True)
            vertices = set()
            for face in struct.iter_unpack("<12fH",output.read_bytes()[84:]):
                if max(face[5],face[8],face[11])>0:
                    vertices.update(tuple(face[k:k+3]) for k in (3,6,9) if face[k+2]==0)
            assert len(vertices)==segments
            samples.append(vertices)
        angle = math.radians(rotation)
        expected = []
        for i in range(segments):
            x, y = 210*math.cos(math.tau*i/segments), 9*math.sin(math.tau*i/segments)
            expected.append((x*math.cos(angle)-y*math.sin(angle), x*math.sin(angle)+y*math.cos(angle), 0))
        for pt in samples[0]:
            # Each STL vertex is float32. Rust also computes its angles in float32.
            assert min(math.dist(pt,ref) for ref in expected) < .00002, (rotation,segments,pt)
            assert min(math.dist(pt,ref) for ref in samples[1]) < .00015, (rotation,segments,pt)
    capsule = 100*40 + 12*20**2*math.sin(math.pi/12)
    check(FIXTURES / "segmented-slot.pcad", [(-70, 70), (-20, 20), (0, 40)], 40*capsule)

    slot_doc = json.loads((FIXTURES / "segmented-slot.pcad").read_bytes())
    slot_nodes = slot_doc["items"][0]["data"]["cad"]["nodes"]
    slot = slot_nodes[4]["primitive"]
    slot.update({"from": {"x": 10, "y": -30}, "to": {"x": 10, "y": 70}})
    input_file.write_text(json.dumps(slot_doc))
    check(input_file, [(-10, 30), (-50, 90), (0, 40)], 40*capsule)
    slot_nodes[4]["primitive"] = {
        "type": "rectangle", "id": slot["id"], "rotation": 45,
        "center": {"x": 14, "y": -4}, "width": 40, "height": 20,
    }
    input_file.write_text(json.dumps(slot_doc))
    r = 30 * math.sqrt(0.5)
    check(input_file, [(14-r, 14+r), (-4-r, -4+r), (0, 40)], 40*800)

    doc = json.loads((FIXTURES / "segmented-primitives.pcad").read_bytes())
    nodes = doc["items"][0]["data"]["cad"]["nodes"]
    sketch, outer, inner, feature = nodes[3:7]
    outer["primitive"]["rotation"] = 90
    sketch["targetId"] = "plane-front"
    # Store the hole before its container and add an island and a separate region.
    nodes[4:6] = [inner, outer]
    island = copy.deepcopy(inner)
    island.update(id="island")
    island["primitive"].update(id="island", radius=5)
    nodes.append(island)
    separate = copy.deepcopy(island)
    separate.update(id="other")
    separate["primitive"].update(id="other", type="polygon", rotation=0, sides=4, center={"x": 110, "y": 0})
    nodes.append(separate)
    for index, bounds, area in [
        (1, [(-50, 50), (0, 40), (-80, 80)], ellipse-hole),
        (2, [(-5, 5), (0, 40), (-5, 5)], hole/16),
        (3, [(105, 115), (0, 40), (-5, 5)], 50),
    ]:
        feature["profileId"] = sketch["id"] + f"-profile-{index}"
        input_file.write_text(json.dumps(doc))
        check(input_file, bounds, 40*area)
    # Inner circle intersects the ellipse. Subdivide into selectable closed cuts,
    # preserve the previous export when an invalid profile is requested, and ignore construction geometry.
    inner["primitive"]["center"]["x"] = 45
    cut_volumes = []
    for index in (1, 2, 3):
        feature["profileId"] = sketch["id"] + f"-profile-{index}"
        input_file.write_text(json.dumps(doc))
        subprocess.run([editor, "--export", input_file, output], check=True)
        _, volume = triangulation(output)
        cut_volumes.append(volume)
    assert all(v > 1000 for v in cut_volumes), cut_volumes
    feature["profileId"] = sketch["id"] + "-profile-99"
    input_file.write_text(json.dumps(doc))
    previous = output.read_bytes()
    result = subprocess.run([editor, "--export", input_file, output], capture_output=True)
    assert result.returncode and b"not found" in result.stderr
    assert output.read_bytes() == previous
    feature["profileId"] = sketch["id"] + "-profile-1"
    for node in [inner, island, separate]:
        node["primitive"]["construction"] = True
    input_file.write_text(json.dumps(doc))
    check(input_file, [(-50, 50), (0, 40), (-80, 80)], 40*ellipse)


def revolve_document(angle=360, axis=((0, 0), (0, 1)), hole=False):
    doc = json.loads((FIXTURES / "revolve-loft-finish.pcad").read_bytes())
    data = doc["items"][0]["data"]
    graph = data["cad"]
    nodes = graph["nodes"]
    excluded = {n["id"] for n in nodes if n["type"] in ("edgeFinish", "loft", "face")
                or n["id"] == "native-sketch-3" or n.get("sketchId") == "native-sketch-3"}
    nodes[:] = [n for n in nodes if n["id"] not in excluded]
    for owner, key in [(graph, "rootNodeIds"), (data["tree"], "orderedNodeIds")]:
        owner[key] = [name for name in owner[key] if name not in excluded]
    feature = next(n for n in nodes if n["type"] == "revolve")
    feature.update(angle=angle, axis={k: dict(zip("xy", xy)) for k, xy in zip("ab", axis)})
    if hole:
        nodes.append({"type": "sketchCornerRectangle", "id": "inner-loop", "sketchId": feature["sketchId"],
                      "p0": {"x": 43, "y": -7}, "p1": {"x": 47, "y": -3}})
    return doc


def compare_solid_packages(actual, reference):
    title, bodies = package_geometry(actual)
    expected_title, expected_bodies = package_geometry(reference)
    assert title == expected_title and [b[0] for b in bodies] == [b[0] for b in expected_bodies]
    for (_, (bounds, volume)), (_, (expected_bounds, expected_volume)) in zip(bodies, expected_bodies):
        for pair, expected in zip(bounds, expected_bounds):
            assert all(math.isclose(a, b, abs_tol=.0001) for a, b in zip(pair, expected)), (bounds, expected_bounds)
        assert volume > 0 and math.isclose(volume, expected_volume, rel_tol=.00001), (volume, expected_volume)


def loft_document(with_loft=True):
    drawing = json.loads((FIXTURES / "revolve-loft-finish.pcad").read_bytes())
    data = drawing["items"][0]["data"]
    excluded = {"native-edge-finish-1", "native-revolve-1", "native-sketch-2", "native-entity-1-2"}
    if not with_loft:
        excluded.add("native-loft-1")
    graph = data["cad"]
    graph["nodes"] = [n for n in graph["nodes"] if n["id"] not in excluded]
    for owner, key in [(graph, "rootNodeIds"), (data["tree"], "orderedNodeIds")]:
        owner[key] = [i for i in owner[key] if i not in excluded]
    return drawing


def edge_document(kind=None, size=3, multiple=False, stepped=False):
    doc = loft_document(with_loft=False)
    data = doc["items"][0]["data"]
    graph = data["cad"]
    nodes = graph["nodes"]
    if stepped:
        top = next(n for n in nodes if n["id"] == "native-entity-1-3")
        top.pop("primitive")
        top.update(type="sketchCornerRectangle", p0=dict(x=0,y=0), p1=dict(x=10,y=20))
        join = dict(id="raised", name="Step", type="extrude", operation="join", depth=10,
                    sketchId=top["sketchId"], profileId=top["sketchId"]+"-profile-1")
        nodes.append(join)
        for owner,key in [(graph,"rootNodeIds"),(data["tree"],"orderedNodeIds")]:
            owner[key].append(join["id"])
    else:
        nodes[:] = [n for n in nodes if n["type"] != "face" and "native-sketch-3" not in (n["id"],n.get("sketchId"))]
        for owner,key in [(graph,"rootNodeIds"),(data["tree"],"orderedNodeIds")]:
            owner[key] = [n for n in owner[key] if n in {node["id"] for node in nodes}]
    if kind:
        edges = [dict(a=[0,0,20],b=[20,0,20],normals=[[0,-1,0],[0,0,1]])]
        if multiple:
            edges.append(dict(a=[0,0,20],b=[0,20,20],normals=[[-1,0,0],[0,0,1]]))
        if stepped:
            edges = [dict(a=[10,0,20],b=[10,20,20],normals=[[1,0,0],[0,0,1]])]
        finish = dict(id="native-edge-finish-1", name=kind.title()+" 1", type="edgeFinish", kind=kind, edges=edges)
        finish["radius" if kind=="fillet" else "distance"] = size
        nodes.append(finish)
        graph["rootNodeIds"].append(finish["id"])
        data["tree"]["orderedNodeIds"].append(finish["id"])
    return doc


def check_edge_finishes(editor, directory):
    source, output, reference = (directory/name for name in ("finish.pcad","finish.3mf","reference-finish.3mf"))
    def compare():
        source.write_text(json.dumps(doc))
        subprocess.run([editor, "--export", source, output], check=True)
        subprocess.run([ROOT/"rust/target/debug/puppycad", "--export", source, reference], check=True)
        compare_solid_packages(output, reference)
        return package_geometry(output)[1]
    doc = json.loads((FIXTURES/"revolve-loft-finish.pcad").read_bytes())
    solids = compare()
    assert len(solids)==3 and abs(solids[0][1][1]-7910)<.001
    step=directory/"combined.step"
    subprocess.run([editor,"--export",source,step],check=True)
    if os.environ.get("PUPPYCAD_TEST_FREECAD"):
        import FreeCAD
        import Part
        imported=Part.read(str(step)).Solids
        assert len(imported)==len(solids)
        for solid,(_,(_,volume)) in zip(imported,solids):
            assert solid.isValid() and math.isclose(solid.Volume,volume,rel_tol=.00001)
    # Finishing an older block must leave a newer separate extrusion active.
    doc=edge_document("chamfer")
    data=doc["items"][0]["data"]
    graph=data["cad"];finish=graph["nodes"].pop()
    original=next(n for n in graph["nodes"] if n["type"]=="extrude")
    rect=next(n for n in graph["nodes"] if n["type"]=="sketchCornerRectangle")
    sk=next(n for n in graph["nodes"] if n["type"]=="sketch")
    second_sketch=dict(sk,id="other-sketch")
    second_rect=dict(rect,id="other-rect",sketchId="other-sketch",p0=dict(x=30,y=0),p1=dict(x=40,y=20))
    other=dict(original,id="other",name="Other",sketchId="other-sketch",profileId="other-sketch-profile-1",depth=10)
    above=dict(other,id="higher",name="Joined",depth=12,operation="join")
    graph["nodes"].extend([second_sketch,second_rect,other,finish,above])
    for owner,key in [(graph,"rootNodeIds"),(data["tree"],"orderedNodeIds")]:
        owner[key][-1:]=["other-sketch","other",finish["id"],"higher"]
    first,last=compare()
    assert abs(first[1][1]-7910)<.001 and abs(last[1][1]-2400)<.001
    for kind in ["chamfer","fillet"]:
        for stepped, multiple in [(False,False),(False,True),(True,False)]:
            doc = edge_document(kind,2.125,multiple,stepped)
            body, = compare()
            if stepped:
                assert body[1][1]>10000
            else:
                assert body[1][1]<8000
    # Reflect both selected edges to the opposite corner. The original Rust
    # exporter can skip this feature there, so use the symmetry of the cube.
    doc = edge_document("fillet", 2.125, multiple=True)
    original_body, = compare()
    edges = doc["items"][0]["data"]["cad"]["nodes"][-1]["edges"]
    for edge in edges:
        for key in ("a", "b"):
            x, y, z = edge[key]
            edge[key] = [20-x, 20-y, z]
        edge["normals"] = [[-x, -y, z] for x, y, z in edge["normals"]]
    source.write_text(json.dumps(doc))
    subprocess.run([editor, "--export", source, output], check=True)
    mirrored_body, = package_geometry(output)[1]
    assert mirrored_body[1][0] == original_body[1][0]
    assert math.isclose(mirrored_body[1][1], original_body[1][1], abs_tol=.001)
    doc=edge_document("fillet")
    nodes=doc["items"][0]["data"]["cad"]["nodes"]
    rect=next(n for n in nodes if n["type"]=="sketchCornerRectangle")
    rect["p0"]["x"]+=480;rect["p1"]["x"]+=480
    selected=nodes[-1]["edges"][0]
    selected["a"][0]+=480;selected["b"][0]+=480
    compare()
    # Extending the block changes one saved endpoint. The supporting normals
    # and line identify the upstream edge even when endpoint order is reversed.
    doc=edge_document("chamfer")
    data=doc["items"][0]["data"]
    nodes=data["cad"]["nodes"]
    finish=nodes[-1]
    rect=next(n for n in nodes if n["type"]=="sketchCornerRectangle")
    rect["p1"]["x"]=32
    edge=finish["edges"][0];edge["a"],edge["b"]=edge["b"],edge["a"];edge["normals"].reverse()
    compare()
    finish["suppressed"]=True
    assert abs(compare()[0][1][1]-12800)<.001
    del finish["suppressed"]
    data["tree"]["rollbackIndex"]=1
    assert abs(compare()[0][1][1]-12800)<.001
    del data["tree"]["rollbackIndex"]
    previous=output.read_bytes()
    for message in ["positive","no longer exists","at least one edge"]:
        if message=="positive": finish["distance"]=0
        elif message=="no longer exists":
            finish["distance"]=2
            edge["a"][2]=22;edge["b"][2]=22
        else: finish["edges"]=[]
        source.write_text(json.dumps(doc))
        result=subprocess.run([editor,"--export",source,output],text=True,capture_output=True)
        assert result.returncode and message in result.stderr and output.read_bytes()==previous, result.stderr

    doc=edge_document("chamfer")
    nodes=doc["items"][0]["data"]["cad"]["nodes"]
    rect=next(n for n in nodes if n["type"]=="sketchCornerRectangle");rect.pop("p0");rect.pop("p1")
    rect.update(type="sketchPrimitive",primitive=dict(id=rect["id"],type="circle",center=dict(x=0,y=0),radius=10,segments=32))
    angle=math.tau/32
    nodes[-1]["edges"]=[dict(a=[10,0,20],b=[10*math.cos(angle),10*math.sin(angle),20],
                             normals=[[0,0,1],[math.cos(angle/2),math.sin(angle/2),0]])]
    source.write_text(json.dumps(doc))
    result=subprocess.run([editor,"--export",source,output],text=True,capture_output=True)
    assert result.returncode and "straight edges" in result.stderr and output.read_bytes()==previous, result.stderr


def check_lofts(editor, directory):
    source = directory / "loft.pcad"
    package = directory / "loft.3mf"
    reference = directory / "expected-loft.3mf"

    def compare():
        source.write_text(json.dumps(doc))
        subprocess.run([editor, "--export", source, package], check=True)
        subprocess.run([ROOT / "rust/target/debug/puppycad", "--export", source, reference], check=True)
        compare_solid_packages(package, reference)
        return package_geometry(package)[1]

    doc = loft_document()
    data = doc["items"][0]["data"]
    nodes = {n["id"]: n for n in data["cad"]["nodes"]}
    loft = nodes["native-loft-1"]
    bodies = compare()
    assert len(bodies) == 2 and abs(bodies[1][1][1]-4858.787) < .01
    step = directory / "loft.step"
    subprocess.run([editor, "--export", source, step], check=True)
    if os.environ.get("PUPPYCAD_TEST_FREECAD"):
        import FreeCAD
        import Part
        shapes = Part.read(str(step)).Solids
        assert len(shapes) == 2
        assert all(shape.isValid() and math.isclose(shape.Volume, body[1][1], rel_tol=.00001)
                   for shape, body in zip(shapes, bodies))
    loft["profiles"].reverse()
    compare()
    loft["profiles"].reverse()
    loft["suppressed"] = True
    assert len(compare()) == 1
    del loft["suppressed"]
    data["tree"]["rollbackIndex"] = 1
    assert len(compare()) == 1
    del data["tree"]["rollbackIndex"]
    circle = nodes["native-entity-1-3"]["primitive"]
    circle.update(radius=14, segments=8)
    loft["operation"] = "join"
    joined, = compare()
    assert joined[1][1] > 8000
    loft["operation"] = "newBody"
    # A section in XZ meets a displaced XY profile at an oblique angle.
    second = nodes["native-sketch-3"]
    second["targetId"] = "plane-front"
    circle.update(radius=6, segments=16, center=dict(x=10,y=-30))
    compare()

    def rejected(reason):
        before = package.read_bytes()
        source.write_text(json.dumps(doc))
        result = subprocess.run([editor, "--export", source, package], capture_output=True, text=True)
        assert result.returncode and reason in result.stderr, result.stderr
        assert package.read_bytes() == before

    circle["center"]["y"] = 0
    rejected("edge-on")
    circle["center"]["y"] = -1
    rejected("intersects itself")
    # Rust keeps the earlier extrusion when it skips the intersecting loft.
    subprocess.run([ROOT / "rust/target/debug/puppycad", "--export", source, reference], check=True)
    assert len(package_geometry(reference)[1]) == 1
    second["targetId"] = nodes["native-sketch-1"]["targetId"]
    circle["center"]["y"] = 10
    rejected("share a centre")
    loft["operation"] = "cut"
    rejected("operation")
    loft["operation"] = "newBody"
    hole = dict(type="sketchCornerRectangle", id="inner", sketchId="native-sketch-1",
                p0=dict(x=4,y=6), p1=dict(x=8,y=10))
    data["cad"]["nodes"].append(hole)
    rejected("cannot have holes")
    loft["profiles"].pop()
    rejected("two profiles")


def check_revolves(editor, directory):
    source = directory / "revolve.pcad"
    package = directory / "revolve.3mf"
    reference = directory / "reference.3mf"

    def compare(doc):
        source.write_text(json.dumps(doc))
        subprocess.run([editor, "--export", source, package], check=True)
        subprocess.run([ROOT / "rust/target/debug/puppycad", "--export", source, reference], check=True)
        compare_solid_packages(package, reference)

    for angle, axis, hole in [
        (360, ((0, 0), (0, 1)), False), (55.125, ((0, 0), (0, -1)), False),
        (101, ((7, 11), (10, 15)), False), (63, ((0, 0), (.000001, .000001)), False), (90, ((40, 0), (40, 10)), False),
        (360, ((0, 0), (0, 1)), True), (95.125, ((0, 0), (0, 1)), True),
    ]:
        compare(revolve_document(angle, axis, hole))

    doc = revolve_document()
    data = doc["items"][0]["data"]
    revolve = next(n for n in data["cad"]["nodes"] if n["type"] == "revolve")
    revolve["suppressed"] = True
    compare(doc)
    assert [name for name, _ in package_geometry(package)[1]] == ["Body 1"]
    del revolve["suppressed"]
    data["tree"]["rollbackIndex"] = 1
    compare(doc)
    assert [name for name, _ in package_geometry(package)[1]] == ["Body 1"]

    # Round openings and multiple holes exercise triangulation of both end caps.
    doc = revolve_document(115.5)
    nodes = doc["items"][0]["data"]["cad"]["nodes"]
    for ident, x in [("small-hole", 43), ("large-hole", 47)]:
        nodes.append(dict(type="sketchPrimitive", id=ident, sketchId="native-sketch-2", primitive=dict(
            type="circle", id=ident, radius=1.25, segments=16, center=dict(x=x, y=-5))))
    compare(doc)
    # The lower cap of the block has a mirrored sketch frame.
    face = dict(type="face", id="bottom", sourceId="native-extrude-1", faceId="native-extrude-1-solid-face-5")
    nodes.insert(0, face)
    next(n for n in nodes if n["id"] == "native-sketch-2")["targetId"] = face["id"]
    compare(doc)

    step = directory / "revolve.step"
    subprocess.run([editor, "--export", source, step], check=True)
    assert step.read_bytes().count(b"=FACETED_BREP(") == 2
    if os.environ.get("PUPPYCAD_TEST_FREECAD"):
        import FreeCAD
        import Part
        shapes = Part.read(str(step)).Solids
        reference_bodies = package_geometry(reference)[1]
        assert len(shapes) == len(reference_bodies) == 2
        for shape, (_, (bounds, volume)) in zip(shapes, reference_bodies):
            assert shape.isValid() and math.isclose(shape.Volume, volume, rel_tol=.00001)
            for axis, (low, high) in zip("XYZ", bounds):
                assert abs(getattr(shape.BoundBox, axis+"Min")-low) < .001
                assert abs(getattr(shape.BoundBox, axis+"Max")-high) < .001

    # A ring partly intersects the first block and crosses its bottom plane.
    doc = revolve_document(360, ((10, 0), (10, 1)))
    nodes = doc["items"][0]["data"]["cad"]["nodes"]
    next(n for n in nodes if n["id"] == "native-sketch-2")["targetId"] = "plane-top"
    rect = next(n for n in nodes if n["id"] == "native-entity-1-2")
    rect.update(p0={"x": 13, "y": 4}, p1={"x": 17, "y": 9})
    revolve = next(n for n in nodes if n["type"] == "revolve")
    for op in ["join", "cut"]:
        revolve["operation"] = op
        compare(doc)

    previous = package.read_bytes()
    for angle, axis, reason in [(0, ((0,0),(0,1)), "360 degrees"), (361, ((0,0),(0,1)), "360 degrees"),
                                (90, ((1,1),(1,1)), "no length"), (90, ((45,0),(45,1)), "crosses")]:
        source.write_text(json.dumps(revolve_document(angle, axis)))
        result = subprocess.run([editor, "--export", source, package], capture_output=True, text=True)
        assert result.returncode and reason in result.stderr, result.stderr
        assert package.read_bytes() == previous


def check_line_contours(editor, directory):
    model = directory / "line-sketch.pcad"
    output = directory / "line-sketch.stl"
    doc = json.loads((FIXTURES / "extruded-rectangle.pcad").read_bytes())
    nodes = doc["items"][0]["data"]["cad"]["nodes"]
    sketch, rectangle, feature = nodes[3:]

    def line(start, end):
        return dict(type="sketchLine", sketchId=sketch["id"], p0=dict(zip("xy", start)), p1=dict(zip("xy", end)))

    # Concave L with reversed and unordered edges. An open component and a
    # branch are both legitimate drawing geometry outside the extrusion.
    corners = [(0, 0), (50, 0), (50, 10), (20, 10), (20, 40), (0, 40)]
    contour = [line(corners[i], corners[i-1]) for i in [0, 3, 5, 2, 4, 1]]
    extras = [line((90, 0), (95, 0)), line((95, 0), (95, 5)), line((95, 0), (100, 0))]
    # A hole inside the upright, plus a separate profile in the L's recess.
    hole = dict(rectangle, p0={"x": 4, "y": 20}, p1={"x": 12, "y": 28})
    recess = dict(rectangle, p0={"x": 30, "y": 20}, p1={"x": 40, "y": 30})
    def write(entities, index=1):
        for i, node in enumerate(entities):
            node["id"] = f"edge-or-shape-{i}"
        nodes[4:] = entities + [feature]
        feature["profileId"] = sketch["id"] + f"-profile-{index}"
        model.write_text(json.dumps(doc))

    def verify(entities, index, bounds, area):
        write(entities, index)
        subprocess.run([editor, "--export", model, output], check=True)
        actual, volume = triangulation(output)
        assert actual == bounds and math.isclose(volume, area * 60), (actual, volume)

    verify(contour+extras, 1, [(0, 50), (0, 40), (0, 60)], 1100)
    verify(contour+[hole, recess]+extras, 1, [(30, 40), (20, 30), (0, 60)], 100)
    verify(contour+[hole, recess]+extras, 2, [(0, 50), (0, 40), (0, 60)], 1100-64)
    saved = output.read_bytes()
    for entities in [contour[:-1], contour+[line(corners[0], (2, 2))]]:
        write(entities)
        assert subprocess.run([editor, "--export", model, output], capture_output=True).returncode
        assert output.read_bytes() == saved
    bowtie = [line((0, 0), (20, 20)), line((20, 20), (0, 20)),
              line((0, 20), (20, 0)), line((20, 0), (0, 0))]
    for index in (1, 2):
        write(bowtie, index)
        subprocess.run([editor, "--export", model, output], check=True)
        _, volume = triangulation(output)
        assert math.isclose(volume, 100 * 60), volume


def check_assembly(editor, directory):
    model = directory / "arm.pcad"
    package = directory / "arm.3mf"
    doc = json.loads((FIXTURES / "assembly.pcad").read_bytes())
    doc["selectedPath"] = [2]
    assembly = doc["items"][2]["data"]
    def export():
        model.write_text(json.dumps(doc))
        subprocess.run([editor, "--export", model, package], check=True)
        title, parts = package_geometry(package)
        assert title == "Arm assembly"
        return dict(parts)

    parts = export()
    assert parts["Base"] == ([(-20, 20), (-20, 20), (0, 10)], 16000)
    assert parts["Upper arm"] == ([(-5, 5), (-20, 20), (10, 15)], 2000)
    for angle in [-45, 0, 60]:
        assembly["actuators"][0]["homeDeg"] = angle
        bounds, volume = export()["Forearm"]
        radians = math.radians(angle)
        xy = [(x*math.cos(radians)-y*math.sin(radians), 15+x*math.sin(radians)+y*math.cos(radians))
              for x in (-5, 5) for y in (-5, 35)]
        expected = [(min(v), max(v)) for v in zip(*xy)] + [(15, 20)]
        assert all(math.isclose(a, b, abs_tol=.0001) for pair, ref in zip(bounds, expected) for a, b in zip(pair, ref))
        assert math.isclose(volume, 2000, rel_tol=.00001)

    fixed = assembly["mates"][0]
    for side in ("a", "b"):
        # Fix a connector to a world connector in either graph direction.
        ground = dict(id="ground", position=dict(x=0, y=2, z=4))
        assembly["connectors"].append(ground)
        old = fixed[side]
        fixed[side] = dict(instanceId=None, connectorId="ground")
        bounds, _ = export()["Upper arm" if side == "a" else "Base"]
        assert bounds[2] == ((4, 9) if side == "a" else (-6, 4))
        fixed[side] = old
        assembly["connectors"].pop()
    # A conflicting second mount and a dangling connector must preserve the last export.
    before = package.read_bytes()
    for bad in [dict(fixed, id="conflict", b=dict(instanceId="arm-1", connectorId="arm1-end")),
                dict(fixed, id="unknown", a=dict(instanceId="base-1", connectorId="unknown"))]:
        assembly["mates"].append(bad)
        model.write_text(json.dumps(doc))
        assert subprocess.run([editor, "--export", model, package], capture_output=True).returncode
        assert package.read_bytes() == before
        assembly["mates"].pop()
    # A free occurrence preserves independent XYZ scale and rotation.
    assembly["mates"].clear()
    instance = assembly["instances"][2]
    instance["transform"] = dict(translation=dict(x=1e-5, y=2, z=3), rotation=dict(x=90, y=90, z=0),
                                 scale=dict(x=-2, y=1, z=3))
    bounds, volume = export()["Forearm"]
    assert bounds == [(0.00001, 15.00001), (-8, 12), (-17, 23)] and math.isclose(volume, 12000)
    named = directory / "named.stl"
    subprocess.run([editor, "--export", FIXTURES / "assembly.pcad", named, "--assembly", "assembly-1"], check=True)
    for label in ("Base", "Upper-arm", "Forearm"):
        assert triangulation(directory / f"named-{label}.stl")[1] > 0
    assert not named.exists()


def free_mate_document(kind):
    doc = json.loads((FIXTURES / "assembly.pcad").read_bytes())
    doc["selectedPath"] = [2]
    data = doc["items"][2]["data"]
    data["instances"] = data["instances"][1:]
    data["instances"][0].pop("transform")
    data["instances"][1]["transform"] = dict(translation=dict(x=30, y=-12, z=7), rotation=dict(x=10, y=20, z=35))
    data["connectors"] = data["connectors"][2:]
    positions = {"revolute": [(0, 10, 5), (0, -10, 0)], "prismatic": [(0, 0, 0), (0, 0, 0)],
                 "planar": [(0, 0, 10), (0, 0, 0)], "ball": [(3, 4, 5), (-1, 0, 2)]}
    for connector, xyz in zip(data["connectors"], positions[kind]):
        connector["position"] = dict(zip("xyz", xyz))
    data["mates"] = data["mates"][1:]
    data["mates"][0]["type"] = kind
    data["actuators"] = []
    return doc


def check_free_mates(editor, directory):
    path = directory / "free-joint.pcad"
    package = directory / "free-joint.3mf"
    reference = ROOT / "rust/target/debug/puppycad"
    for kind in ("revolute", "prismatic", "planar", "ball"):
        path.write_text(json.dumps(free_mate_document(kind)))
        subprocess.run([editor, "--export", path, package], check=True)
        parts = dict(package_geometry(package)[1])
        assert parts["Upper arm"] == ([(-5, 5), (-20, 20), (0, 5)], 2000)
        box, volume = parts["Forearm"]
        assert math.isclose(volume, 2000, rel_tol=.00001)
        if kind == "prismatic":
            assert all(abs(a-b)<.0001 for bounds, expected in zip(box[:2],[(-5,5),(-20,20)]) for a,b in zip(bounds,expected))
            assert box[2][0] > 1 and abs(box[2][1]-box[2][0]-5)<.0001
        if kind == "planar":
            assert all(abs(a-b)<.0001 for a,b in zip(box[2], (10, 15)))
        if reference.is_file():
            # The two writers use different cap triangulations. Compare all
            # reference corner positions in world space for the moving link.
            for tool, stem in [(editor,"flex-joint"), (reference,"rust-joint")]:
                subprocess.run([tool, "--export", path, directory/(stem+".stl")], check=True, stdout=subprocess.PIPE)
            def vertices(stem):
                raw = (directory/(stem+"-Forearm.stl")).read_bytes()[84:]
                return {r[i:i+3] for r in struct.iter_unpack("<12fH", raw) for i in (3, 6, 9)}
            actual = vertices("flex-joint")
            assert max(min(math.dist(p,q) for q in actual) for p in vertices("rust-joint")) < .001, kind

    # Two ground ball joints position both ends of a bar. Its fastened
    # companion must move with it. The first instance stays grounded.
    doc = free_mate_document("ball")
    data = doc["items"][2]["data"]
    moving = data["instances"][1]
    moving["transform"] = dict(translation=dict(x=30,y=5,z=3))
    data["instances"].append(dict(id="tool", name="Tool", partId=moving["partId"]))
    def connector(id_, owner, xyz):
        return dict(id=id_, instanceId=owner, position=dict(zip("xyz", xyz)))
    def mate(id_, kind, a, b):
        def ref(c): return dict(instanceId=c["instanceId"], connectorId=c["id"])
        return dict(id=id_, type=kind, a=ref(a), b=ref(b))
    a, b = [connector("ground-"+str(i), None, (10,25+i,8)) for i in (0,10)]
    c, d = [connector("pin-"+str(i), moving["id"], (0,i,0)) for i in (0,10)]
    e = connector("tool-base", "tool", (0,0,0))
    data["connectors"] = [a,b,c,d,e]
    data["mates"] = [mate("lock","fasten",d,e), mate("ball-1","ball",a,c), mate("ball-2","ball",b,d)]
    path.write_text(json.dumps(doc))
    subprocess.run([editor, "--export", path, package], check=True)
    parts = dict(package_geometry(package)[1])
    for name, y in [("Forearm",25), ("Tool",35)]:
        for bounds, target in zip(parts[name][0], [(5,15),(y-20,y+20),(8,13)]):
            assert all(abs(x-z)<.001 for x,z in zip(bounds,target)), (name,parts[name])
    saved = package.read_bytes()
    b["position"]["y"] += 3
    path.write_text(json.dumps(doc))
    failure = subprocess.run([editor, "--export", path, package], capture_output=True)
    assert failure.returncode and b"Conflicting assembly mates" in failure.stderr and package.read_bytes()==saved


def horizontal_surfaces(path, x, y):
    # Intersect a vertical line with horizontal facets. This distinguishes an
    # inward pocket from one with an equally sized cut on the opposite face.
    heights = set()
    for record in struct.iter_unpack("<12fH", path.read_bytes()[84:]):
        a, b, c = (record[i:i+3] for i in (3, 6, 9))
        if a[2] != b[2] or b[2] != c[2]:
            continue
        crosses = [(v[0]-u[0])*(y-u[1]) - (v[1]-u[1])*(x-u[0])
                   for u, v in [(a, b), (b, c), (c, a)]]
        if min(crosses) >= 0 or max(crosses) <= 0:
            heights.add(round(a[2], 5))
    return heights


def check_faces(editor, directory):
    project = json.loads((FIXTURES / "face-sketch.pcad").read_bytes())
    data = project["items"][0]["data"]
    nodes = data["cad"]["nodes"]
    block, top, pocket, side, boss = (nodes[i] for i in (5, 6, 9, 10, 13))
    model, mesh = directory / "faces.pcad", directory / "faces.stl"
    area = 16 * 10**2 * math.sin(math.tau / 32)
    def export():
        model.write_text(json.dumps(project))
        subprocess.run([editor, "--export", model, mesh], check=True)
        return triangulation(mesh)
    def check(depth, removed, expected_z, boss_z=(5, 15)):
        bounds, volume = export()
        assert bounds == [(-40, 46), (-30, 30), (0, depth)], bounds
        assert math.isclose(volume, 80*60*depth + 1200 - removed*area, rel_tol=0.00001), volume
        assert horizontal_surfaces(mesh, 1.123, 2.234) == set(expected_z)
        assert horizontal_surfaces(mesh, 43.213, 1.234) == set(boss_z)
    check(20, 8, (0, 12))
    # The same stable face IDs resolve on every reference plane. Reversing
    # rectangle endpoints changes the side numbering to follow the saved loop.
    for target, expected in [("plane-front", [(-40, 46), (0, 20), (-30, 30)]),
                             ("plane-right", [(0, 20), (-40, 46), (-30, 30)])]:
        nodes[3]["targetId"] = target
        bounds, volume = export()
        assert bounds == expected and math.isclose(volume, 97200-8*area, rel_tol=0.00001)
    nodes[3]["targetId"] = "plane-top"
    rect = nodes[4]
    rect["p0"], rect["p1"] = rect["p1"], rect["p0"]
    assert export()[0] == [(-46, 40), (-30, 30), (0, 20)]
    rect["p0"], rect["p1"] = rect["p1"], rect["p0"]
    for options, heights in [({"endCondition": "symmetric"}, (-10, 2)),
                             ({"flip": True}, (-20, -8))]:
        block.update(options)
        export()
        assert horizontal_surfaces(mesh, 1.123, 2.234) == set(heights)
        for key in options:
            del block[key]
    # A suppressed new body cannot redirect later cuts into an earlier solid.
    nodes.insert(6, dict(block, id="inactive", suppressed=True))
    model.write_text(json.dumps(project))
    result = subprocess.run([editor, "--export", model, mesh], capture_output=True)
    assert result.returncode and b"active target body" in result.stderr
    del nodes[6]
    # The first inner side of a square hole follows its clockwise boundary.
    hole = copy.deepcopy(nodes[8])
    hole.update(id="hole", sketchId=block["sketchId"])
    hole["primitive"].update(id="hole", segments=4)
    nodes.insert(5, hole)
    pocket["suppressed"] = True
    side["faceId"] = "extrude-1-solid-face-5"
    side_rect = nodes[13]
    old_points = side_rect["p0"], side_rect["p1"]
    side_rect.update(p0={"x": 4, "y": 4}, p1={"x": 8, "y": 8})
    boss["operation"] = "newBody"
    export()
    marker_bounds, marker_volume = triangulation(directory / "faces-2.stl")
    root2 = math.sqrt(2)
    expected = [(-4*root2, root2), (-10+2*root2, -10+7*root2), (4, 8)]
    for got, pair in zip(marker_bounds, expected):
        assert all(abs(x-y)<0.00001 for x, y in zip(got, pair)), marker_bounds
    assert math.isclose(marker_volume, 96, rel_tol=0.00001)
    side_rect["p0"], side_rect["p1"] = old_points
    boss["operation"] = "join"
    side["faceId"] = "extrude-1-solid-face-2"
    del pocket["suppressed"]
    del nodes[5]
    block["depth"] = 26
    check(26, 8, (0, 18))
    pocket["flip"] = True
    check(26, 0, (0, 26))
    pocket["flip"] = False
    pocket["endCondition"] = "symmetric"
    check(26, 4, (0, 22))
    for condition in ["through-all", "up-to-body"]:
        pocket.update(endCondition=condition, targetId=block["id"])
        check(26, 26, ())
    pocket["endCondition"] = "blind"
    top["faceId"] = "extrude-1-solid-face-5"
    check(26, 8, (8, 26))
    # The third extrusion can itself support a sketch. On this top face
    # the local normal points toward +X and a cut enters the rectangular boss.
    import_face = dict(top, id="boss-top", sourceId=boss["id"], faceId=boss["id"]+"-solid-face-6")
    sketch = dict(nodes[11], id="child", targetId=import_face["id"])
    rect = dict(nodes[12], id="child-rect", sketchId="child", p0={"x": 24, "y": 7}, p1={"x": 33, "y": 10})
    cut = dict(boss, id="child-cut", sketchId="child", profileId="child-profile-1", operation="cut", depth=2)
    nodes.extend([import_face, sketch, rect, cut])
    _, volume = export()
    assert math.isclose(volume, 80*60*26 + 1200 - 8*area - 54, rel_tol=0.00001)
    del nodes[-4:]
    data["tree"]["rollbackIndex"] = 2
    assert math.isclose(export()[1], 80*60*26 - 8*area, rel_tol=0.00001)
    del data["tree"]["rollbackIndex"]
    for id_, suppressed in [("extrude-1-solid-face-99", False), ("other-solid-face-6", False),
                            ("extrude-1-solid-face-6", True)]:
        top["faceId"] = id_
        block["suppressed"] = suppressed
        model.write_text(json.dumps(project))
        previous = mesh.read_bytes()
        result = subprocess.run([editor, "--export", model, mesh], capture_output=True)
        assert result.returncode and mesh.read_bytes() == previous


def check_offset_profiles(editor, tool, directory):
    reference = ROOT / "rust/target/debug/puppycad"
    radius = 12
    outer = radius + 3 / math.cos(math.pi/6)
    cases = [
        (dict(type="rectangle", width=32, height=18), (38*24 - 32*18)*10),
        (dict(type="polygon", sides=6, radius=radius), 30*math.sin(math.pi/3)*(outer**2-radius**2)),
    ]
    for shape, volume in cases:
        drawing = json.loads((FIXTURES/"segmented-slot.pcad").read_bytes())
        nodes = drawing["items"][0]["data"]["cad"]["nodes"]
        source = next(n for n in nodes if n["type"]=="sketchPrimitive")
        source["primitive"] = dict(shape, id=source["id"], center=dict(x=10, y=-15), rotation=30)
        next(n for n in nodes if n["type"]=="extrude")["depth"] = 10
        path = directory / (shape["type"]+"-offset.pcad")
        path.write_text(json.dumps(drawing))
        authored = subprocess.run([tool, path, source["sketchId"], "offset", source["id"], '"3 mm"'],
                                  check=True, capture_output=True)
        path.write_bytes(authored.stdout)
        output = path.with_suffix(".3mf")
        subprocess.run([editor, "--export", path, output], check=True)
        (_, (_, actual)), = package_geometry(output)[1]
        assert math.isclose(volume, actual, rel_tol=.00001), (shape, volume, actual)
        if reference.is_file():
            rust = path.with_name(path.stem+"-rust.3mf")
            subprocess.run([reference, "--export", path, rust], check=True)
            compare_solid_packages(output, rust)


def check_closed_offset(editor, tool, directory):
    drawing = json.loads((FIXTURES / "offset.pcad").read_bytes())
    data = drawing["items"][0]["data"]
    nodes = data["cad"]["nodes"]
    del nodes[4:]
    sketch = nodes[3]
    chain = dict(id="offset-chain-1", type="offsetChain", closed=True, value=5,
                 side=-1, sources=[], targets=[])
    sketch["relations"] = [chain]
    inner = [(0, 0), (30, 0), (30, 20), (0, 20)]
    outer = [(-5, -5), (35, -5), (35, 25), (-5, 25)]
    for i in range(4):
        reverse = i % 2 == 1
        target = "copy-" + str(i)
        source = "source-" + str(i)
        chain["sources"].append(dict(entityId=source, reversed=reverse))
        chain["targets"].append(target)
        for name, corners in [(source, inner), (target, outer)]:
            a, b = corners[i], corners[(i + 1) % 4]
            if name == source and reverse:
                a, b = b, a
            nodes.append(dict(type="sketchLine", id=name, sketchId=sketch["id"],
                              p0=dict(zip("xy", a)), p1=dict(zip("xy", b))))
    feature = dict(type="extrude", id="rim", name="Rim", sketchId=sketch["id"],
                   profileId="native-sketch-1-profile-1", operation="newBody", depth=10)
    nodes.append(feature)
    data["cad"]["rootNodeIds"].append(feature["id"])
    data["tree"]["orderedNodeIds"].append(feature["id"])
    path = directory / "closed-offset.pcad"
    mesh = path.with_suffix(".3mf")
    path.write_text(json.dumps(drawing))
    subprocess.run([editor, "--export", path, mesh], check=True)
    (_, (bounds, volume)), = package_geometry(mesh)[1]
    assert bounds == [(-5, 35), (-5, 25), (0, 10)] and math.isclose(volume, 6000)
    rust = ROOT / "rust/target/debug/puppycad"
    if rust.is_file():
        reference = directory / "closed-offset-rust.3mf"
        subprocess.run([rust, "--export", path, reference], check=True)
        compare_solid_packages(mesh, reference)
    before = mesh.read_bytes()
    chain["sources"][1]["reversed"] = False
    path.write_text(json.dumps(drawing))
    assert subprocess.run([editor, "--export", path, mesh], capture_output=True).returncode
    assert mesh.read_bytes() == before

    # Rebuild from the four original edges in both winding directions. The
    # orientation of the first edge fixes the traversal of this closed chain.
    sketch["relations"] = []
    nodes[:] = [n for n in nodes if n["id"] not in chain["targets"]]
    first = next(n for n in nodes if n["id"]=="source-0")
    for sign in (-1, 1):
        if sign==1:
            first["p0"], first["p1"] = first["p1"], first["p0"]
        path.write_text(json.dumps(drawing))
        authored = subprocess.run([tool, path, sketch["id"], "offset", "source-2", '"5"'],
                                  check=True, capture_output=True)
        path.write_bytes(authored.stdout)
        updated = json.loads(authored.stdout)["items"][0]["data"]["cad"]["nodes"]
        relation, = next(n for n in updated if n["type"]=="sketch")["relations"]
        assert relation["closed"] and relation["side"]==sign and len(relation["targets"])==4
        rebuilt = directory / f"authored-chain-{sign}.3mf"
        subprocess.run([editor, "--export", path, rebuilt], check=True)
        compare_solid_packages(rebuilt, mesh)
        if rust.is_file():
            subprocess.run([rust, "--export", path, reference], check=True)
            compare_solid_packages(rebuilt, reference)


def check_curves(editor, directory):
    reference = ROOT / "rust/target/debug/puppycad"
    def extrude(name, source, close=None):
        part = source["items"][0]["data"]
        nodes = part["cad"]["nodes"]
        if close:
            nodes.append(dict(type="sketchLine", id="closing-edge", sketchId="native-sketch-1",
                              p0=close[0], p1=close[1]))
        nodes.append(dict(type="extrude", id="curve-solid", name="Curve solid", sketchId="native-sketch-1",
                          profileId="native-sketch-1-profile-1", operation="newBody", depth=10))
        part["tree"]["orderedNodeIds"].append("curve-solid")
        part["cad"]["rootNodeIds"].append("curve-solid")
        path = directory / (name+".pcad")
        path.write_text(json.dumps(source))
        dest = directory / (name+".3mf")
        subprocess.run([editor, "--export", path, dest], check=True)
        (_, (bounds, volume)), = package_geometry(dest)[1]
        if reference.is_file():
            ref = directory / (name+"-rust.3mf")
            subprocess.run([reference, "--export", path, ref], check=True, stdout=subprocess.DEVNULL)
            (_, (ref_bounds, ref_volume)), = package_geometry(ref)[1]
            assert all(abs(a-b) < .0001 for axis, expected in zip(bounds, ref_bounds)
                       for a, b in zip(axis, expected)), (name, bounds, ref_bounds)
            assert math.isclose(volume, ref_volume, rel_tol=.00001), (name, volume, ref_volume)
        return volume
    source = json.loads((FIXTURES / "splines.pcad").read_bytes())
    assert abs(extrude("periodic", copy.deepcopy(source)) - 97590.235) < .01
    shapes = source["items"][0]["data"]["cad"]["nodes"]
    shapes[-2]["spline"]["construction"] = True
    control = shapes[-1]["spline"]
    control["points"] = [dict(x=x,y=y) for x,y in [(0,0),(5,25),(15,25),(20,0),(25,-5),(35,-5),(40,0)]]
    # Close both cubic segments with a low return path, clear of their inflection.
    for n, (a,b) in enumerate([((40,0),(40,-10)),((40,-10),(0,-10))]):
        shapes.append(dict(id=f"side-{n}", type="sketchLine", sketchId="native-sketch-1",
                           p0=dict(zip("xy",a)), p1=dict(zip("xy",b))))
    extrude("control", source, (dict(x=0,y=-10),dict(x=0,y=0)))
    source = json.loads((FIXTURES / "tangent-arc.pcad").read_bytes())
    nodes = source["items"][0]["data"]["cad"]["nodes"]
    assert abs(extrude("tangent", source, (nodes[-1]["spline"]["points"][-1],nodes[4]["p0"])) - 18625.894) < .01
    ellipse = json.loads((FIXTURES / "splines.pcad").read_bytes())
    nodes = ellipse["items"][0]["data"]["cad"]["nodes"]
    del nodes[4:]
    nodes.append(dict(id="elliptic", type="sketchPrimitive", sketchId="native-sketch-1",
                      primitive=dict(type="ellipticArc", id="elliptic", center=dict(x=10,y=20),
                                     width=80,height=30,rotation=90,startAngle=0,sweep=-math.pi,segments=64)))
    assert extrude("elliptic",ellipse,(dict(x=10,y=-20),dict(x=10,y=60))) > 9300


def check_step(editor, directory):
    source = directory / "step-input.pcad"
    target = directory / "design.STEP"
    sensor = json.loads((ROOT / "tests/fixtures/sensor-unit.pcad").read_bytes())
    sensor["assembly"]["name"] = "Sensor unit assembly"
    source.write_text(json.dumps(sensor))
    subprocess.run([editor, "--export", source, target], check=True)
    baseline = target.read_bytes()
    assert baseline.startswith(b"ISO-10303-21;") and baseline.endswith(b"END-ISO-10303-21;\n")
    assert baseline.count(b"=FACETED_BREP(") == 5 and baseline.count(b"=NEXT_ASSEMBLY_USAGE_OCCURRENCE(") == 6
    assert b"SI_UNIT(.MILLI.,.METRE.)" in baseline and b"Frozen source" not in baseline
    # Original Biscuit ea4670d AP214 entities, including all part geometry,
    # stable product IDs and solved placements. Local and frozen headers differ.
    entities = baseline.split(b"\nDATA;\n", 1)[1]
    assert hashlib.sha256(entities).hexdigest() == "11888f0916f929892a84bd488cccf77b622242e7acc1b295856ebe5db2884790"
    subprocess.run([editor, "--export", source, target], check=True)
    assert target.read_bytes() == baseline
    # The reference exporter rejects a disconnected interior void shell.
    base = next(part for part in sensor["parts"] if part["id"] == "base")
    sensor["nodes"].extend([dict(id="void", type="box", size=[1, 1, 1], position=[50, 20, .5]),
                            dict(id="void-cut", type="subtract", a=base["node"], b="void")])
    base["node"] = "void-cut"
    source.write_text(json.dumps(sensor))
    fail = subprocess.run([editor, "--export", source, target], capture_output=True)
    assert fail.returncode and b"void shell" in fail.stderr and target.read_bytes() == baseline

    # Two instances share an arm. A mirror with nonuniform scale needs its
    # own local definition and keeps the existing rigid occurrence placement.
    arm = json.loads((FIXTURES / "assembly.pcad").read_bytes())
    arm["items"][2]["data"]["instances"][2]["name"] = "Link's Käpälä"
    target = directory / "arm.StP"
    for scaled, definitions in [(False, 2), (True, 3)]:
        if scaled:
            # The v4 solver permits nonuniform scale on freely placed instances.
            arm["items"][2]["data"]["mates"] = []
            arm["items"][2]["data"]["actuators"] = []
            arm["items"][2]["data"]["instances"][2]["transform"]["scale"] = dict(x=-2, y=1, z=1)
        source.write_text(json.dumps(arm))
        subprocess.run([editor, "--export", source, target, "--assembly", "Arm assembly"], check=True)
        data = target.read_bytes()
        assert data.count(b"=FACETED_BREP(") == definitions
        assert data.count(b"=NEXT_ASSEMBLY_USAGE_OCCURRENCE(") == 3
        assert b"Link''s K" in data and b"00E4" in data
    if os.environ.get("PUPPYCAD_TEST_FREECAD"):
        import FreeCAD
        import Part
        shapes = Part.read(str(target)).Solids
        manufacturing = directory / "scaled.3mf"
        subprocess.run([editor, "--export", source, manufacturing, "--assembly", "Arm assembly"], check=True)
        bodies = package_geometry(manufacturing)[1]
        assert len(shapes) == len(bodies) == 3
        for shape, (_, (bounds, volume)) in zip(shapes, bodies):
            assert shape.isValid() and math.isclose(shape.Volume, volume, rel_tol=.00001)
            box = shape.BoundBox
            for k, (low, high) in zip("XYZ", bounds):
                assert abs(getattr(box, k+"Min") - low) < .001 and abs(getattr(box, k+"Max") - high) < .001


def check_line_solver(editor, tool, directory, sdk):
    fixture = sdk/"cad/tests/fixtures/constrained-plate.pcad"
    base = json.loads(fixture.read_bytes())
    sketch = "native-sketch-1"
    source = directory/"line-edit.pcad"

    def solve(doc, command, *args):
        source.write_text(json.dumps(doc))
        return subprocess.run([tool,source,sketch,command,*args],capture_output=True,text=True)

    def result(doc, command, *args):
        run = solve(doc,command,*args)
        assert run.returncode==0, run.stderr
        return json.loads(run.stdout)

    mixed=json.loads((FIXTURES/"dimensions.pcad").read_bytes())
    report=result(mixed,"inspect")
    assert report["degrees_of_freedom"]==3
    assert [r["id"] for r in report["distances"]]==["dist","dx","oy"]
    assert [r["measured"] for r in report["distances"]]==[31.622777,-40.8,30]
    moved=result(result(mixed,"center","point",'{"x":20,"y":40}'),"inspect")
    # Rust projects Y onto the 30 mm dimension and moves the line along X.
    assert moved["points"]==[dict(id="point",center=dict(x=20,y=30)),dict(id="origin",center=dict(x=0,y=0))]
    assert moved["lines"][0]["p0"]==dict(x=10,y=0) and moved["lines"][0]["p1"]==dict(x=60.8,y=0)
    assert moved["degrees_of_freedom"]==3 and abs(moved["distances"][0]["value"]-math.hypot(10,30))<1e-6
    assert "fixed" in solve(mixed,"center","origin",'{"x":20,"y":40}').stderr
    changed=result(mixed,"variable","w",json.dumps(dict(name="w",expression="2 cm")))
    after=result(changed,"inspect")
    assert after["lines"][0]["p0"]==dict(x=0,y=0) and after["lines"][0]["p1"]==dict(x=40,y=0)
    assert after["points"]==[dict(id="point",center=dict(x=-.8,y=24.6)),dict(id="origin",center=dict(x=0,y=0))]
    assert abs(after["distances"][0]["value"]-math.hypot(.8,24.6))<1e-6
    changed=result(mixed,"distance","dx",json.dumps("-32 mm"))
    assert result(changed,"inspect")["points"][0]["center"]==dict(x=18.8,y=30)
    assert "measured" in solve(changed,"distance","dist",json.dumps("39")).stderr
    ref=result(changed,"reference","dx","null")
    report=result(ref,"inspect")
    assert report["degrees_of_freedom"]==4 and "expression" not in report["distances"][1]
    driving=result(ref,"reference","dx","null")
    assert result(driving,"inspect")["degrees_of_freedom"]==3
    # A two-point sketch has no physical segments. The distance acts on its
    # actual anchors and its solver-only segment stays out of the document.
    only_points=json.loads(json.dumps(mixed))
    rows=only_points["items"][0]["data"]["cad"]["nodes"]
    rows.remove(next(n for n in rows if n["id"]=="line"))
    sheet=next(n for n in rows if n["type"]=="sketch")
    diag=next(r for r in sheet["relations"] if r["id"]=="dist")
    diag["a"]=dict(entityId="origin",point="center")
    sheet["relations"]=[diag,sheet["relations"][-1]]
    real=result(only_points,"reference","dist","null")
    resized=result(real,"distance","dist",json.dumps("2 in"))
    actual=result(resized,"inspect")
    assert actual["lines"]==[] and actual["degrees_of_freedom"]==1
    assert abs(actual["distances"][0]["measured"]-50.8)<.0001
    radial=result(result(resized,"center","point",'{"x":-3,"y":4}'),"inspect")
    assert math.dist(list(radial["points"][0]["center"].values()),[-30.48,40.64])<.0001
    for axis,value in [("z",10),("x",99)]:
        bad=json.loads(json.dumps(mixed))
        next(n for n in bad["items"][0]["data"]["cad"]["nodes"] if n["type"]=="sketch")["relations"][2].update(axis=axis,value=value)
        assert solve(bad,"inspect").returncode

    assert result(base,"inspect")["degrees_of_freedom"]==0
    edited = result(base,"length","bottom","32.125")
    report = result(edited,"inspect")
    assert report["degrees_of_freedom"]==0 and report["redundant_constraints"]==[]
    corners = [(0,0),(32.125,0),(32.125,10),(0,10)]
    for i,line in enumerate(report["lines"]):
        assert (line["p0"]["x"],line["p0"]["y"])==corners[i]
        assert (line["p1"]["x"],line["p1"]["y"])==corners[(i+1)%4]
    data = edited["items"][0]["data"]
    nodes=data["cad"]["nodes"]
    assert [n["id"] for n in nodes]==[n["id"] for n in base["items"][0]["data"]["cad"]["nodes"]]
    assert next(n for n in nodes if n["id"]=="width")["constraint"]["value"]==32.125
    assert next(n for n in nodes if n["type"]=="sketch")["relations"]==next(
        n for n in base["items"][0]["data"]["cad"]["nodes"] if n["type"]=="sketch")["relations"]
    # Annotated graph dimensions become sketch relations in both native editors.
    sketch_node=next(n for n in nodes if n["type"]=="sketch")
    sketch_node["variables"]=[dict(name="span",kind="length",expression="#inch / 2"),
                              dict(name="inch",kind="length",expression="1 in")]
    next(n for n in nodes if n["id"]=="width")["name"]="Driving width"
    expression="#span * 2 + 6.725 mm"
    annotated=result(edited,"expression","bottom",json.dumps(expression))
    measured=result(annotated,"inspect")
    assert measured["degrees_of_freedom"]==0 and measured["lines"][0]["length"]==32.125
    assert measured["lines"][0]["expression"]==expression
    ann_nodes=annotated["items"][0]["data"]["cad"]["nodes"]
    assert all(n["id"]!="width" for n in ann_nodes)
    relation=next(r for n in ann_nodes if n["type"]=="sketch" for r in n["relations"] if r["id"]=="width")
    assert relation==dict(type="length",entityId="bottom",id="width",value=32.125,expression=expression,name="Driving width")
    plain=result(annotated,"expression","bottom",json.dumps(" 3.2125e1 "))
    assert "expression" not in result(plain,"inspect")["lines"][0]
    for invalid in ("1mm + 2","1 / 0","#missing","45 deg","1mm * 1mm","1e999"):
        assert solve(annotated,"expression","bottom",json.dumps(invalid)).returncode,invalid
    renamed=result(annotated,"variable","inch",json.dumps(dict(name="unit",expression="2 cm")))
    renamed_nodes=renamed["items"][0]["data"]["cad"]["nodes"]
    renamed_sketch=next(n for n in renamed_nodes if n["type"]=="sketch")
    assert renamed_sketch["variables"][0]["expression"]=="#unit / 2"
    propagated=result(renamed,"inspect")
    assert propagated["lines"][0]["length"]==26.725 and propagated["lines"][2]["length"]==26.725
    assert propagated["degrees_of_freedom"]==0
    for changes,reason in [(dict(name="unit",expression=""),"Unknown"),
                           (dict(name="unit",expression="#span"),"Cyclic"),
                           (dict(name="span",expression="1 in"),"Duplicate")]:
        assert reason in solve(renamed,"variable","unit",json.dumps(changes)).stderr
    unused=result(renamed,"variable","",json.dumps(dict(name="shaft",expression="(pi / 2) rad")))
    removed=result(unused,"variable","shaft",json.dumps(dict(name="shaft",expression="")))
    assert removed==renamed
    source.write_text(json.dumps(annotated))
    documented=directory/"expression.3mf"
    subprocess.run([editor,"--export",source,documented],check=True)
    accepted_bytes=documented.read_bytes()
    next(n for n in ann_nodes if n["id"]=="bottom")["p1"]["x"]=31
    source.write_text(json.dumps(annotated))
    bad=subprocess.run([editor,"--export",source,documented],capture_output=True)
    assert bad.returncode and b"disagrees" in bad.stderr and documented.read_bytes()==accepted_bytes
    next(n for n in ann_nodes if n["id"]=="bottom")["p1"]["x"]=32.125
    relation["value"]=19
    assert "disagrees" in solve(annotated,"inspect").stderr
    source.write_text(json.dumps(edited))
    package,reference=directory/"lines.3mf",directory/"rust-lines.3mf"
    subprocess.run([editor,"--export",source,package],check=True)
    subprocess.run([ROOT/"rust/target/debug/puppycad","--export",source,reference],check=True)
    compare_solid_packages(package,reference)
    assert math.isclose(package_geometry(package)[1][0][1][1],1606.25,abs_tol=.001)
    assert solve(edited,"p0","bottom",'{"x":1,"y":1}').returncode
    assert "0.01 to 500" in solve(edited,"length","bottom","0").stderr

    doc=json.loads((FIXTURES/"graph-constrained-line.pcad").read_bytes())
    graph=doc["items"][0]["data"]["cad"]
    graph["nodes"][-1]["constraint"].update(type="vertical")
    assert solve(doc,"inspect").returncode
    doc=json.loads((FIXTURES/"graph-constrained-line.pcad").read_bytes())
    nodes=doc["items"][0]["data"]["cad"]["nodes"]
    nodes.pop()
    line=next(n for n in nodes if n["type"]=="sketchLine")
    long_id="segment-with-a-long-stable-id-"*3
    nodes[-1]["constraint"]["entityId"]=long_id
    line.update(id=long_id,p0=dict(x=20,y=5),p1=dict(x=-20,y=5))
    sized=result(doc,"length",long_id,"50.5")
    twice=result(sized,"length",long_id,"60.25")
    rows=twice["items"][0]["data"]["cad"]["nodes"]
    assert next(n for n in rows if n["id"]==long_id)["p1"]==dict(x=-40.25,y=5)
    relations=next(n for n in rows if n["type"]=="sketch")["relations"]
    assert len(relations)==1 and relations[0]["value"]==60.25
    shifted=result(twice,"p0",long_id,'{"x":22,"y":11}')
    moved=result(shifted,"inspect")
    assert moved["degrees_of_freedom"]==2 and shifted==twice
    horizontal=result(doc,"p1",long_id,'{"x":-42,"y":11}')
    assert result(horizontal,"inspect")["lines"][0]==dict(id=long_id,p0=dict(x=20,y=5),p1=dict(x=-42,y=5),length=62)
    # A shared corner follows a drag from either incident line, including
    # when the selected endpoint occurs later in the saved point list.
    pair=json.loads((FIXTURES/"graph-constrained-line.pcad").read_bytes())
    edges=pair["items"][0]["data"]["cad"]["nodes"]
    del edges[-2:]
    edges[-2]["relations"]=[dict(type="coincident",id="corner",
                                   a=dict(entityId="native-entity-1",point="p1"),
                                   b=dict(entityId="second",point="p1"))]
    edges.append(dict(type="sketchLine",id="second",sketchId=sketch,p0=dict(x=50,y=60),p1=dict(x=50,y=20)))
    for selected in ("native-entity-1","second"):
        joined=result(pair,"p1",selected,'{"x":56,"y":28}')
        report=result(joined,"inspect")
        assert report["degrees_of_freedom"]==6
        assert [r["p0"] for r in report["lines"]]==[dict(x=-50,y=20),dict(x=50,y=60)]
        assert all(r["p1"]==dict(x=56,y=28) for r in report["lines"])
    free_angle=json.loads((FIXTURES/"graph-constrained-line.pcad").read_bytes())
    del free_angle["items"][0]["data"]["cad"]["nodes"][-2]
    rotated=result(free_angle,"p1","native-entity-1",'{"x":74,"y":25.5}')
    segment=result(rotated,"inspect")["lines"][0]
    assert segment["p0"]==dict(x=-50,y=20)
    assert math.dist([segment["p1"][k] for k in ("x","y")],[49.901775,24.431131])<.001
    relations.append(dict(id="pin-end",type="fixed",anchor=dict(entityId=long_id,point="p1"),position=dict(x=-40.25,y=5)))
    assert "fixed" in solve(twice,"length",long_id,"66").stderr


def check_step_import(editor, directory, libs):
    fixtures=libs/"cad/tests/fixtures"
    source=fixtures/"step-bracket.step"
    expected=json.loads((fixtures/"step-bracket.pcad").read_bytes())
    listing=subprocess.check_output([editor,"--step-bodies",source])
    assert json.loads(listing)==[dict(id=900001,name="Puppy's L Ø")]
    drawing=directory/"import.pcad"
    subprocess.run([editor,"--import-step",source,drawing],check=True)
    assert json.loads(drawing.read_bytes())==expected
    assert expected["nodes"][0]["import"]["sha256"]==hashlib.sha256(source.read_bytes()).hexdigest()
    mesh=directory/"imported-bracket.stl"
    subprocess.run([editor,"--export",drawing,mesh],check=True)
    bounds,volume=triangulation(mesh)
    assert bounds==[(0,20),(0,20),(0,5)] and abs(volume-1500)<.001
    if os.environ.get("PUPPYCAD_TEST_FREECAD"):
        import FreeCAD
        import Part
        exchange=directory/"reexported-bracket.step"
        subprocess.run([editor,"--export",drawing,exchange],check=True)
        brep=Part.read(str(exchange))
        assert len(brep.Solids)==1 and brep.Solids[0].isValid()
        assert abs(brep.Volume-1500)<1e-6
        box=brep.BoundBox
        assert (box.XLength,box.YLength,box.ZLength)==(20,20,5)
    previous=drawing.read_bytes()
    assert subprocess.run([editor,"--import-step",source,drawing],capture_output=True).returncode
    assert drawing.read_bytes()==previous
    variants=[
        (".CENTI.",".KILO.","unit"),
        ("#132=FACETED_BREP_SHAPE_REPRESENTATION", "#132=UNRELATED_REPRESENTATION", "context"),
        ("#21,#20,#19,#18,#17,#16","#16,#17,#18,#19,#20,#21","winding"),
        ("#15,#14,#13,#12,#11,#10","#15,#14,#13,#12,#11,#15","repeats"),
        ("#104=FACE_OUTER_BOUND('',#103,.F.);","#104=FACE_OUTER_BOUND('',#103,.U.);","orientation"),
        ("#102,#105,#108","#102,#102,#108","distinct"),
        ("#132=FACETED_BREP_SHAPE_REPRESENTATION","#1=FACETED_BREP_SHAPE_REPRESENTATION","Duplicate"),
        ("END-ISO-10303-21;","END-ISO-10303-21; garbage","Unexpected"),
    ]
    for old,new,reason in variants:
        text=source.read_text()
        assert text.count(old)==1
        broken=directory/"invalid.step"
        broken.write_text(text.replace(old,new))
        out=directory/"invalid-import.pcad"
        proc=subprocess.run([editor,"--import-step",broken,out],text=True,capture_output=True)
        assert proc.returncode and reason in proc.stderr and not out.exists(),proc

    source=fixtures/"step-mounts.step"
    drawing=directory/"mounts.pcad"
    subprocess.run([editor,"--import-step",source,drawing,"--assembly"],check=True)
    assert json.loads(drawing.read_bytes())==json.loads((fixtures/"step-mounts.pcad").read_bytes())
    package=directory/"mounts.3mf"
    subprocess.run([editor,"--export",drawing,package],check=True)
    bodies=package_geometry(package)[1]
    assert len(bodies)==2
    assert [box for _,(box,_) in bodies]==[[(0,20),(0,20),(0,5)],[(60,80),(30,50),(5,10)]]
    assert all(abs(volume-1500)<.001 for _,(_,volume) in bodies)
    if os.environ.get("PUPPYCAD_TEST_FREECAD"):
        exchange=directory/"mounts.step"
        subprocess.run([editor,"--export",drawing,exchange],check=True)
        solids=Part.read(str(exchange)).Solids
        assert len(solids)==2 and all(solid.isValid() and abs(solid.Volume-1500)<.001 for solid in solids)
    original=source.read_text()
    renamed=directory/"duplicate-occurrence.step"
    renamed.write_text(original.replace("('right','right mount'","('left','right mount'")
                       .replace("('bracket','L bracket'","('world','L bracket'"))
    imported=directory/"duplicate-occurrence.pcad"
    subprocess.run([editor,"--import-step",renamed,imported,"--assembly"],check=True)
    model=json.loads(imported.read_bytes())
    assert model["parts"][0]["id"]=="step-part-302"
    left,right=model["assembly"]["instances"]
    assert left["id"]=="left" and right["id"]=="step-occurrence-420" and right["step_id"]=="left"
    assert left["assembly"]==right["assembly"]=="mount"

    for old,new,reason in [
        ("#403,#401);","#403,#401);\n#900101=CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#403,#401);","Ambiguous"),
        ("#402=ITEM_DEFINED_TRANSFORMATION('','',#202,#200)","#402=ITEM_DEFINED_TRANSFORMATION('','',#204,#200)","belong"),
        ("(6.,3.,.5)","(60.,3.,.5)","500 mm"),
        ("#312,#302,$","#312,#322,$","cycle"),
    ]:
        assert original.count(old)==1
        broken=directory/"broken-mount.step"
        broken.write_text(original.replace(old,new))
        out=directory/"rejected-mounts.pcad"
        result=subprocess.run([editor,"--import-step",broken,out,"--assembly"],capture_output=True,text=True)
        assert result.returncode and reason in result.stderr and not out.exists(),result


def run(directory):
    editor, edit = directory/"puppycad", directory/"edit"
    compiler = os.environ.get("FLEX_COMPILER", "flexscript")
    env = dict(os.environ, FLEXSCRIPT_LIB=os.environ.get("FLEXSCRIPT_LIB", str(ROOT.parent/"flexscript/libs")))
    for source, binary in [(ROOT/"main.flex", editor), (ROOT/"tests/edit.flex", edit), (ROOT/"tests/line-edit.flex",directory/"line-edit")]:
        subprocess.run([compiler, "--native-pipeline", "direct", source, "-o", binary], env=env, check=True)
    solid_fixtures = {"assembly", "extrude-options", "extruded-rectangle", "face-sketch",
                      "feature-tree", "revolve-loft-finish", "segmented-primitives", "segmented-slot"}
    exported = set()
    for model in sorted(FIXTURES.glob("*.pcad")):
        subprocess.run([edit, model], check=True)
        out = directory/"roundtrip.pcad"
        subprocess.run([editor, "--roundtrip", model, out], check=True)
        assert json.loads(out.read_bytes()) == json.loads(model.read_bytes()), model
        native_package = directory/f"{model.stem}-fixture.3mf"
        rust_package = directory/f"{model.stem}-fixture-rust.3mf"
        reference = subprocess.run([ROOT/"rust/target/debug/puppycad", "--export", model, rust_package], capture_output=True)
        actual = subprocess.run([editor, "--export", out, native_package], capture_output=True)
        assert (actual.returncode == 0) == (reference.returncode == 0), (model, actual.stderr, reference.stderr)
        if reference.returncode == 0:
            compare_solid_packages(native_package, rust_package)
            exported.add(model.stem)
        else:
            assert not native_package.exists(), model
    assert solid_fixtures <= exported, solid_fixtures - exported

    check_step_import(editor,directory,Path(env["FLEXSCRIPT_LIB"]))

    sensor = ROOT / "tests/fixtures/sensor-unit.pcad"
    out = directory / "sensor.pcad"
    subprocess.run([editor, "--roundtrip", sensor, out], check=True)
    assert json.loads(out.read_bytes()) == json.loads(sensor.read_bytes())
    subprocess.run([editor, "--export", out, directory / "sensor.stl"], check=True)
    exports = sorted(directory.glob("sensor-*.stl"))
    assert len(exports) == 6
    lid = directory / "sensor-Enclosure-lid.stl"
    bounds, volume = triangulation(lid)
    for actual, expected in zip(bounds, [(0, 102), (-1.414214, 29.698494), (13, 44.112708)]):
        assert all(math.isclose(a, b, abs_tol=0.00001) for a, b in zip(actual, expected))
    assert math.isclose(volume, 102*42*2, rel_tol=0.00001)
    invalid = json.loads(sensor.read_bytes())
    invalid['schema_version'] = 2
    out.write_text(json.dumps(invalid))
    before = lid.read_bytes()
    assert subprocess.run([editor, "--export", out, directory / "sensor.stl"], capture_output=True).returncode != 0
    assert lid.read_bytes() == before

    future_part = json.loads((FIXTURES/"extruded-rectangle.pcad").read_bytes())
    future_part["items"][0]["data"]["cad"]["nodes"].append(dict(type="futureSolid",id="future"))
    future_schema = dict(version=9,items=[],payload="exact")
    for drawing in (future_part,future_schema,invalid):
        source = directory/"protected.pcad"
        content = ("  "+json.dumps(drawing,ensure_ascii=False,indent=3)+"\r\n").encode()
        source.write_bytes(content)
        subprocess.run([editor,"--roundtrip",source,out],check=True)
        assert out.read_bytes() == content
        stat = source.stat()
        subprocess.run([editor,"--roundtrip",source,source],check=True)
        assert source.stat().st_ino==stat.st_ino and source.stat().st_mtime_ns==stat.st_mtime_ns
    for content in (b"{bad json}", b'{"version":4,"items":[]}\0junk', b'{"unrelated":true}'):
        source.write_bytes(content)
        before=out.read_bytes()
        assert subprocess.run([editor,"--roundtrip",source,out],capture_output=True).returncode
        assert out.read_bytes()==before

    check_line_solver(editor,directory/"line-edit",directory,Path(env["FLEXSCRIPT_LIB"]))
    check_faces(editor, directory)
    check_curves(editor, directory)
    check_closed_offset(editor, directory/"line-edit", directory)
    check_offset_profiles(editor, directory/"line-edit", directory)
    check_revolves(editor, directory)
    check_lofts(editor, directory)
    check_edge_finishes(editor, directory)
    check_step(editor, directory)
    check_profiles(editor, directory)
    check_line_contours(editor, directory)
    check_assembly(editor, directory)
    check_free_mates(editor, directory)
    stl = directory/"body.stl"
    for filename, volume in [("extruded-rectangle", 960000), ("extrude-options", 816000), ("feature-tree", 537500)]:
        subprocess.run([editor, "--export", FIXTURES/(filename+".pcad"), stl], check=True)
        assert math.isclose(triangulation(stl)[1], volume)
        reference = directory/"names-rust.3mf"
        package = directory/"names-flex.3mf"
        source = FIXTURES/(filename+".pcad")
        subprocess.run([ROOT/"rust/target/debug/puppycad","--export",source,reference],check=True)
        subprocess.run([editor,"--export",source,package],check=True)
        assert package_geometry(package)==package_geometry(reference), filename

    # Only active boolean features combine manufacturing bodies. Moving the
    # rollback bar before the join restores the extrusion's authored name.
    model = json.loads((FIXTURES/"feature-tree.pcad").read_bytes())
    data = model["items"][0]["data"]
    tree, graph = data["tree"], data["cad"]
    joined = dict(next(n for n in graph["nodes"] if n["id"]=="native-extrude-1"),
                  id="separate-body", name="Another extrusion", depth=20)
    graph["nodes"].append(joined)
    graph["rootNodeIds"].append(joined["id"])
    tree["orderedNodeIds"].append(joined["id"])
    for rollback, names in [(None, ["Body 1","Body 2"]), (1, ["Extrusion 1"])]:
        if rollback is not None:
            tree["rollbackIndex"] = rollback
        source = directory/"manufacturing-names.pcad"
        source.write_text(json.dumps(model))
        subprocess.run([editor,"--export",source,package],check=True)
        subprocess.run([ROOT/"rust/target/debug/puppycad","--export",source,reference],check=True)
        result = package_geometry(package)
        assert result==package_geometry(reference) and [name for name,_ in result[1]]==names

    part = directory/"edited.pcad"
    original = json.loads((FIXTURES/"extruded-rectangle.pcad").read_bytes())
    nodes = original["items"][0]["data"]["cad"]["nodes"]
    rectangle = nodes[4]
    for fixed, expected in [((), (-80.5, 80.5)), (("p0",), (-80, 81)), (("p1",), (-81, 80)), (("p0", "p1"), None)]:
        nodes[3]["relations"] = [
            {"id": "fix-"+point, "type": "fixed", "anchor": {"entityId": rectangle["id"], "point": point}, "position": rectangle[point]}
            for point in fixed
        ]
        part.write_text(json.dumps(original))
        result = subprocess.run([edit, part, rectangle["id"], "width", "161"], capture_output=True)
        if expected is None:
            assert result.returncode == 1 and b"Both rectangle corners are fixed" in result.stderr
            continue
        assert result.returncode == 0, result.stderr
        updated = json.loads(result.stdout)["items"][0]["data"]["cad"]["nodes"]
        assert updated[3]["relations"][-1]["value"] == 161
        assert tuple(updated[4][c]["x"] for c in ("p0", "p1")) == expected
        part.write_bytes(result.stdout)
        subprocess.run([editor, "--export", part, stl], check=True)
        bounds, volume = triangulation(stl)
        assert bounds == [expected, (-50, 50), (0, 60)] and math.isclose(volume, 966000)
    multi = copy.deepcopy(original)
    graph = multi["items"][0]["data"]["cad"]["nodes"]
    second = dict(graph[5], id="second-body", depth=8)
    graph.append(second)
    multi["items"][0]["name"] = 'Plate "wide" & β'
    graph[5]["name"] = "Block <one>"
    second["name"] = "Block & two"
    part.write_text(json.dumps(multi))
    package = directory/"part.3MF"
    subprocess.run([editor, "--export", part, package], check=True)
    title, bodies = package_geometry(package)
    assert title == multi["items"][0]["name"]
    assert [name for name, _ in bodies] == ["Block <one>", "Block & two"]
    for (_, (bounds, volume)), depth in zip(bodies, [60, 8]):
        assert bounds == [(-80, 80), (-50, 50), (0, depth)] and math.isclose(volume, 16000*depth)
    single_part = directory / "two-bodies.step"
    subprocess.run([editor, "--export", part, single_part], check=True)
    step = single_part.read_bytes()
    assert step.count(b"=PRODUCT(") == 1 and step.count(b"=FACETED_BREP(") == 2
    assert b'Plate "wide" &' in step and b'Block <one>' in step and b'Block & two' in step
    original_package = package.read_bytes()
    subprocess.run([editor, "--export", part, package], check=True)
    assert package.read_bytes() == original_package
    assert subprocess.run([editor, "--export", FIXTURES/"splines.pcad", package], capture_output=True).returncode
    assert package.read_bytes() == original_package
    subprocess.run([editor, "--export", part, stl], check=True)
    assert triangulation(stl)[1] == 960000
    extra = directory/"body-2.stl"
    assert math.isclose(triangulation(extra)[1], 128000)
    extra.unlink()
    extra.mkdir()
    original_export = stl.read_bytes()
    graph[5]["depth"] = 7
    part.write_text(json.dumps(multi))
    assert subprocess.run([editor, "--export", part, stl], capture_output=True).returncode
    assert stl.read_bytes() == original_export and extra.is_dir()
    before = stl.read_bytes()
    result = subprocess.run([editor, "--export", FIXTURES/"splines.pcad", stl], capture_output=True)
    assert result.returncode and stl.read_bytes() == before
    print("v4 round trips, profiles, face attachments, dimension edits, STEP, STL/3MF topology and failed-export preservation passed")


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as temp:
        run(Path(temp))
