"""Drive the native editor on a headless Xvfb display and compare its export with Rust."""
from pathlib import Path
import ctypes as C
import os
import secrets
import shutil
import subprocess
import time
import tempfile
import math
import json
import re
from project import compare_solid_packages, edge_document, loft_document, revolve_document, free_mate_document, horizontal_surfaces, package_geometry, triangulation


# Xvfb has no output window or physical input devices. Keep all child
# processes off the desktop display and GPU, even when launched from a desktop.
root = Path(__file__).resolve().parents[1]
output_dir = root / 'workdir/flexscript'
xvfb = shutil.which('Xvfb') or output_dir/'headless/Xvfb'
if not os.access(xvfb, os.X_OK):
    raise SystemExit('The native window test requires Xvfb (xorg-server-xvfb).')
for variable in ('DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'XAUTHORITY'):
    os.environ.pop(variable, None)
os.environ.update(SDL_VIDEODRIVER='x11', SDL_AUDIODRIVER='dummy', LIBGL_ALWAYS_SOFTWARE='1',
                  GALLIUM_DRIVER='llvmpipe', LP_NUM_THREADS='2')
os.nice(10)
with tempfile.TemporaryDirectory() as tmp:
    auth = Path(tmp) / 'auth'
    auth.touch()
    number = next((n for n in range(90, 150) if not Path(f'/tmp/.X11-unix/X{n}').exists()))
    display = f':{number}'
    subprocess.run(['xauth', '-f', auth, 'add', display, '.', secrets.token_hex(16)], check=True)
    server = subprocess.Popen([xvfb, display, '-screen', '0', '990x720x24', '-noreset', '-nolisten', 'tcp', '-auth', auth], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, start_new_session=True)
    os.environ.update(DISPLAY=display, XAUTHORITY=str(auth))
    app = None
    try:
        xlib = C.CDLL('libX11.so.6')
        xtest = C.CDLL('libXtst.so.6')
        pointer = C.c_void_p
        XID = C.c_ulong
        integer = C.c_int
        xlib.XOpenDisplay.argtypes = [C.c_char_p]
        xlib.XOpenDisplay.restype = pointer
        for _ in range(80):
            connection = xlib.XOpenDisplay(None)
            if connection:
                break
            if server.poll() is not None:
                raise RuntimeError(server.stderr.read().decode())
            time.sleep(0.05)
        assert connection
        xlib.XDefaultRootWindow.argtypes = [pointer]
        xlib.XDefaultRootWindow.restype = XID
        xlib.XQueryTree.argtypes = [pointer, XID, C.POINTER(XID), C.POINTER(XID), C.POINTER(C.POINTER(XID)), C.POINTER(C.c_uint)]
        xlib.XFree.argtypes = [pointer]
        xlib.XSetInputFocus.argtypes = [pointer, XID, integer, XID]
        xlib.XStringToKeysym.argtypes = [C.c_char_p]
        xlib.XStringToKeysym.restype = XID
        xlib.XKeysymToKeycode.argtypes = [pointer, XID]
        xlib.XSync.argtypes = [pointer, integer]
        xtest.XTestFakeKeyEvent.argtypes = [pointer, C.c_uint, integer, XID]

        def title(window):
            return subprocess.run(['xprop', '-id', hex(window), '_NET_WM_NAME'], text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout

        def open_app(path, ready="selects a dimension"):
            if path is None:
                process = subprocess.Popen([root/"run.sh"], cwd=tmp)
            else:
                process = subprocess.Popen([output_dir/'puppycad', path])
            for _ in range(120):
                root_window = XID()
                parent = XID()
                children = C.POINTER(XID)()
                count = C.c_uint()
                xlib.XQueryTree(connection, xlib.XDefaultRootWindow(connection), C.byref(root_window), C.byref(parent), C.byref(children), C.byref(count))
                ids = list(children[:count.value]) if count.value else []
                if children:
                    xlib.XFree(children)
                for window in ids:
                    if ready in title(window):
                        xlib.XSetInputFocus(connection, window, 2, 0)
                        xlib.XSync(connection, 0)
                        time.sleep(0.15)
                        return (process, window)
                if process.poll() is not None:
                    raise RuntimeError('Editor exited during startup')
                time.sleep(0.05)
            process.terminate()
            process.wait()
            raise RuntimeError('Native test window missing')

        def key(name):
            parts = name.split('+')
            codes = [xlib.XKeysymToKeycode(connection, xlib.XStringToKeysym(stroke.encode())) for stroke in parts]
            for code in codes:
                xtest.XTestFakeKeyEvent(connection, code, 1, 0)
            for code in reversed(codes):
                xtest.XTestFakeKeyEvent(connection, code, 0, 0)
            xlib.XSync(connection, 0)
            time.sleep(0.12)

        xlib.XWarpPointer.argtypes = [pointer, XID, XID, integer, integer, C.c_uint, C.c_uint, integer, integer]
        xtest.XTestFakeButtonEvent.argtypes = [pointer, C.c_uint, integer, XID]

        def click(window, x, y, button=1, count=1):
            xlib.XWarpPointer(connection, 0, window, 0, 0, 0, 0, x, y)
            for _ in range(count):
                xtest.XTestFakeButtonEvent(connection, button, 1, 0)
                xtest.XTestFakeButtonEvent(connection, button, 0, 0)
            xlib.XSync(connection, 0)
            time.sleep(0.15)

        def focus_search(window, x):
            from PIL import Image
            click(window, x, 100)
            # SDL must have started text input before X sends the first key.
            frame = Path(tmp) / 'search-focus.png'
            for _ in range(60):
                subprocess.run(['magick', 'import', '-window', hex(window), frame], check=True)
                with Image.open(frame) as img:
                    if img.convert('RGB').getpixel((20, 113)) == (143, 217, 184):
                        return
                time.sleep(.05)
            raise AssertionError('Search field did not gain focus')

        def drag(window, button, start, end, cancel=False, preview=None, dragging="Dragging hinge"):
            def move(pos):
                xlib.XWarpPointer(connection, 0, window, 0, 0, 0, 0, *pos)
                xlib.XSync(connection, 0)
                time.sleep(.15)
            move(start)
            xtest.XTestFakeButtonEvent(connection, button, 1, 0)
            xlib.XSync(connection, 0)
            time.sleep(.15)
            move(end)
            if preview:
                preview()
            if cancel:
                verify(window, dragging)
                key('Escape')
            xtest.XTestFakeButtonEvent(connection, button, 0, 0)
            xlib.XSync(connection, 0)
            time.sleep(.15)

        def verify(window, wanted):
            for _ in range(150):
                actual = title(window)
                if wanted in actual:
                    print(wanted, flush=True)
                    return
                time.sleep(0.05)
            raise AssertionError((wanted, actual))

        def capture(window, name, sketch, crossing_y=None, surface=(119, 163, 130)):
            from PIL import Image
            output = output_dir / name
            for _ in range(40):
                subprocess.run(['magick', 'import', '-window', hex(window), output], check=True)
                pixels = Image.open(output).convert('RGB')
                colors = dict(((rgb, c) for c, rgb in pixels.crop((0, 80, pixels.width, pixels.height)).getcolors(pixels.width * pixels.height)))
                if sketch and colors.get((143, 217, 184), 0) > 100:
                    if crossing_y is None or sum(pixels.getpixel((x, crossing_y)) == (143, 217, 184)
                                                  for x in range(pixels.width)) == 4:
                        return
                if not sketch and colors.get(surface, 0) > 1000:
                    return
                time.sleep(0.1)
            raise AssertionError((name, sorted(colors.items(), key=lambda kv: -kv[1])[:8]))
        def type_value(text):
            for char in text:
                key({'.':'period','-':'minus',' ':'space','=':'equal','#':'Shift_L+3','+':'Shift_L+equal','*':'Shift_L+8'}.get(char,char))

        app,window=open_app(None,"Click two rectangle corners")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads((Path(tmp)/"untitled.pcad").read_bytes())["version"]==4
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Construction plate.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        click(window,400,398)
        click(window,560,318)
        key("c")
        click(window,480,358)
        click(window,496,358)
        key("l")
        click(window,440,334)
        click(window,520,334)
        key("w")
        verify(window,"Pick sketch geometry, then Apply")
        click(window,460,334)
        verify(window,"1 selected")
        key("Return")
        verify(window,"Construction geometry")
        key("w")
        click(window,460,334)
        click(window,496,358)
        verify(window,"2 selected")
        click(window,496,358)
        verify(window,"1 selected")
        click(window,496,358)
        click(window,50,56)
        verify(window,"Construction geometry")
        def construction_flags():
            key("Control_L+s")
            verify(window,"Saved")
            nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
            return [n.get("construction") or n.get("primitive",{}).get("construction",False)
                    for n in nodes if n.get("sketchId")=="native-sketch-1" and n["type"].startswith("sketch")]
        assert construction_flags()==[False,True,True]
        key("Control_L+z")
        assert construction_flags()==[False,False,True]
        key("Control_L+y")
        assert construction_flags()==[False,True,True]
        key("w")
        click(window,400,358)
        key("Escape")
        verify(window,"Construction edit cancelled")
        assert construction_flags()==[False,True,True]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        capture(window,"construction-geometry.png",True)
        from PIL import Image
        with Image.open(output_dir/"construction-geometry.png") as shot:
            pixels=shot.convert("RGB")
            counts=pixels.crop((390,305,570,415)).getcolors(180*110)
            assert sum(n for n,c in counts if c==(56,148,189))>80,counts
            line=[pixels.getpixel((x,334))==(56,148,189) for x in range(442,518)]
            if not any(line):  # OpenGL rasterizes horizontal edges on a pixel boundary.
                line=[pixels.getpixel((x,333))==(56,148,189) for x in range(442,518)]
            assert sum(a!=b for a,b in zip(line,line[1:]))>=10,line
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"construction-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        _,objects=package_geometry(reference)
        assert len(objects)==1 and math.isclose(objects[0][1][1],8000,abs_tol=.001)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Split ellipse.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        key("Shift_L+e")
        click(window,480,358)
        click(window,560,358)
        click(window,480,318)
        verify(window,"Curve added")
        click(window,474,55)  # Split ellipse
        verify(window,"Pick a cut on an ellipse or elliptic arc")
        click(window,560,358)
        verify(window,"Pick the second cut")
        click(window,560,358)
        verify(window,"distinct interior split points")
        xlib.XWarpPointer(connection,0,window,0,0,0,0,480,318)
        xlib.XSync(connection,0)
        time.sleep(.15)
        subprocess.run(["magick","import","-window",hex(window),output_dir/"ellipse-split-picks.png"],check=True)
        click(window,480,318)
        verify(window,"Ellipse split")
        click(window,400,358)
        verify(window,"Ellipse split")
        def split_nodes():
            key("Control_L+s")
            verify(window,"Saved")
            return json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        nodes=split_nodes()
        joins=lambda nodes: [r for n in nodes if n["type"]=="sketch" for r in n["relations"] if r["type"]=="sameEllipse"]
        assert [(r["a"],r["b"]) for r in joins(nodes)]==[("native-entity-1","native-entity-2"),("native-entity-1","native-entity-3")]
        key("Control_L+z")
        assert len(joins(split_nodes()))==1
        key("Control_L+y")
        assert len(joins(split_nodes()))==2
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"Sweep 90 deg")
        key("Tab")
        verify(window,"Width 40 mm")
        key("F2")
        type_value("50")
        key("Return")
        verify(window,"Edited")
        nodes=split_nodes()
        pieces=[n["primitive"] for n in nodes if n.get("primitive",{}).get("type")=="ellipticArc"]
        assert len(pieces)==3 and all(n["width"]==50 and n["height"]==20 for n in pieces)
        assert all(math.isclose(p["sweep"],a,abs_tol=1e-10) for p,a in zip(pieces,(math.pi/2,math.pi/2,math.pi),strict=True))
        key("Escape")
        key("e")
        verify(window,"Extruded")
        split_nodes()
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"split-ellipse-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Elliptical washer.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        click(window,576,20)  # More tools
        click(window,576,20)  # Conics
        click(window,236,55)  # Ellipse
        verify(window,"Click the ellipse center")
        click(window,480,358)
        click(window,480,358)
        verify(window,"Choose a first axis endpoint away from the center")
        click(window,540,278)
        click(window,600,198)
        verify(window,"Choose a height away from the first axis")
        xlib.XWarpPointer(connection,0,window,0,0,0,0,460,318)
        xlib.XSync(connection,0)
        capture(window,"rotated-ellipse-preview.png",True)
        click(window,460,318)
        verify(window,"Curve added")
        key("Shift_L+c")
        verify(window,"Click the first circle point")
        click(window,464,358)
        click(window,480,342)
        key("BackSpace")
        verify(window,"Click the second circle point")
        click(window,480,342)
        click(window,496,358)
        verify(window,"Curve added")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        def conic_shapes():
            return [n["primitive"] for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"] if n["type"]=="sketchPrimitive"]
        ellipse,=conic_shapes()
        assert ellipse["type"]=="ellipse" and ellipse["width"]==50 and ellipse["height"]==20
        assert abs(ellipse["rotation"]-math.degrees(math.atan2(4,3)))<.000001 and ellipse["segments"]==128
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        ellipse,circle=conic_shapes()
        assert circle["type"]=="circle" and circle["center"]==dict(x=0,y=0) and circle["radius"]==4 and circle["segments"]==64
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"conics-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        volume=package_geometry(ref)[1][0][1][1]
        expected=10*(64*25*10*math.sin(math.tau/128)-32*4**2*math.sin(math.tau/64))
        assert math.isclose(volume,expected,rel_tol=.000001)
        key("Escape")
        app.wait(3)

        app,window=open_app(path)
        key("F8")
        verify(window,"Width 50 mm   DOF 8")
        key("F3")
        type_value("span = 3 cm")
        key("Return")
        verify(window,"Variables updated")
        key("F2")
        type_value("2 * #span")
        key("Return")
        verify(window,"Edited")
        key("Tab")
        verify(window,"Height 20 mm   DOF 7")
        key("F2")
        type_value("24 mm")
        key("Return")
        verify(window,"Edited")
        key("Tab")
        verify(window,"Rotation 53.13")
        key("F2")
        type_value("-30 deg")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"Rotation 53.13")
        key("Control_L+y")
        verify(window,"Rotation -30 deg   DOF 5")
        key("r")
        verify(window,"Reference Rotation -30 deg   DOF 6")
        key("F2")
        verify(window,"Reference ellipse dimension is measured")
        key("r")
        verify(window,"Rotation -30 deg   DOF 5")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"ellipse-rotation-dimension.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        ellipse,circle=conic_shapes()
        assert (ellipse["width"],ellipse["height"],ellipse["rotation"])==(60,24,-30)
        sketch=next(n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"] if n["type"]=="sketch")
        assert [(r["type"],r["value"]) for r in sketch["relations"]]==[("width",60),("height",24),("rotation",-30)]
        assert sketch["relations"][0]["expression"]=="2 * #span"
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"washer-driven-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_name("Elliptical washer-2.3mf"),ref)
        expected=10*(64*30*12*math.sin(math.tau/128)-32*4**2*math.sin(math.tau/64))
        assert math.isclose(package_geometry(ref)[1][0][1][1],expected,rel_tol=.000001)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Tangent join.pcad"
        path.write_bytes((root/"rust/fixtures/tangent-arc.pcad").read_bytes())
        app,window=open_app(path,"Click two rectangle corners")
        def smooth_nodes():
            key("Control_L+s")
            verify(window,"Saved")
            return {n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        key("j")
        click(window,560,358)
        verify(window,"Pick the curve that should follow")
        drag(window,1,(640,358),(680,358),cancel=True,dragging="Dragging joined curve")
        verify(window,"Smooth drag cancelled")
        drag(window,1,(640,358),(680,358))
        verify(window,"Smooth point moved")
        moved=smooth_nodes()
        assert moved["native-entity-1"]["p1"]==dict(x=50,y=0)
        assert moved["native-entity-2"]["arc"]["center"]==dict(x=50,y=20)
        assert moved["native-entity-3"]["spline"]["points"][0]==dict(x=70,y=20)
        key("Control_L+z")
        assert smooth_nodes()["native-entity-1"]["p1"]==dict(x=40,y=0)
        key("j")
        click(window,560,358)
        click(window,697,335)
        verify(window,"Smooth join updated")
        assert len(smooth_nodes()["native-sketch-1"]["relations"])==1
        key("Control_L+z")
        assert len(smooth_nodes()["native-sketch-1"]["relations"])==2
        key("Control_L+y")
        assert len(smooth_nodes()["native-sketch-1"]["relations"])==1
        key("j")
        click(window,560,358)
        click(window,697,335)
        verify(window,"Smooth join updated")
        assert len(smooth_nodes()["native-sketch-1"]["relations"])==2
        key("Shift_L+j")
        verify(window,"Pick a curve endpoint")
        click(window,688,118)
        verify(window,"Click the new tangent arc endpoint")
        click(window,480,358,button=5)  # Zoom out to fit the full preview.
        subprocess.run(["magick","import","-window",hex(window),output_dir/"tangent-arc-preview.png"],check=True)
        click(window,480,358)
        verify(window,"Tangent arc added")
        nodes=smooth_nodes()
        joins=nodes["native-sketch-1"]["relations"]
        assert joins[-1]["a"]==dict(entityId="native-entity-3",point="point2") and joins[-1]["b"]==dict(entityId="native-entity-4",point="p0")
        assert nodes["native-entity-4"]["arc"]["segments"]==128
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"tangent-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        damaged=json.loads(path.read_bytes())
        spline=next(n for n in damaged["items"][0]["data"]["cad"]["nodes"] if n["type"]=="sketchSpline")
        spline["spline"]["points"][1]["x"]+=1
        invalid=Path(tmp)/"damaged-tangent.pcad"
        invalid.write_text(json.dumps(damaged))
        exported=path.with_suffix(".3mf")
        original=exported.read_bytes()
        rejected=subprocess.run([output_dir/"puppycad","--export",invalid,exported],capture_output=True)
        assert rejected.returncode and b"smooth join" in rejected.stderr and exported.read_bytes()==original

        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Create patterns.pcad"
        drawing=json.loads((root/"rust/fixtures/patterns.pcad").read_bytes())
        nodes=drawing["items"][0]["data"]["cad"]["nodes"]
        nodes[:]=[n for n in nodes if n["type"] in ("referencePlane","sketch") or
                  n["id"] in ("native-entity-1","native-entity-7","native-entity-8")]
        next(n for n in nodes if n["type"]=="sketch")["relations"]=[]
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Click two rectangle corners")
        key("F5")
        click(window,536,318)
        verify(window,"1 selected")
        key("g")
        verify(window,"linear-pattern-1   Columns   2")
        click(window,500,238)
        verify(window,"1 selected")
        key("c")
        verify(window,"circular-pattern-1   Count   4")
        key("r")
        verify(window,"Pick the rotation center")
        click(window,480,158)
        verify(window,"circular-pattern-1   Count   4")
        key("Control_L+s")
        verify(window,"Saved")
        made=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        grid,ring=next(n for n in made if n["type"]=="sketch")["relations"]
        assert grid["sources"]==["native-entity-1"] and ring["sources"]==["native-entity-7"]
        assert ring["centerAnchor"]==dict(entityId="native-entity-8",point="center") and ring["center"]==dict(x=0,y=50)
        assert len(grid["instances"])==1 and len(ring["instances"])==3
        key("Escape")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Pattern handles.pcad"
        path.write_bytes((root/"rust/fixtures/patterns.pcad").read_bytes())
        app,window=open_app(path,"Click two rectangle corners")
        key("F5")
        verify(window,"linear-pattern-1   Columns   3")

        def pattern_frame(name):
            from PIL import Image
            target=output_dir/name
            subprocess.run(["magick","import","-window",hex(window),target],check=True)
            with Image.open(target) as img:
                return img.convert("RGB").crop((0,76,img.width,img.height))

        def handle_centers(frame):
            # The white rim belongs to the draggable handles. Pick the actual drawn controls.
            w,h=frame.size
            pixels=frame.load()
            rim={(x,y+76) for y in range(h) for x in range(w) for r,g,b in [pixels[x,y]]
                 if 220<=r<=224 and 238<=g<=242 and 245<=b<=250}
            centers=[]
            while rim:
                pending=[rim.pop()]
                group=[]
                while pending:
                    x,y=pending.pop()
                    group.append((x,y))
                    for dx in (-1,0,1):
                        for dy in (-1,0,1):
                            pt=(x+dx,y+dy)
                            if pt in rim:
                                rim.remove(pt)
                                pending.append(pt)
                if len(group)>20:
                    xs,ys=zip(*group)
                    centers.append((round((min(xs)+max(xs))/2),round((min(ys)+max(ys))/2)))
            assert len(centers)==2,centers
            return centers

        def saved_pattern():
            key("Control_L+s")
            verify(window,"Saved")
            nodes={n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
            return (*nodes["native-sketch-1"]["relations"],nodes)

        before=pattern_frame("pattern-grid-handles.png")
        row,column=sorted(handle_centers(before))
        scale=(column[0]-row[0])/20
        end=(column[0]+round(5*scale),column[1]-round(2*scale))
        def check_preview():
            verify(window,"Dragging pattern")
            assert before.tobytes()!=pattern_frame("pattern-grid-preview.png").tobytes()
        drag(window,1,column,end,cancel=True,preview=check_preview,dragging="Dragging pattern")
        verify(window,"Pattern drag cancelled")
        assert before.tobytes()==pattern_frame("pattern-grid-cancelled.png").tobytes()
        drag(window,1,column,end)
        grid,ring,nodes=saved_pattern()
        def near(actual,expected):
            assert abs(actual-expected)<.2,(actual,expected)
        near(grid["step"]["x"],25)
        near(grid["step"]["y"],2)
        near(nodes["native-entity-2"]["primitive"]["center"]["x"],35)
        assert grid["rowStep"]==dict(x=0,y=15)
        key("Control_L+z")
        assert saved_pattern()[0]["step"]==dict(x=20,y=0)
        key("Control_L+y")
        assert saved_pattern()[0]["step"]==grid["step"]
        # Shift holds the original row direction even with horizontal pointer movement.
        shift=xlib.XKeysymToKeycode(connection,xlib.XStringToKeysym(b"Shift_L"))
        xtest.XTestFakeKeyEvent(connection,shift,1,0)
        xlib.XSync(connection,0)
        try:
            drag(window,1,row,(row[0]+round(2*scale),row[1]-round(4*scale)))
        finally:
            xtest.XTestFakeKeyEvent(connection,shift,0,0)
            xlib.XSync(connection,0)
        shifted=saved_pattern()[0]["rowStep"]
        near(shifted["x"],0)
        near(shifted["y"],19)
        key("Control_L+z")
        drag(window,1,row,(row[0]+round(2*scale),row[1]-round(4*scale)))
        grid,_,nodes=saved_pattern()
        near(grid["rowStep"]["x"],2)
        near(grid["rowStep"]["y"],19)
        near(nodes["native-entity-4"]["primitive"]["center"]["y"],29)

        key("p")
        verify(window,"circular-pattern-1   Count   4")
        center,sweep=sorted(handle_centers(pattern_frame("pattern-ring-handles.png")),key=lambda pt:pt[1])
        drag(window,1,sweep,(center[0]-(sweep[1]-center[1]),center[1]))
        _,ring,nodes=saved_pattern()
        assert abs(ring["angle"]-270)<1,ring
        drag(window,1,center,(center[0]+round(5*scale),center[1]+round(3*scale)))
        _,ring,nodes=saved_pattern()
        assert ring["centerAnchor"]==dict(entityId="native-entity-8",point="center")
        assert ring["center"]==nodes["native-entity-8"]["primitive"]["center"]
        near(ring["center"]["x"],5)
        near(ring["center"]["y"],47)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F5")
        key("p")
        key("Tab")
        verify(window,"Angle (deg)")
        assert saved_pattern()[1]==ring
        # Pick an empty center and drag again. The former anchor stays in place.
        key("r")
        click(window,350,275)
        verify(window,"Angle (deg)")
        center_before=saved_pattern()[1]["center"]
        fixed_center=min(handle_centers(pattern_frame("pattern-free-center.png")),key=lambda p:abs(p[0]-350)+abs(p[1]-275))
        drag(window,1,fixed_center,(fixed_center[0]+8,fixed_center[1]-4))
        _,free,nodes=saved_pattern()
        assert "centerAnchor" not in free and free["center"]!=center_before
        near(nodes["native-entity-8"]["primitive"]["center"]["x"],5)
        assert free["center"]["x"]>center_before["x"] and free["center"]["y"]>center_before["y"]
        key("Escape")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Spline pattern center.pcad"
        drawing=json.loads((root/"rust/fixtures/patterns.pcad").read_bytes())
        nodes=drawing["items"][0]["data"]["cad"]["nodes"]
        spline=next(n for n in nodes if n["id"]=="native-entity-8")
        spline.pop("primitive")
        spline.update(type="sketchSpline",spline=dict(id=spline["id"],type="spline",mode="fit",closed=False,
                      points=[dict(x=-12,y=40),dict(x=0,y=50),dict(x=12,y=45)]))
        ring=next(n for n in nodes if n["type"]=="sketch")["relations"][1]
        ring["centerAnchor"]["point"]="point1"
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Click two rectangle corners")
        key("F5")
        key("p")
        verify(window,"circular-pattern-1   Count   4")
        center,sweep=sorted(handle_centers(pattern_frame("pattern-spline-center.png")),key=lambda pt:pt[1])
        key("r")
        click(window,*center)
        verify(window,"Count   4")
        drag(window,1,center,(center[0]-14,center[1]-10))
        _,ring,nodes=saved_pattern()
        assert ring["centerAnchor"]==dict(entityId="native-entity-8",point="point1")
        fit=nodes["native-entity-8"]["spline"]["points"]
        assert ring["center"]==fit[1] and fit[1]["x"]<0 and fit[1]["y"]>50
        assert fit[0]==dict(x=-12,y=40) and fit[2]==dict(x=12,y=45)
        key("Control_L+z")
        assert saved_pattern()[1]["center"]==dict(x=0,y=50)
        # Sweep snapping keeps the full-turn handle continuous across the angle seam.
        center,sweep=sorted(handle_centers(pattern_frame("pattern-spline-center.png")),key=lambda pt:pt[1])
        radius=sweep[1]-center[1]
        delta=math.radians(68)
        end=(center[0]-round(radius*math.sin(delta)),center[1]+round(radius*math.cos(delta)))
        xtest.XTestFakeKeyEvent(connection,shift,1,0)
        xlib.XSync(connection,0)
        try:
            drag(window,1,sweep,end)
        finally:
            xtest.XTestFakeKeyEvent(connection,shift,0,0)
            xlib.XSync(connection,0)
        assert saved_pattern()[1]["angle"]==285
        key("Escape")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Patterned pads.pcad"
        path.write_bytes((root/"rust/fixtures/patterns.pcad").read_bytes())
        app,window=open_app(path,"Click two rectangle corners")
        click(window,765,55)
        verify(window,"linear-pattern-1   Columns   3")
        def set_pattern_field(value,wanted):
            key("F2")
            type_value(value)
            key("Return")
            verify(window,wanted)
        set_pattern_field("2","Columns   2")
        key("Tab")
        set_pattern_field("3","Rows   3")
        key("Tab")
        set_pattern_field("25 mm","Step X (mm)   25")
        key("Tab")
        key("Tab")
        key("Tab")
        set_pattern_field("12","Row Y (mm)   12")
        key("p")
        verify(window,"circular-pattern-1   Count   4")
        set_pattern_field("3","Count   3")
        key("Tab")
        set_pattern_field("-180 deg","Angle (deg)   -180")
        key("Control_L+z")
        verify(window,"Angle (deg)   360")
        key("Control_L+y")
        verify(window,"Angle (deg)   -180")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-pattern.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        made={n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        grid,ring=made["native-sketch-1"]["relations"]
        assert grid["instances"][1:3]==[["native-entity-4"],["native-entity-5"]] and grid["step"]==dict(x=25,y=0)
        assert made["native-entity-5"]["primitive"]["center"]==dict(x=35,y=22)
        assert ring["centerAnchor"]==dict(entityId="native-entity-8",point="center") and ring["angle"]==-180
        assert made["native-entity-10"]["p0"]==dict(x=0,y=70) and "native-entity-11" not in made
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"DOF 9")
        key("Tab")
        verify(window,"Diameter 8 mm")
        key("F2")
        type_value("10")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        circles=[n["primitive"] for n in nodes if n.get("primitive",{}).get("type")=="circle"]
        assert len(circles)==6 and all(c["radius"]==5 for c in circles)
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"pattern-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        bounds,volume=package_geometry(ref)[1][0][1]
        assert bounds==[(5,15),(5,15),(0,10)] and math.isclose(volume,10*16*25*math.sin(math.pi/16),rel_tol=.00001)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Offset slot.pcad"
        drawing=json.loads((root/"rust/fixtures/segmented-slot.pcad").read_bytes())
        data=drawing["items"][0]["data"]
        data["cad"]["nodes"]=[n for n in data["cad"]["nodes"] if n["type"]!="extrude"]
        data["cad"]["rootNodeIds"].remove("extrude-1")
        data["tree"]["orderedNodeIds"].remove("extrude-1")
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Click two rectangle corners")
        click(window,576,20)  # More tools
        click(window,705,55)  # Offset
        verify(window,"Pick a source to offset")
        click(window,480,278)
        verify(window,"Set value")
        type_value("5")
        key("Return")
        verify(window,"Offset applied")
        click(window,480,258)
        verify(window,"Set value")
        type_value("8")
        key("Return")
        verify(window,"Offset applied")
        key("Control_L+s")
        verify(window,"Saved")
        made={n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        assert made["native-entity-2"]["primitive"]["width"]==56
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"offset-slot-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        bounds,volume=package_geometry(ref)[1][0][1]
        assert bounds==[(-78,78),(-28,28),(0,10)]
        assert math.isclose(volume,10*(100*16+12*(28**2-20**2)*math.sin(math.pi/12)),rel_tol=.00001)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Create offsets.pcad"
        drawing=json.loads((root/"rust/fixtures/offset.pcad").read_bytes())
        nodes=drawing["items"][0]["data"]["cad"]["nodes"]
        nodes[:]=[n for n in nodes if n["id"] not in {"native-entity-"+str(i) for i in (3,4,6,7,8)}]
        next(n for n in nodes if n["type"]=="sketch")["relations"]=[]
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"DOF 11")
        key("F4")
        type_value("40")
        key("Return")
        verify(window,"collapses")
        key("Escape")
        key("F4")
        type_value("-3.5 mm")
        key("Return")
        verify(window,"offset-chain-1   Offset -3.5 mm")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert not json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"][3]["relations"]
        key("Control_L+y")
        key("o")
        verify(window,"offset-chain-1")
        key("Tab")
        verify(window,"native-entity-1")
        for _ in range(4):
            key("Tab")
        verify(window,"Diameter 20 mm")
        key("F4")
        type_value("6 mm")
        key("Return")
        verify(window,"offset-distance-1   Offset 6 mm")
        key("Control_L+s")
        verify(window,"Saved")
        made={n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        assert made["native-entity-3"]["p1"]==made["native-entity-4"]["p0"]==dict(x=43.5,y=-3.5)
        assert made["native-entity-6"]["primitive"]["radius"]==16
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-created-offset.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        for _ in range(4):
            key("Tab")
        verify(window,"Diameter 20 mm")
        key("F2")
        type_value("24")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        made={n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        assert [made["native-entity-"+i]["primitive"]["radius"] for i in ("5","6")]==[12,18]
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"created-offset-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        bounds,volume=package_geometry(ref)[1][0][1]
        assert bounds==[(62,98),(-8,28),(0,10)]
        assert math.isclose(volume,10*16*(18**2-12**2)*math.sin(2*math.pi/32),rel_tol=.00001)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Offset sketch.pcad"
        path.write_bytes((root/"rust/fixtures/offset.pcad").read_bytes())
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"DOF 13")
        key("o")
        verify(window,"Offset 5 mm")
        key("F2")
        type_value("40")
        key("Return")
        verify(window,"collapses")
        key("Escape")
        key("F2")
        type_value("-3.5")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"Offset 5 mm")
        key("Control_L+y")
        verify(window,"Offset -3.5 mm")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-offset.png"],check=True)
        key("o")
        verify(window,"Offset 3 mm")
        key("F2")
        type_value("6")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        nodes={n["id"]:n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        assert nodes["native-entity-3"]["p1"]==nodes["native-entity-4"]["p0"]==dict(x=43.5,y=-3.5)
        assert nodes["native-entity-6"]["primitive"]["radius"]==16
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"offset-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        bounds,volume=package_geometry(ref)[1][0][1]
        assert bounds==[(64,96),(-6,26),(0,10)],bounds
        assert math.isclose(volume,10*16*(16**2-10**2)*math.sin(2*math.pi/32),rel_tol=.00001)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Mirror sketch.pcad"
        fixture=root/"rust/fixtures/mirror-symmetry.pcad"
        path.write_bytes(fixture.read_bytes())
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"DOF 11")
        drag(window,1,(609,315),(652,272))
        key("Control_L+s")
        verify(window,"Saved")
        def mirror_nodes():
            graph=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
            return {node["id"]:node for node in graph}
        graph=mirror_nodes()
        src=graph["native-entity-2"]["primitive"]
        copy=graph["native-entity-3"]["primitive"]
        assert src["center"]==dict(x=40,y=20) and copy["center"]==dict(x=-40,y=20)
        click(window,256,272)
        verify(window,"Diameter 24 mm")
        key("F2")
        type_value("28 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"Diameter 24 mm")
        key("Control_L+y")
        verify(window,"Diameter 28 mm")
        key("m")
        verify(window,"Pick the mirror line")
        click(window,480,200)
        verify(window,"Pick the point, line or circle to mirror")
        click(window,351,520)
        verify(window,"Mirror copy added")
        key("m")
        key("p")
        click(window,480,200)
        verify(window,"Pick the first symmetry anchor")
        click(window,480,100)
        verify(window,"Choose an anchor on a different entity")
        click(window,566,487)
        verify(window,"Pick the opposite symmetry anchor")
        click(window,394,487)
        verify(window,"Point symmetry updated")
        key("Control_L+s")
        verify(window,"Saved")
        graph=mirror_nodes()
        sketch=graph["native-sketch-1"]
        assert all(graph["native-entity-"+i]["primitive"]["radius"]==14 for i in ("2","3"))
        assert graph["native-entity-6"]["p0"]==dict(x=20,y=-30) and graph["native-entity-6"]["p1"]==dict(x=40,y=-45)
        assert not any(r["type"]=="symmetric" for r in sketch["relations"])
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        sketch=mirror_nodes()["native-sketch-1"]
        assert sketch["relations"][:2]==json.loads(fixture.read_bytes())["items"][0]["data"]["cad"]["nodes"][3]["relations"]
        key("m")
        click(window,480,200)
        verify(window,"Pick the point, line or circle to mirror")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-mirror.png"],check=True)
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)
        saved=path.read_bytes()
        # Extrude each region from the same reopened, edited sketch.
        for profile in (1,2):
            path=Path(tmp)/f"Mirrored profile {profile}.pcad"
            path.write_bytes(saved)
            app,window=open_app(path,"Click two rectangle corners")
            if profile==2:
                key("p")
                verify(window,"Profile 2")
            key("e")
            verify(window,"Extruded")
            key("Control_L+s")
            verify(window,"Saved")
            key("Control_L+Alt_L+e")
            verify(window,"Exported")
            ref=Path(tmp)/f"mirror-{profile}-rust.3mf"
            subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
            compare_solid_packages(path.with_suffix(".3mf"),ref)
            bounds,volume=package_geometry(ref)[1][0][1]
            assert bounds[1]==(6,34) and math.isclose(volume,10*16*14**2*math.sin(2*math.pi/32),rel_tol=.00001)
            if profile==1:
                first_x=bounds[0]
            else:
                assert first_x==(-bounds[0][1],-bounds[0][0])
            key("Escape")
            app.wait(3)

        stale=Path(tmp)/"Stale mirror.pcad"
        damaged=json.loads(path.read_bytes())
        nodes=damaged["items"][0]["data"]["cad"]["nodes"]
        next(n for n in nodes if n["id"]=="native-entity-2")["primitive"]["center"]["x"]=41
        original_bytes=json.dumps(damaged).encode()
        stale.write_bytes(original_bytes)
        app,window=open_app(stale,"Read-only")
        verify(window,"Saved geometry disagrees with symmetry: mirror-1")
        key("Control_L+s")
        key("Control_L+e")
        key("Escape")
        app.wait(3)
        assert stale.read_bytes()==original_bytes and not stale.with_suffix(".stl").exists()

        path=Path(tmp)/"Signed sweep.pcad"
        drawing=json.loads((root/"rust/fixtures/extruded-rectangle.pcad").read_bytes())
        nodes=drawing["items"][0]["data"]["cad"]["nodes"]
        sk="native-sketch-1"
        nodes[4:5]=[
            dict(id="chord",type="sketchLine",sketchId=sk,p0=dict(x=-10,y=0),p1=dict(x=10,y=0)),
            dict(id="rim",type="sketchArc",sketchId=sk,arc=dict(id="rim",type="arc",center=dict(x=0,y=0),
                radius=10,startAngle=0,sweep=math.pi,segments=64))]
        path.write_text(json.dumps(drawing))
        app,window=open_app(path)
        key("F8")
        key("Tab")
        verify(window,"Radius 10 mm   DOF 9")
        key("r")
        verify(window,"Reference Radius 10 mm")
        key("F2")
        verify(window,"Reference arc dimension is measured")
        key("Tab")
        verify(window,"Sweep 180 deg")
        key("F3")
        type_value("turn = 90 deg")
        key("Return")
        verify(window,"Variables updated")
        key("F2")
        type_value("-2 * #turn")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"Sweep 180 deg   DOF 9")
        key("Control_L+y")
        verify(window,"Sweep -180 deg   DOF 8")
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-arc-sweep.png"],check=True)
        from PIL import Image
        with Image.open(output_dir/"sketch-arc-sweep.png") as frame:
            frame=frame.convert("RGB")
            x=frame.width//2
            rim_y=max(y for y in range(80,frame.height) if frame.getpixel((x,y))==frame.getpixel((x+1,y)) and
                      frame.getpixel((x,y))[0]>180 and frame.getpixel((x,y))[2]<90)
        click(window,x,rim_y)
        verify(window,"Reference Radius 10 mm")
        key("Tab")
        verify(window,"Sweep -180 deg")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        graph=saved["items"][0]["data"]["cad"]["nodes"]
        arc=next(n["arc"] for n in graph if n["id"]=="rim")
        sketch=next(n for n in graph if n["id"]==sk)
        assert abs(arc["sweep"]+math.pi)<1e-10 and arc["radius"]==10 and arc["startAngle"]==0
        assert [(r["type"],r["value"]) for r in sketch["relations"]]==[("radius",10),("arcSweep",-180)]
        assert sketch["relations"][0]["reference"] and sketch["relations"][1]["expression"]=="-2 * #turn"
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"semicircle-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        bounds,volume=package_geometry(ref)[1][0][1]
        assert bounds[1]==(-10,0) and math.isclose(volume,60*32*50*math.sin(math.pi/32),rel_tol=.00001)
        key("F8")
        key("Tab")
        key("Tab")
        verify(window,"Sweep -180 deg   DOF 8")
        key("F2")
        type_value("90 deg")
        key("Return")
        verify(window,"Extrusion profile not found")
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)
        arc["sweep"]=math.pi
        path.write_text(json.dumps(saved))
        app,window=open_app(path,"Read-only")
        verify(window,"Arc dimension disagrees with saved geometry")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Circular pocket.pcad"
        original=json.loads((root/"rust/fixtures/face-sketch.pcad").read_bytes())
        path.write_text(json.dumps(original))
        app,window=open_app(path)
        click(window,696,55)
        verify(window,"Diameter 20 mm   DOF 3")
        key("F3")
        type_value("bore = 1 cm")
        key("Return")
        verify(window,"Variables updated")
        key("F2")
        type_value("#bore + 6 mm")
        key("Return")
        verify(window,"Edited")
        key("Tab")
        verify(window,"Radius 8 mm   DOF 2")
        key("r")
        verify(window,"Reference Radius 8 mm   DOF 2")
        key("F2")
        verify(window,"Reference circle dimension is measured")
        key("F3")
        type_value("bore = 2 cm")
        key("Return")
        verify(window,"Variables updated")
        key("Control_L+z")
        verify(window,"Reference Radius 8 mm   DOF 2")
        key("Control_L+y")
        verify(window,"Reference Radius 13 mm   DOF 2")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        nodes=saved["items"][0]["data"]["cad"]["nodes"]
        sketch=next(n for n in nodes if n["id"]=="native-sketch-2")
        circle=next(n for n in nodes if n["id"]=="native-entity-2")["primitive"]
        assert circle["radius"]==13 and circle["center"]==dict(x=0,y=0) and circle["segments"]==32
        assert [(r["type"],r["value"]) for r in sketch["relations"]]==[("diameter",26),("radius",13)]
        assert sketch["relations"][0]["expression"]=="#bore + 6 mm" and sketch["relations"][1]["reference"]
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"circle-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        key("F8")
        verify(window,"Diameter 26 mm   DOF 2")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-circle-diameter.png"],check=True)
        key("F2")
        key("End")
        type_value(" + 2 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        changed=next(n for n in nodes if n["id"]=="native-sketch-2")["relations"]
        assert changed[0]["id"]==sketch["relations"][0]["id"] and changed[0]["expression"]=="#bore + 6 mm + 2 mm"
        assert [r["value"] for r in changed]==[28,14]
        key("Delete")
        verify(window,"Diameter 28 mm   DOF 3")
        key("Control_L+z")
        verify(window,"Diameter 28 mm   DOF 2")
        key("Escape")
        key("Escape")
        app.wait(3)

        stale=Path(tmp)/"Stale circular pocket.pcad"
        circle["radius"]=12
        original_bytes=json.dumps(saved).encode()
        stale.write_bytes(original_bytes)
        app,window=open_app(stale,"Read-only")
        verify(window,"Circle dimension disagrees with saved geometry")
        key("Control_L+s")
        key("Control_L+e")
        key("Escape")
        app.wait(3)
        assert stale.read_bytes()==original_bytes and not stale.with_suffix(".stl").exists()

        path=Path(tmp)/"topology-mate.pcad"
        drawing=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        drawing["selectedPath"]=[2]
        node=drawing["items"][1]["data"]["cad"]["nodes"][4]
        node.pop("p0");node.pop("p1")
        node.update(type="sketchPrimitive",primitive=dict(type="circle",id=node["id"],center=dict(x=0,y=0),radius=15,segments=48))
        data=drawing["items"][2]["data"]
        data.update(instances=[dict(id="plate",name="Plate",partId="part-base"),
                               dict(id="pin",name="Pin",partId="part-arm",transform=dict(translation=dict(x=80,y=0,z=0)))],
                    mates=[],connectors=[],actuators=[])
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Assembly opened")
        key("3")
        verify(window,"Top view")
        from PIL import Image
        frame=Path(tmp)/"topology-top.png"
        subprocess.run(["magick","import","-window",hex(window),frame],check=True)
        with Image.open(frame).convert("RGB") as pixels:
            # Measure the projected silhouettes from the private display.
            mid=pixels.height//2
            def filled(x,y):
                r,g,b=pixels.getpixel((x,y))
                return g>r+20 and r>50 and g>b+15
            spans=[]
            for x in range(100,pixels.width-100):
                if filled(x,mid):
                    if spans and spans[-1][-1]==x-1:spans[-1].append(x)
                    else:spans.append([x])
            plate,pin=[span for span in spans if len(span)>15]
            edge_x=plate[len(plate)//2]
            edge_y=next(y for y in range(100,mid) if filled(edge_x,y))
            corner=(plate[0]-1,edge_y-1)
            rim=(pin[-1]+1,mid)
        for pick,kind in [(corner,"Vertex"),((edge_x,edge_y),"Edge")]:
            click(window,*pick)
            verify(window,"Selected Plate   |   "+kind)
            xlib.XWarpPointer(connection,0,window,0,0,0,0,*rim)
            xlib.XSync(connection,0)
            key("j")
            verify(window,"Curved edge selected   |   Choose a mate type")
            if kind=="Edge":
                subprocess.run(["magick","import","-window",hex(window),output_dir/"assembly-rim-mate.png"],check=True)
            key("1")
            verify(window,"Created mate   fasten")
            key("Control_L+s")
            verify(window,"Saved")
            assembly=json.loads(path.read_bytes())["items"][2]["data"]
            first,second=assembly["connectors"]
            assert first["instanceId"]=="plate" and second["instanceId"]=="pin"
            a,b=first["position"],second["position"]
            assert sorted(abs(a[k]) for k in "xy")==([20,20] if kind=="Vertex" else [0,20]) and a["z"]==10
            assert abs(b["x"])<.0001 and abs(b["y"])<.0001 and b["z"]==5
            if kind=="Vertex":
                key("Control_L+z")
                key("Control_L+s")
                verify(window,"Saved")
                assert json.loads(path.read_bytes())["items"]==drawing["items"]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"topology-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"placed-plates.pcad"
        drawing=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        drawing["selectedPath"]=[2]
        data=drawing["items"][2]["data"]
        data.update(instances=[dict(id="stand",name="Stand",partId="part-base"),
                               dict(id="tray",name="Tray",partId="part-base")],
                    mates=[],connectors=[],actuators=[])
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Assembly opened")
        key("t")
        click(window,70,193)
        verify(window,"Selected Tray")
        click(window,235,543)
        for x,y,value in [(112,563,"-87.5"),(244,592,"90"),(178,621,"0")]:
            click(window,x,y)
            verify(window,"Set value")
            type_value(value)
            key("Return")
            if value=="0":
                verify(window,"scale must be nonzero")
                key("Control_L+a")
                type_value("-1.5")
                key("Return")
            verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        instances=json.loads(path.read_bytes())["items"][2]["data"]["instances"]
        expected=dict(translation=dict(x=-87.5,y=0,z=0),rotation=dict(x=0,y=0,z=90),scale=dict(x=1,y=-1.5,z=1))
        assert instances[0]==data["instances"][0] and instances[1]["transform"]==expected
        subprocess.run(["magick","import","-window",hex(window),output_dir/"assembly-placement.png"],check=True)
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        previous=json.loads(path.read_bytes())["items"][2]["data"]["instances"][1]["transform"]
        assert previous=={k:v for k,v in expected.items() if k!="scale"}
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][2]["data"]["instances"]==instances
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"placed-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        # Older Rust exports reflect vertices without reversing triangle winding.
        title_actual,bodies=package_geometry(path.with_suffix(".3mf"))
        title_rust,reference=package_geometry(ref)
        assert title_actual==title_rust and [b[0] for b in bodies]==[b[0] for b in reference]==["Stand","Tray"]
        for (_, (bounds,volume)), (_, (rust_bounds,rust_volume)) in zip(bodies,reference,strict=True):
            assert bounds==rust_bounds and volume==abs(rust_volume)
        assert [volume for _,(_,volume) in bodies]==[16000,24000]
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"fastened-placement.pcad"
        arm=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        arm["selectedPath"]=[2]
        path.write_text(json.dumps(arm))
        app,window=open_app(path,"Assembly opened")
        key("t")
        click(window,70,193)
        verify(window,"Selected Upper arm")
        click(window,235,543)
        click(window,112,563)
        verify(window,"Set value")
        type_value("8")
        key("Return")
        verify(window,"Mate solved to 0")
        key("Control_L+s")
        verify(window,"Saved")
        fixed=json.loads(path.read_bytes())["items"][2]["data"]
        assert fixed["instances"][1]["transform"]["translation"]==dict(x=0,y=0,z=10)
        click(window,70,165)
        verify(window,"Selected Base")
        click(window,235,543)
        click(window,112,563)
        verify(window,"Set value")
        type_value("12")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        shifted=json.loads(path.read_bytes())["items"][2]["data"]
        assert shifted["mates"]==fixed["mates"] and shifted["actuators"]==fixed["actuators"]
        for before,after in zip(fixed["instances"],shifted["instances"],strict=True):
            original=before["transform"]["translation"]
            assert after["transform"]["translation"]==dict(original,x=original["x"]+12)
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"inspected-elbow.pcad"
        arm=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        arm["selectedPath"]=[2]
        source=arm["items"][2]["data"]
        source.pop("actuators")
        path.write_text(json.dumps(arm))
        app,window=open_app(path,"Assembly opened")
        key("t")
        click(window,72,221)
        verify(window,"Selected Forearm")
        click(window,75,277)
        verify(window,"Selected mate   Elbow")
        click(window,172,615)
        verify(window,"Set value")
        type_value("0")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        driven=json.loads(path.read_bytes())["items"][2]["data"]
        assert driven["actuators"]==[dict(id="servo-1",type="servo",mateId="mate-elbow",homeDeg=0,
                                          commandRange=dict(minDeg=-180,maxDeg=180))]
        key("F2")
        type_value("42")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        driven=json.loads(path.read_bytes())["items"][2]["data"]
        assert driven["actuators"][0]["homeDeg"]==42
        subprocess.run(["magick","import","-window",hex(window),output_dir/"assembly-mate-inspector.png"],check=True)
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened")
        key("t")
        click(window,72,221)
        verify(window,"Selected Forearm")
        click(window,75,277)
        verify(window,"Selected mate   Elbow")
        key("Delete")
        verify(window,"Removed mate   mate-elbow")
        key("Control_L+s")
        verify(window,"Saved")
        detached=json.loads(path.read_bytes())["items"][2]["data"]
        assert detached["actuators"]==[] and detached["mates"]==source["mates"][:1]
        assert detached["connectors"]==source["connectors"][:2]
        assert detached["instances"]==driven["instances"]
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][2]["data"]==driven
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][2]["data"]==detached
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"detached-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        key("t")
        for name,y in [("Upper arm",193),("Forearm",193),("Base",165)]:
            click(window,75,y)
            verify(window,"Selected "+name)
            key("Delete")
            verify(window,"Removed instance")
        key("Control_L+s")
        verify(window,"Saved")
        empty=json.loads(path.read_bytes())
        assert empty["items"][:2]==arm["items"][:2] and empty["items"][2]["data"]["instances"]==[]
        assert not empty["items"][2]["data"]["mates"] and not empty["items"][2]["data"]["actuators"]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened")
        key("i")
        verify(window,"Inserted Base")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"face-mate.pcad"
        pair=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        pair["selectedPath"]=[2]
        data=pair["items"][2]["data"]
        data["instances"]=[dict(id="left",name="Left plate",partId="part-base"),
                           dict(id="right",name="Right plate",partId="part-base",
                                transform=dict(translation=dict(x=100,y=0,z=0),rotation=dict(x=0,y=0,z=15)))]
        for field in ("connectors","mates","actuators"):
            data.pop(field,None)
        path.write_text(json.dumps(pair))
        app,window=open_app(path,"Assembly opened")
        key("3")
        verify(window,"Top view")
        from PIL import Image
        frame=output_dir/"mate-top-view.png"
        subprocess.run(["magick","import","-window",hex(window),frame],check=True)
        with Image.open(frame).convert("RGB") as pixels:
            # Pick well inside each top face where the two plates cross the centre row.
            y=pixels.height//2
            runs=[]
            for x in range(120,pixels.width-120):
                r,g,b=pixels.getpixel((x,y))
                if g>r+20 and r>50 and g>b+15:
                    if runs and runs[-1][-1]==x-1:
                        runs[-1].append(x)
                    else:
                        runs.append([x])
            left,right=[row[len(row)//2] for row in runs if len(row)>10]
        click(window,right,y)
        verify(window,"Selected Right plate")
        key("g")
        verify(window,"Fastened to world")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"]==pair["items"]
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        grounded=json.loads(path.read_bytes())["items"][2]["data"]
        world,local=grounded["connectors"]
        assert world["instanceId"] is None and local["instanceId"]=="right"
        assert world["position"]==dict(x=100,y=0,z=0) and abs(world["rotation"]["z"]-15)<.001
        assert grounded["mates"][0]["type"]=="fasten"
        key("Control_L+z")
        click(window,780,55)
        verify(window,"Pick a vertex, edge or face on the first instance")
        click(window,left,y)
        verify(window,"Pick on the second instance")
        click(window,left,y)
        verify(window,"different instance")
        click(window,right,y)
        verify(window,"Choose a mate type")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"assembly-mate-choices.png"],check=True)
        key("Escape")
        verify(window,"Mate cancelled")
        for digit,kind in [("2","revolute"),("3","prismatic"),("4","planar"),("5","ball")]:
            key("3")
            verify(window,"Top view")
            click(window,780,55)
            verify(window,"Pick a vertex, edge or face on the first instance")
            click(window,left,y)
            verify(window,"Pick on the second instance")
            key("6")
            verify(window,"Bottom view")
            click(window,right,y)
            verify(window,"Choose a mate type")
            key(digit)
            verify(window,"Created mate   "+kind)
            key("Control_L+s")
            verify(window,"Saved")
            made=json.loads(path.read_bytes())["items"][2]["data"]
            assert len(made["connectors"])==2 and [mate["type"] for mate in made["mates"]]==[kind]
            native=Path(tmp)/(kind+"-native.3mf")
            rust=Path(tmp)/(kind+"-rust.3mf")
            subprocess.run([output_dir/"puppycad","--export",path,native],check=True)
            subprocess.run([root/"rust/target/debug/puppycad","--export",path,rust],check=True)
            compare_solid_packages(native,rust)
            key("Control_L+z")
        key("3")
        verify(window,"Top view")
        click(window,left,y)
        verify(window,"Selected Left plate")
        xlib.XWarpPointer(connection,0,window,0,0,0,0,right,y)
        xlib.XSync(connection,0)
        key("j")
        verify(window,"Choose a mate type")
        key("1")
        verify(window,"Created mate   fasten")
        key("Control_L+s")
        verify(window,"Saved")
        made=json.loads(path.read_bytes())["items"][2]["data"]
        assert [c["instanceId"] for c in made["connectors"]]==["left","right"]
        assert made["mates"]==[dict(id="mate-1",type="fasten",
                                   a=dict(instanceId="left",connectorId="connector-1"),
                                   b=dict(instanceId="right",connectorId="connector-2"))]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"mate-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        key("Escape")
        app.wait(3)

        exchange=Path(tmp)/"sensor.step"
        subprocess.run([output_dir/"puppycad","--export",root/"tests/fixtures/sensor-unit.pcad",exchange],check=True)
        original_step=exchange.read_bytes()
        path=exchange.with_suffix(".pcad")
        app,window=open_app(exchange,"Add   Enclosure base")
        key("Right")
        verify(window,"Add   Enclosure lid")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"step-body-preview.png"],check=True)
        key("Return")
        verify(window,"STEP body imported")
        key("Control_L+z")
        verify(window,"Sketch 1")
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        original=json.loads(path.read_bytes())
        lid=original["nodes"][0]
        assert lid["import"]["name"]=="Enclosure lid" and len(lid["triangles"])==72
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"PCAD design opened")
        from PIL import Image
        def import_view():
            frame=Path(tmp)/"import-view.png"
            subprocess.run(["magick","import","-window",hex(window),frame],check=True)
            with Image.open(frame) as image:
                return image.convert("RGB").crop((0,76,840,640)).tobytes()
        before_import=import_view()
        key("Control_L+i")
        verify(window,"Enter a path")
        type_value("sensor.step")
        key("Return")
        verify(window,"Add   Enclosure base")
        key("t")
        verify(window,"Replace "+lid["id"])
        before_orbit=import_view()
        drag(window,3,(475,320),(585,375))
        assert import_view()!=before_orbit
        key("Escape")
        verify(window,"STEP import cancelled")
        assert json.loads(path.read_bytes())==original and import_view()==before_import
        click(window,756,20)
        verify(window,"Enter a path")
        type_value("sensor.step")
        key("Return")
        key("t")
        key("Return")
        verify(window,"STEP body imported")
        key("Control_L+s")
        verify(window,"Saved")
        replaced=json.loads(path.read_bytes())
        assert replaced["parts"]==original["parts"] and len(replaced["nodes"])==1
        assert replaced["nodes"][0]["id"]==lid["id"] and replaced["nodes"][0]["import"]["name"]=="Enclosure base"
        key("Control_L+e")
        verify(window,"Exported")
        assert triangulation(path.with_suffix(".stl"))[1]>0 and exchange.read_bytes()==original_step
        key("Escape")
        app.wait(3)

        complete=Path(tmp)/"complete.step"
        complete.write_bytes(original_step)
        app,window=open_app(complete,"Add   Enclosure base")
        key("a")
        verify(window,"Import assembly")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"step-assembly-preview.png"],check=True)
        key("Return")
        verify(window,"STEP assembly imported")
        key("Control_L+z")
        verify(window,"Sketch 1")
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        whole=complete.with_suffix(".pcad")
        drawing=json.loads(whole.read_bytes())
        uses=drawing["assembly"]["instances"]
        assert len(uses)==6 and len(drawing["parts"])==5 and uses[4]["part"]==uses[5]["part"]
        assert drawing["assembly"]["mates"][1]["parent_frame"]==dict(position=[0,0,13],rotation=[45,0,0])
        key("Escape")
        app.wait(3)
        app,window=open_app(whole,"PCAD design opened")
        key("Control_L+e")
        verify(window,"Exported")
        exports=sorted(whole.parent.glob("complete-*.stl"))
        assert len(exports)==6 and all(triangulation(part)[1]>0 for part in exports)
        assert complete.read_bytes()==original_step
        key("Escape")
        app.wait(3)

        # Keep both the model bytes and existing exports intact while inspecting a
        # new record, even after exercising ordinary edit shortcuts and gestures.
        path = Path(tmp)/"Unsupported feature.pcad"
        drawing = json.loads((root/"rust/fixtures/extruded-rectangle.pcad").read_bytes())
        graph = drawing["items"][0]["data"]["cad"]
        graph["nodes"].append(dict(id="future-7",type="futureBlend",payload="精密 — untouched"))
        graph["rootNodeIds"].append("future-7")
        original = (json.dumps(drawing,ensure_ascii=False,indent=3)+"\r\n").encode()
        path.write_bytes(original)
        sentinel = path.with_suffix(".3mf")
        sentinel.write_bytes(b"previous export")
        for attempt in range(2):
            app, window = open_app(path,"Read-only  |  Geometry preview")
            verify(window,"Unsupported CAD record: futureBlend  (future-7)")
            capture(window,"read-only-preview.png",False)
            key("Right")
            click(window,150,55)
            key("F2")
            key("plus")
            key("Control_L+s")
            key("Control_L+Alt_L+e")
            click(window,24,53)
            verify(window,"Read-only  |  File source")
            key("End")
            subprocess.run(["magick","import","-window",hex(window),output_dir/"read-only-source.png"],check=True)
            key("Control_L+z")
            key("F7")
            click(window,420,260)
            key("Home")
            key("Tab")
            verify(window,"Read-only  |  Geometry preview")
            key("Escape")
            app.wait(3)
            assert path.read_bytes()==original and sentinel.read_bytes()==b"previous export"
            assert list(path.parent.glob("Unsupported feature*.3mf"))==[sentinel]

        path=Path(tmp)/"Constrained line.pcad"
        original=(root/"rust/fixtures/graph-constrained-line.pcad").read_bytes()
        path.write_bytes(original)
        app,window=open_app(path,"Click two rectangle corners")
        click(window,665,55)
        verify(window,"100 mm   DOF 2")
        click(window,57,55)
        verify(window,"Set value")
        type_value("125.125")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"100 mm   DOF 2")
        key("Control_L+y")
        verify(window,"125.125 mm   DOF 2")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        key("Escape")
        app.wait(3)
        saved=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        line=next(n for n in saved if n["type"]=="sketchLine")
        assert line["p0"]=={"x":-50,"y":20} and line["p1"]=={"x":75.125,"y":20}
        assert [(r["id"],r["constraint"]["type"]) for r in saved if r["type"]=="sketchConstraint"]==[
            ("horizontal-1","horizontal"),("length-1","lineLength")]
        assert next(n for n in saved if n["id"]=="length-1")["constraint"]["value"]==125.125
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"125.125 mm   DOF 2")
        from PIL import Image
        selected=output_dir/"constrained-line.png"
        subprocess.run(["magick","import","-window",hex(window),selected],check=True)
        with Image.open(selected) as shot:
            pixels=shot.convert("RGB")
            ink=[(x,y) for y in range(80,pixels.height) for x in range(pixels.width)
                 if pixels.getpixel((x,y))==(255,176,69)]
        assert len(ink)>500
        start=(max(x for x,y in ink)-3,(max(y for x,y in ink)+min(y for x,y in ink))//2)
        end=(start[0]-60,start[1]-40)
        drag(window,1,start,end)
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]==saved
        key("Escape")
        key("Escape")
        app.wait(3)

        # Remove the driving dimension. The endpoint can now move along X.
        free=json.loads(path.read_bytes())
        free_nodes=free["items"][0]["data"]["cad"]["nodes"]
        free_nodes.remove(next(n for n in free_nodes if n["id"]=="length-1"))
        path=Path(tmp)/"Horizontal line.pcad"
        path.write_text(json.dumps(free))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"125.125 mm   DOF 3")
        drag(window,1,start,end,cancel=True,dragging="Dragging endpoint")
        verify(window,"Drag cancelled")
        drag(window,1,start,end)
        key("Control_L+s")
        verify(window,"Saved")
        after=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        line=next(n for n in after if n["type"]=="sketchLine")
        assert line["p0"]==dict(x=-50,y=20) and line["p1"]["y"]==20
        assert -50<line["p1"]["x"]<75.125
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]==free_nodes
        key("Escape")
        key("Escape")
        app.wait(3)

        libs=Path(os.environ.get("FLEXSCRIPT_LIB",root.parent/"flexscript/libs"))
        path=Path(tmp)/"Constrained plate.pcad"
        path.write_bytes((libs/"cad/tests/fixtures/constrained-plate.pcad").read_bytes())
        app,window=open_app(path)
        click(window,696,55)
        verify(window,"bottom   20 mm   DOF 0")
        click(window,480,55)
        verify(window,"New variable")
        type_value("w = 1 in")
        key("Return")
        verify(window,"Variables updated")
        key("Control_L+s")
        verify(window,"Saved")
        var_sketch=next(n for n in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"] if n["type"]=="sketch")
        assert var_sketch["variables"]==[dict(name="w",kind="length",expression="1 in")], var_sketch["variables"]
        key("F2")
        type_value("#w + 6.725 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        package=path.with_suffix(".3mf")
        reference=Path(tmp)/"reference-constrained-plate.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(package,reference)
        assert math.isclose(package_geometry(package)[1][0][1][1],1606.25,abs_tol=.001)
        click(window,696,55)
        verify(window,"bottom   32.125 mm   DOF 0")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"constrained-plate.png"],check=True)
        key("F2")
        verify(window,"Set value")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"line-expression.png"],check=True)
        key("End")
        type_value(" + 1 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"32.125 mm")
        key("F3")
        verify(window,"Clear to delete")
        type_value("w = 2 cm")
        key("Return")
        verify(window,"Variables updated")
        key("Control_L+s")
        verify(window,"Saved")
        saved_plate=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        sketch=next(n for n in saved_plate if n["type"]=="sketch")
        assert sketch["variables"]==[dict(name="w",kind="length",expression="2 cm")]
        dimension=next(r for r in sketch["relations"] if r["id"]=="width")
        assert dimension["expression"]=="#w + 6.725 mm" and dimension["value"]==26.725
        assert next(n for n in saved_plate if n["id"]=="top")["p0"]["x"]==26.725
        key("Escape")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Placed points.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        click(window,576,20)
        click(window,589,55)  # Point
        verify(window,"Click to place a sketch point")
        click(window,410,310)
        verify(window,"Point added")
        click(window,647,55)  # Datum
        verify(window,"Click to place a construction point")
        click(window,550,400)
        key("Control_L+z")
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        def placed_graph():
            return json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        nodes=placed_graph()
        dots=[n["primitive"] for n in nodes if n["type"]=="sketchPrimitive"]
        assert len(dots)==2 and all(p["type"]=="point" for p in dots)
        assert not dots[0].get("construction") and dots[1]["construction"]
        assert dots[0]["id"]!=dots[1]["id"] and dots[0]["center"]["x"]<dots[1]["center"]["x"]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"New distance to measure sketch points")
        shot=output_dir/"placed-points.png"
        subprocess.run(["magick","import","-window",hex(window),shot],check=True)
        with Image.open(shot) as image:
            pixels=image.convert("RGB")
            markers=[(x,y) for y in range(76,pixels.height) for x in range(pixels.width)
                     if pixels.getpixel((x,y))==(143,217,184)]
            handles=[]
            for left,right in [(0,pixels.width//2),(pixels.width//2,pixels.width)]:
                marker=[(x,y) for x,y in markers if left<=x<right]
                assert 20<=len(marker)<=100
                handles.append(tuple(round(sum(axis)/len(axis)) for axis in zip(*marker)))
            assert pixels.getpixel(handles[0])==(143,217,184)
            assert pixels.getpixel(handles[1])!=(143,217,184)
        start=handles[0];end=(start[0]+52,start[1]+50)
        drag(window,1,start,end,cancel=True,dragging="Dragging sketch point")
        key("Control_L+s")
        assert placed_graph()==nodes
        drag(window,1,start,end)
        key("Control_L+s")
        moved=placed_graph()
        assert moved[-2]["primitive"]["center"]!=dots[0]["center"] and moved[-1]==nodes[-1]
        key("Control_L+z")
        key("n")
        key("x")
        for handle in handles:
            click(window,*handle)
        verify(window,"native-distance-1   X distance")
        key("F2")
        type_value("22 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        graph=placed_graph()
        placed=[n["primitive"] for n in graph if n["type"]=="sketchPrimitive"]
        dimension=next(n for n in graph if n["type"]=="sketch")["relations"][0]
        assert dimension["a"]==dict(entityId=dots[0]["id"],point="center")
        assert dimension["b"]==dict(entityId=dots[1]["id"],point="center")
        assert placed[0]==dots[0] and placed[1]["center"]["x"]==dots[0]["center"]["x"]+22
        key("Escape")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Point dimensions.pcad"
        path.write_bytes((root/"rust/fixtures/dimensions.pcad").read_bytes())
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"50.8 mm   DOF 3")
        key("d")
        verify(window,"Reference Distance   31.622777 mm")
        key("F2")
        verify(window,"Reference distance is measured")
        key("d")
        verify(window,"X distance   -40.8 mm")
        key("F2")
        type_value("-32 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"X distance   -40.8 mm")
        key("Control_L+y")
        verify(window,"X distance   -32 mm")
        key("r")
        verify(window,"Reference X distance   -32 mm   DOF 4")
        key("r")
        verify(window,"X distance   -32 mm   DOF 3")
        key("F3")
        verify(window,"Clear to delete")
        type_value("w = 2 cm")
        key("Return")
        verify(window,"Variables updated")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        key("Escape")
        app.wait(3)
        items=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        dot=next(n for n in items if n["id"]=="point")["primitive"]
        assert dot["center"]==dict(x=8,y=24.6)
        rels={r["id"]:r for n in items if n["type"]=="sketch" for r in n["relations"]}
        assert rels["len"]["labelPosition"]==dict(x=25,y=-12) and rels["oy"]["expression"]=="#w + 4.6 mm"
        assert abs(rels["dist"]["value"]-math.hypot(8,24.6))<.000001
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        key("d")
        verify(window,"Reference Distance")
        key("d")
        key("d")
        verify(window,"Y distance   24.6 mm   DOF 3")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"point-distance.png"],check=True)
        with Image.open(output_dir/"point-distance.png") as shot:
            pixels=shot.convert("RGB")
            upper=[(x,y) for y in range(80,pixels.height//2) for x in range(pixels.width)
                   if pixels.getpixel((x,y))==(255,176,69)]
            lower=[(x,y) for y in range(pixels.height//2,pixels.height) for x in range(pixels.width)
                   if pixels.getpixel((x,y))==(143,217,184)]
        assert upper and lower
        end_x,end_y=max(lower)
        point_x,point_y=max(upper)
        key("n")
        verify(window,"Pick first anchor")
        key("y")
        key("r")
        click(window,end_x,end_y)
        verify(window,"Y reference distance   Pick second anchor")
        click(window,point_x-2,point_y-2)
        verify(window,"native-distance-1   Reference Y distance   24.6 mm")
        key("Control_L+s")
        verify(window,"Saved")
        items=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        relation=next(r for n in items if n["type"]=="sketch" for r in n["relations"] if r["id"]=="native-distance-1")
        assert relation["a"]==dict(entityId="line",point="p1") and relation["b"]==dict(entityId="point",point="center")
        key("Delete")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]==items
        key("Escape")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Unsupported line relation.pcad"
        source=json.loads(original)
        source["items"][0]["data"]["cad"]["nodes"][-2]["constraint"]["type"]="futureLineAxis"
        original=json.dumps(source).encode()
        path.write_bytes(original)
        app,window=open_app(path,"Read-only  |  Saved sketch")
        verify(window,"Sketch relation needs solver support: futureLineAxis")
        capture(window,"read-only-sketch.png",True)
        key("l")
        click(window,320,300)
        click(window,460,420)
        key("Control_L+s")
        key("Tab")
        verify(window,"File source")
        key("Tab")
        verify(window,"Saved sketch")
        key("Escape")
        app.wait(3)
        assert path.read_bytes()==original

        path = Path(tmp)/"Other part.pcad"
        drawing = json.loads((root/"rust/fixtures/extruded-rectangle.pcad").read_bytes())
        drawing["extensionMetadata"] = {"type":"futureBlend","payload":[1,2,3]}
        other = json.loads(json.dumps(drawing["items"][0]))
        other.update(id="hidden-part",name="Other part")
        drawing["items"].append(other)
        other["data"]["tree"]["rollbackIndex"] = 0
        unsupported = dict(id="hidden-1",type="futureBlend",suppressed=True)
        other["data"]["cad"]["nodes"].append(unsupported)
        original = json.dumps(drawing,indent=2).encode()
        path.write_bytes(original)
        app,window = open_app(path,"Read-only  |  Geometry preview")
        verify(window,"Unsupported CAD record: futureBlend  (hidden-1)")
        key("F6")
        key("Shift_L+n")
        key("Control_L+s")
        key("Escape")
        app.wait(3)
        assert path.read_bytes()==original
        other["data"]["cad"]["nodes"].remove(unsupported)
        path.write_text(json.dumps(drawing))
        app,window = open_app(path)
        key("equal")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)
        saved=json.loads(path.read_bytes())
        assert saved["extensionMetadata"]==drawing["extensionMetadata"]
        assert saved["items"][1]==other
        assert saved["revision"]==drawing.get("revision",0)+1

        path = Path(tmp)/"Future standalone.pcad"
        future = dict(format="puppycorp.pcad",schema_version=2,units="mm",nodes=[],futureData=[f"entry-{i} "+("x"*(i%23)) for i in range(250)])
        original = json.dumps(future,separators=(",",":")).encode()
        path.write_bytes(original)
        app,window = open_app(path,"Read-only  |  File source")
        from PIL import Image
        def source_pixels():
            frame=Path(tmp)/"source-page.png"
            subprocess.run(["magick","import","-window",hex(window),frame],check=True)
            with Image.open(frame) as shot:
                return shot.convert("RGB").crop((0,76,shot.width,shot.height)).tobytes()
        first_page=source_pixels()
        key("Next")
        assert source_pixels()!=first_page
        key("Home")
        assert source_pixels()==first_page
        key("Control_L+s")
        key("Tab")
        verify(window,"Unsupported standalone schema")
        key("Escape")
        app.wait(3)
        assert path.read_bytes()==original and not path.with_suffix(".stl").exists()

        path = Path(tmp)/'Exact dimensions.pcad'
        original = (root/'rust/fixtures/extruded-rectangle.pcad').read_bytes()
        path.write_bytes(original)
        app, window = open_app(path)
        key('Tab')
        verify(window,'width 160 mm')
        click(window,130,20)
        verify(window,'Set value')
        key('Control_L+c')
        type_value('1.0000007')
        key('Return')
        verify(window,'up to six places')
        key('Control_L+a')
        key('Control_L+v')
        key('Return')
        verify(window,'Value unchanged')
        key('F2')
        verify(window,'Set value')
        type_value('-5')
        key('Return')
        verify(window,'positive length')
        key('Control_L+a')
        type_value('162.125')
        subprocess.run(['magick','import','-window',hex(window),output_dir/'exact-width.png'],check=True)
        click(window,838,55)
        verify(window,'width 162.125 mm')
        key('Control_L+z')
        verify(window,'width 160 mm')
        key('Control_L+y')
        verify(window,'width 162.125 mm')
        key('Tab')
        key('Tab')
        verify(window,'depth 60 mm')
        key('F2')
        verify(window,'Set value')
        type_value('100')
        key('Escape')
        verify(window,'Edit cancelled')
        assert path.read_bytes()==original
        key('F2')
        type_value('60.625')
        key('Return')
        verify(window,'depth 60.625 mm')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        nodes=json.loads(path.read_bytes())['items'][0]['data']['cad']['nodes']
        assert next(n for n in nodes if n['type']=='sketch')['relations'][0]['value']==162.125
        app,window=open_app(path)
        verify(window,'depth 60.625 mm')
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        actual=package_geometry(path.with_suffix('.3mf'))[1]
        ref=Path(tmp)/'width-reference.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        assert actual==package_geometry(ref)[1]
        assert actual[0][1][0]==[(-81.0625,81.0625),(-50,50),(0,60.625)]
        key('Escape')
        app.wait(3)

        path = Path(tmp)/"History.pcad"
        path.write_bytes((root/"rust/fixtures/feature-tree.pcad").read_bytes())
        app, window = open_app(path)
        click(window, 600, 55)
        verify(window, "1 / 3   Extrusion 1   active")
        click(window, 75, 124)
        verify(window, "Extrusion 3   suppressed")
        key("s")
        verify(window, "Extrusion 3   active")
        key("Control_L+Alt_L+e")
        verify(window, "Exported")
        before_move = path.with_suffix(".3mf").rename(Path(tmp)/"before-move.3mf")
        assert math.isclose(package_geometry(before_move)[1][0][1][1],527500,abs_tol=.01)
        key("Up")
        verify(window, "different body")
        key("Down")
        verify(window, "3 / 3   Extrusion 3   active")
        key("Control_L+s")
        verify(window, "Saved")
        moved = json.loads(path.read_bytes())["items"][0]["data"]
        order = ["native-extrude-1", "native-extrude-2", "native-extrude-3"]
        assert [n["id"] for n in moved["cad"]["nodes"] if n["type"]=="extrude"] == order
        for sequence in (moved["cad"]["rootNodeIds"], moved["tree"]["orderedNodeIds"]):
            assert [id for id in sequence if id in order] == order
        key("Control_L+Alt_L+e")
        verify(window, "Exported")
        ref = Path(tmp)/"reordered.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        after_move = path.with_suffix(".3mf").rename(Path(tmp)/"native-reordered.3mf")
        compare_solid_packages(after_move,ref)
        assert math.isclose(package_geometry(after_move)[1][0][1][1],511500,abs_tol=.01)
        key("Up")
        verify(window, "2 / 3   Extrusion 3   active")
        key("Left")
        key("s")
        verify(window, "Extrusion 1   suppressed")
        key("Right")
        verify(window, "Extrusion 3   error   Boolean extrusion needs an active target body")
        key("Control_L+z")
        verify(window, "Extrusion 3   active")
        key("Control_L+y")
        verify(window, "Extrusion 3   error")
        key("Left")
        key("s")
        verify(window, "Extrusion 1   active")
        key("b")
        verify(window, "Extrusion 1   rolled back")
        key("Control_L+s")
        verify(window, "Saved")
        key("Escape")
        key("Escape")
        app.wait(3)
        assert json.loads(path.read_bytes())["items"][0]["data"]["tree"]["rollbackIndex"] == 0
        app, window = open_app(path)
        key("F7")
        verify(window, "Extrusion 1   rolled back")
        key("Right")
        key("a")
        verify(window, "Extrusion 3   active")
        key("Right")
        verify(window, "Extrusion 2   rolled back")
        capture(window, "feature-history.png", False)
        key("Control_L+s")
        verify(window, "Saved")
        key("Control_L+Alt_L+e")
        verify(window, "Exported")
        reference = Path(tmp)/"reference-history.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf").rename(Path(tmp)/"native-rollback.3mf"), reference)
        key("e")
        verify(window, "Extrusion 2   active")
        key("s")
        verify(window, "Extrusion 2   suppressed")
        key("Control_L+s")
        verify(window, "Saved")
        key("Escape")
        key("Escape")
        app.wait(3)
        data = json.loads(path.read_bytes())["items"][0]["data"]
        assert "rollbackIndex" not in data["tree"]
        assert next(n for n in data["cad"]["nodes"] if n["id"]=="native-extrude-2")["suppressed"]
        app, window = open_app(path)
        key("Control_L+Alt_L+e")
        verify(window, "Exported")
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"), reference)
        key("Escape")
        app.wait(3)

        damaged = Path(tmp)/"Failed feature.pcad"
        drawing = json.loads((root/"rust/fixtures/feature-tree.pcad").read_bytes())
        cut = next(
            n for n in drawing["items"][0]["data"]["cad"]["nodes"] if n.get("operation")=="cut")
        del cut["suppressed"]
        cut["depth"] = 0
        damaged.write_text(json.dumps(drawing))
        app, window = open_app(damaged, "Extrusion 3 failed")
        key("F7")
        click(window, 75, 124)
        verify(window, "Extrusion 3   error   Extrusion needs a positive depth")
        capture(window, "feature-error.png", False)
        key("Control_L+Alt_L+e")
        verify(window, "Extrusion needs a positive depth")
        assert not damaged.with_suffix(".3mf").exists()
        key("s")
        verify(window, "Extrusion 3   suppressed")
        key("Control_L+s")
        verify(window, "Saved")
        key("Escape")
        key("Escape")
        app.wait(3)
        app, window = open_app(damaged)
        key("Control_L+Alt_L+e")
        verify(window, "Exported")
        subprocess.run([root/"rust/target/debug/puppycad","--export",damaged,reference],check=True)
        compare_solid_packages(damaged.with_suffix(".3mf"),reference)
        key("Escape")
        app.wait(3)

        for kind in ('Chamfer','Fillet'):
            from PIL import Image
            path=Path(tmp)/(kind+' edges.pcad')
            path.write_text(json.dumps(edge_document()))
            app,window=open_app(path)
            key('3')
            verify(window,'Top view')
            frame=Path(tmp)/'edge-pick-top.png'
            subprocess.run(['magick','import','-window',hex(window),frame],check=True)
            with Image.open(frame) as shot:
                pixels=shot.convert('RGB')
                cx,cy=pixels.width//2,pixels.height//2
                surface=pixels.getpixel((cx,cy))
                assert surface[1]>surface[0] and surface[0]>55,surface
                ys=[y for y in range(78,pixels.height) if pixels.getpixel((cx,y))==surface]
                xs=[x for x in range(pixels.width) if pixels.getpixel((x,cy))==surface]
            edge_y=max(ys)-1
            click(window,580,21)
            verify(window,'Pick straight edges')
            click(window,145,55)
            verify(window,'Fillet')
            key('Escape')
            verify(window,'Edge finish cancelled')
            key('Shift_L+h' if kind=='Fillet' else 'h')
            verify(window,'Pick straight edges')
            click(window,25,92)
            verify(window,'closer to a visible edge')
            click(window,cx,edge_y)
            verify(window,'Selected edges 1')
            click(window,cx,edge_y)
            verify(window,'Selected edges 0')
            click(window,cx,edge_y)
            verify(window,'Selected edges 1')
            click(window,max(xs)-1,cy)
            verify(window,'Selected edges 2')
            selected=output_dir/'selected-finish-edges.png'
            for _ in range(40):
                subprocess.run(['magick','import','-window',hex(window),selected],check=True)
                with Image.open(selected) as shot:
                    colors=shot.convert('RGB').crop((min(xs)-2,min(ys)-2,max(xs)+3,max(ys)+3))
                    if sum(
                        count for count,rgb in colors.getcolors(colors.width*colors.height) if rgb==(255,176,68)
                    )>200: break
                time.sleep(.05)
            else: raise AssertionError('Picked edges did not highlight')
            click(window,296,55)
            verify(window,kind+' created')
            field='radius' if kind=='Fillet' else 'distance'
            verify(window,kind+' 1  '+field+' 1 mm')
            key('F2')
            type_value('2.125')
            key('Return')
            verify(window,field+' 2.125 mm')
            key('Control_L+z')
            verify(window,field+' 1 mm')
            key('Control_L+y')
            verify(window,field+' 2.125 mm')
            key('Control_L+s')
            verify(window,'Saved')
            key('Escape')
            app.wait(3)
            saved=json.loads(path.read_bytes())['items'][0]['data']
            finish=saved['cad']['nodes'][-1]
            assert finish['type']=='edgeFinish' and finish['kind']==kind.lower() and finish[field]==2.125
            assert saved['tree']['orderedNodeIds'][-1]==finish['id'] and len(finish['edges'])==2
            app,window=open_app(path)
            key('Control_L+Alt_L+e')
            verify(window,'Exported')
            reference=Path(tmp)/'reference-finish.3mf'
            subprocess.run([root/'rust/target/debug/puppycad','--export',path,reference],check=True)
            compare_solid_packages(path.with_suffix('.3mf'),reference)
            if kind=='Fillet':
                subprocess.run(['magick','import','-window',hex(window),output_dir/'finished-fillet.png'],check=True)
            key('Escape')
            app.wait(3)

        path=Path(tmp)/'Two-section loft.pcad'
        drawing=loft_document(with_loft=False)
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,'Click two rectangle corners')
        click(window,490,20)
        verify(window,'First loft profile 1')
        key('Escape')
        verify(window,'Loft cancelled')
        key('9')
        click(window,65,55)
        verify(window,'Sketch 1  |  First loft profile 1')
        click(window,300,55)
        verify(window,'Sketch 3  |  Second loft profile 1')
        click(window,65,55)
        click(window,300,55)
        verify(window,'different sketch')
        key('Tab')
        verify(window,'Sketch 3  |  Second loft profile 1')
        subprocess.run(['magick','import','-window',hex(window),output_dir/'loft-profiles.png'],check=True)
        click(window,300,55)
        verify(window,'Loft created')
        key('Control_L+z')
        verify(window,'Sketch 3')
        key('Control_L+y')
        verify(window,'|  Solid')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        expected=loft_document()
        expected['revision']=drawing.get('revision',0)+1
        actual=json.loads(path.read_bytes())
        # The author's new node is appended after the existing geometry records.
        graph=expected['items'][0]['data']['cad']
        node=next(n for n in graph['nodes'] if n['type']=='loft')
        graph['nodes'].remove(node)
        graph['nodes'].append(node)
        assert actual==expected
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        reference=Path(tmp)/'reference-loft.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,reference],check=True)
        compare_solid_packages(path.with_suffix('.3mf'),reference)
        # Driving the base depth moves the second section's supporting plane.
        initial_volume=package_geometry(reference)[1][1][1][1]
        key('F2')
        verify(window,'Set value')
        type_value('28.75')
        key('Return')
        verify(window,'depth 28.75 mm')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        app,window=open_app(path)
        verify(window,'depth 28.75 mm')
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,reference],check=True)
        # Interactive export preserves the earlier package.
        updated=path.with_name(path.stem+'-2.3mf')
        _,prior_bodies=package_geometry(path.with_suffix('.3mf'))
        assert prior_bodies[1][1][0][2]==(0,20)
        compare_solid_packages(updated,reference)
        _,(bounds,volume)=package_geometry(updated)[1][1]
        assert bounds[2]==(0,28.75) and math.isclose(volume,initial_volume*28.75/20,rel_tol=.00001)
        key('t')
        focus_search(window,75)
        for letter in 'loft':
            key(letter)
        click(window,92,163)
        verify(window,'Selected Loft 1')
        click(window,58,615)
        verify(window,'Isolated Loft 1')
        subprocess.run(['magick','import','-window',hex(window),output_dir/'loft-solid.png'],check=True)
        key('Escape')
        app.wait(3)

        path=Path(tmp)/'Joined loft.pcad'
        drawing=loft_document(with_loft=False)
        graph=drawing['items'][0]['data']['cad']
        circle=next(n for n in graph['nodes'] if n['type']=='sketchPrimitive')['primitive']
        circle.update(radius=14,segments=8)
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,'Click two rectangle corners')
        key('9')
        key('Tab')
        key('Return')
        verify(window,'Sketch 3  |  Second loft profile 1')
        click(window,420,55)
        verify(window,'Loft created')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        loft=next(n for n in json.loads(path.read_bytes())['items'][0]['data']['cad']['nodes'] if n['type']=='loft')
        assert loft['operation']=='join'
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        reference=Path(tmp)/'reference-joined-loft.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,reference],check=True)
        compare_solid_packages(path.with_suffix('.3mf'),reference)
        assert len(package_geometry(reference)[1])==1
        key('Escape')
        app.wait(3)

        path=Path(tmp)/'Authored revolve.pcad'
        app,window=open_app(path,'Click two rectangle corners')
        click(window,520,330)
        click(window,580,365)
        verify(window,'Sketch edited')
        click(window,45,21)
        verify(window,'choose X or Y')
        click(window,550,280)
        click(window,550,390)
        verify(window,'crosses the revolve axis')
        key('Escape')
        verify(window,'Revolve cancelled')
        key('8')
        click(window,490,398)
        verify(window,'Click the axis endpoint')
        click(window,470,318)
        verify(window,'Revolved')
        verify(window,'angle 360 deg')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        authored=json.loads(path.read_bytes())['items'][0]['data']
        feature=next(n for n in authored['cad']['nodes'] if n['type']=='revolve')
        assert feature['id'] in authored['cad']['rootNodeIds'] and authored['tree']['orderedNodeIds'][-1]==feature['id']
        assert feature['axis']=={'a':{'x':2.5,'y':-10},'b':{'x':-2.5,'y':10}}
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        ref=Path(tmp)/'reference-authored-revolve.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        compare_solid_packages(path.with_suffix('.3mf'),ref)
        key('Escape')
        app.wait(3)

        path=Path(tmp)/'Revolve angle.pcad'
        drawing=revolve_document()
        path.write_text(json.dumps(drawing))
        app,window=open_app(path)
        for _ in range(3):
            key('Tab')
        verify(window,'Revolve 1  angle 360 deg')
        key('F2')
        type_value('361')
        key('Return')
        verify(window,'at most 360 degrees')
        key('Control_L+a')
        type_value('95.125')
        key('Return')
        verify(window,'angle 95.125 deg')
        key('Control_L+z')
        verify(window,'angle 360 deg')
        key('Control_L+y')
        verify(window,'angle 95.125 deg')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        next(n for n in drawing['items'][0]['data']['cad']['nodes'] if n['type']=='revolve')['angle']=95.125
        drawing['revision']=drawing.get('revision',0)+1
        assert json.loads(path.read_bytes())==drawing
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        subprocess.run(['magick','import','-window',hex(window),output_dir/'revolve-angle.png'],check=True)
        ref=Path(tmp)/'reference-revolve.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        compare_solid_packages(path.with_suffix('.3mf'),ref)
        key('Escape')
        app.wait(3)

        # Choose the second active extrusion by its sketch dimension. The
        # intervening suppressed feature must not shift the command target.
        path=Path(tmp)/'Feature fields.pcad'
        original=json.loads((root/'rust/fixtures/feature-tree.pcad').read_bytes())
        path.write_text(json.dumps(original))
        app,window=open_app(path)
        key('t')
        focus_search(window,75)
        for letter in 'height':
            key(letter)
        click(window,90,193)
        verify(window,'Extrusion 2  height 50 mm')
        key('F2')
        verify(window,'Set value')
        type_value('51.75')
        key('Home')
        key('Right')
        key('Delete')
        key('2')
        key('Return')
        verify(window,'height 52.75 mm')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        saved=json.loads(path.read_bytes())
        before={n['id']:n for n in original['items'][0]['data']['cad']['nodes']}
        after={n['id']:n for n in saved['items'][0]['data']['cad']['nodes']}
        assert {key for key in before if before[key]!=after[key]}=={'native-sketch-2','native-entity-1-2'}
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        ref=Path(tmp)/'height-reference.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        assert package_geometry(path.with_suffix('.3mf'))==package_geometry(ref)
        key('Escape')
        app.wait(3)

        path=Path(tmp)/'Exact hinge.pcad'
        original=json.loads((root/'tests/fixtures/sensor-unit.pcad').read_bytes())
        path.write_text(json.dumps(original))
        app,window=open_app(path,'PCAD design opened')
        key('F2')
        verify(window,'Set value')
        type_value('91')
        key('Return')
        verify(window,'travel limits')
        key('Control_L+a')
        type_value('41.375')
        key('Return')
        verify(window,'41.375 deg')
        key('Control_L+z')
        verify(window,'45 deg')
        key('Control_L+y')
        verify(window,'41.375 deg')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        original['assembly']['mates'][1]['value']=41.375
        assert json.loads(path.read_bytes())==original
        app,window=open_app(path,'PCAD design opened')
        verify(window,'41.375 deg')
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        assert len(package_geometry(path.with_suffix('.3mf'))[1])==6
        key('Escape')
        app.wait(3)

        path=Path(tmp)/'Exact elbow.pcad'
        arm=json.loads((root/'rust/fixtures/assembly.pcad').read_bytes())
        arm['selectedPath']=[2]
        path.write_text(json.dumps(arm))
        app,window=open_app(path,'Assembly opened')
        key('F2')
        verify(window,'Set value')
        type_value('-15.125')
        key('Return')
        verify(window,'-15.125 deg')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        assert json.loads(path.read_bytes())['items'][2]['data']['actuators'][0]['homeDeg']==-15.125
        app,window=open_app(path,'Assembly opened')
        verify(window,'-15.125 deg')
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        actual=package_geometry(path.with_suffix('.3mf'))[1]
        ref=Path(tmp)/'elbow-reference.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        for (name,(box,volume)),(other,(reference,expected)) in zip(actual,package_geometry(ref)[1],strict=True):
            assert name==other and math.isclose(volume,expected,rel_tol=.00001)
            assert all(abs(x-y)<.0001 for a,b in zip(box,reference) for x,y in zip(a,b))
        key('Escape')
        app.wait(3)
        # An occurrence chooser uses evaluated instance paths, including repeated
        # and nested definitions. View commands leave document history intact.
        path = Path(tmp) / 'Support chooser.pcad'
        sensor = (root/'tests/fixtures/sensor-unit.pcad').read_bytes()
        path.write_bytes(sensor)
        app, window = open_app(path, 'PCAD design opened')
        click(window, 150, 110, button=3)
        verify(window, 'Fit view')
        key('Return')
        verify(window, 'View fitted')
        key('t')
        focus_search(window, 75)
        for letter in 'post':
            key(letter)
        click(window, 75, 249, count=2)
        verify(window, 'Occurrence post-a')
        key('Right')
        verify(window, 'Occurrence post-b')
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'part-occurrence-chooser.png'], check=True)
        key('Return')
        verify(window, 'Isolated Sensor mounting support')
        key('Shift_L+F10')
        verify(window, 'Fix Sensor mounting support')
        key('End')
        key('Return')
        verify(window, 'View fitted')
        key('Menu')
        verify(window, 'Fix Sensor mounting support')
        key('Down')
        key('Home')
        key('space')
        verify(window, 'Edited')
        key('Control_L+s')
        verify(window, 'Saved')
        expected = json.loads(sensor)
        saved = json.loads(path.read_bytes())
        pins = saved['assembly'].pop('fixed_instances')
        assert list(pins)==['post-b'] and saved==expected
        key('Shift_L+F10')
        verify(window, 'Unfix Sensor mounting support')
        key('Tab')
        verify(window, 'Menu dismissed')
        key('Shift_L+F10')
        verify(window, 'Unfix Sensor mounting support')
        key('Return')
        verify(window, 'Edited')
        key('Control_L+s')
        verify(window, 'Saved')
        assert json.loads(path.read_bytes())==expected
        key('F11')
        verify(window, 'Showing all parts')
        focus_search(window, 260)
        for letter in 'controller-pcb':
            key('minus' if letter=='-' else letter)
        click(window, 75, 165, count=2)
        verify(window, 'Isolated Controller board')
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Arm chooser.pcad'
        arm = json.loads((root/'rust/fixtures/assembly.pcad').read_bytes())
        arm['selectedPath'] = [2]
        original_arm = json.dumps(arm).encode()
        path.write_bytes(original_arm)
        app, window = open_app(path, 'Assembly opened')
        key('t')
        focus_search(window, 75)
        for letter in 'part-arm':
            key('minus' if letter=='-' else letter)
        click(window, 75, 165)
        verify(window, 'Occurrence arm-1')
        click(window, 157, 617)
        verify(window, 'Occurrence arm-2')
        click(window, 55, 617)
        verify(window, 'Isolated Forearm')
        key('F11')
        verify(window, 'Showing all parts')
        key('Escape')
        app.wait(3)
        assert path.read_bytes()==original_arm
        # Open saved curves, then author complete drawing entities through the native tools.
        path = Path(tmp) / 'Spline fixture.pcad'
        path.write_bytes((root/'rust/fixtures/splines.pcad').read_bytes())
        app, window = open_app(path, 'Click two rectangle corners')
        capture(window, 'spline-sketch.png', True)
        # The off-centre curve fits on opening. Picking at the view centre
        # writes the fitted world coordinate into the drawing.
        key('Home')
        verify(window, 'Sketch fitted')
        key('l')
        click(window,480,358)
        click(window,500,358)
        key('Control_L+s')
        verify(window,'Saved')
        after_fit = json.loads(path.read_bytes())['items'][0]['data']['cad']['nodes'][-1]
        assert abs(after_fit['p0']['x'] + 7.5) < .00001 and abs(after_fit['p0']['y']) < .00001, after_fit
        key('Control_L+z')
        key('Control_L+s')
        verify(window,'Saved')
        key('e')
        verify(window, 'Extruded')
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        assert abs(package_geometry(path.with_suffix('.3mf'))[1][0][1][1] - 97590.235) < .01
        key('Control_L+z')
        key('Escape')
        app.wait(3)

        path = Path(tmp) / 'Arched plate.pcad'
        app, window = open_app(path, 'Click two rectangle corners')
        click(window, 580, 20)
        click(window, 240, 55)
        verify(window, 'Click the arc start')
        click(window, 400, 358)
        click(window, 480, 278)
        click(window, 560, 358)
        verify(window, 'Curve added')
        key('Control_L+z')
        key('Control_L+y')
        key('l')
        click(window, 560, 358)
        click(window, 400, 358)
        verify(window, 'Sketch edited')
        key('Control_L+s')
        verify(window, 'Saved')
        drawing = json.loads(path.read_bytes())
        arc = next(n['arc'] for n in drawing['items'][0]['data']['cad']['nodes'] if n['type']=='sketchArc')
        assert arc['center'] == dict(x=0,y=0) and arc['radius'] == 20 and abs(arc['sweep']+math.pi)<1e-10
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'Click two rectangle corners')
        capture(window, 'reopened-arc.png', True)
        key('e')
        verify(window, 'Extruded')
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        actual = package_geometry(path.with_suffix('.3mf'))[1][0][1]
        assert actual[0]==[(-20,20),(0,20),(0,10)] and abs(actual[1] - 64*200*math.sin(math.pi/64)*10) < .005
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Drawn splines.pcad'
        app, window = open_app(path, 'Click two rectangle corners')
        key('b')
        for x,y in [(550,358),(570,300),(590,415)]:
            click(window,x,y)
        key('Return')
        verify(window, 'Invalid spline point count')
        click(window,610,358)
        click(window,561,55)  # Finish
        verify(window,'Curve added')
        key('s')
        click(window,650,320)
        click(window,670,310)
        key('Escape')
        key('k')
        for x,y in [(400,380),(440,380),(440,340),(399,338)]:
            click(window,x,y)
        key('BackSpace')
        click(window,400,340)
        key('Return')
        verify(window,'Curve added')
        key('Control_L+s')
        verify(window,'Saved')
        draw = json.loads(path.read_bytes())
        splines = [n['spline'] for n in draw['items'][0]['data']['cad']['nodes'] if n['type']=='sketchSpline']
        assert [(s['mode'],s['closed'],len(s['points'])) for s in splines]==[('control',False,4),('fit',True,4)]
        capture(window,'authored-splines.png',True)
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'Click two rectangle corners')
        key('e')
        verify(window,'Extruded')
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        assert package_geometry(path.with_suffix('.3mf'))[1][0][1][1] > 1000
        key('Control_L+z')
        key('Escape')
        app.wait(3)
        # Track a thin red square on an off-center point of the top surface.
        # Its rendered centroid gives a pixel-space oracle independent of the picker.
        from PIL import Image, ImageChops
        path = Path(tmp) / 'Orbit pivot.pcad'
        pivot_doc = dict(format='puppycorp.pcad', schema_version=1, units='mm',
                         nodes=[dict(id='base', type='box', size=[80,50,6]),
                                dict(id='mark', type='box', size=[4,4,.05])],
                         parts=[dict(id='base',node='base'),dict(id='mark',node='mark',color='#ff0000')],
                         assembly=dict(instances=[dict(id=i,part=i) for i in ('base','mark')],
                                       mates=[dict(id='ground',type='fixed',parent='world',child='base'),
                                              dict(id='spot',type='fixed',parent='base',child='mark',
                                                   parent_frame=dict(position=[15,29,6]))]))
        path.write_text(json.dumps(pivot_doc))
        app, window = open_app(path, 'PCAD design opened')
        def read_marker(file):
            subprocess.run(['magick','import','-window',hex(window),file],check=True)
            with Image.open(file) as shot:
                region = shot.convert('RGB').crop((0,76,800,640))
            red, green, blue = region.split()
            mask = ImageChops.subtract(red,ImageChops.lighter(green,blue)).point(lambda n: 255 if n>110 else 0)
            box = mask.getbbox()
            assert box
            pixels = [(x,y+76) for x in range(box[0],box[2]) for y in range(box[1],box[3]) if mask.getpixel((x,y))]
            assert len(pixels)>8, pixels
            return tuple(sum(axis)/len(pixels) for axis in zip(*pixels)), region
        for projection in ('orthographic','perspective'):
            if projection=='perspective':
                key('o')
                verify(window,'Perspective')
            key('0')
            verify(window,'Isometric view')
            before, original_image = read_marker(Path(tmp)/'before-orbit.png')
            start = tuple(round(v) for v in before)
            drag(window,3,start,(start[0]+96,start[1]+36))
            for _ in range(50):
                after, rotated_image = read_marker(output_dir/f'orbit-pivot-{projection}.png')
                if ImageChops.difference(original_image, rotated_image).getbbox():
                    break
                time.sleep(.05)
            else:
                raise AssertionError('Orbit did not rotate the drawing')
            assert max(abs(a-b) for a,b in zip(before,after))<1, (projection,before,after)
            drag(window,2,(400,320),(455,338))
            click(window,455,338,button=4)
            key('0')
            verify(window,'Isometric view')
            for _ in range(50):
                _, refitted = read_marker(Path(tmp)/'refitted.png')
                if not ImageChops.difference(original_image,refitted).getbbox():
                    break
                time.sleep(.05)
            else:
                raise AssertionError('Standard view did not restore its framing')

        key('Escape')
        app.wait(3)
        # Two occurrences share one part. From the top, the higher one covers
        # the centre of the lower one, leaving a strip on either side to pick.
        path = Path(tmp) / 'Pair.pcad'
        pair = dict(format='puppycorp.pcad', schema_version=1, units='mm',
                    nodes=[dict(id='plate', type='box', size=[40, 30, 5])],
                    parts=[dict(id='plate', node='plate', name='Plate')],
                    assembly=dict(instances=[dict(id='lower', part='plate', name='Lower plate'),
                                             dict(id='upper', part='plate', name='Upper plate')],
                                  mates=[dict(id='ground', type='fixed', parent='world', child='lower'),
                                         dict(id='stack', type='fixed', parent='lower', child='upper',
                                              parent_frame=dict(position=[18, 0, 12]))]))
        original_bytes = json.dumps(pair).encode()
        path.write_bytes(original_bytes)
        app, window = open_app(path, 'PCAD design opened')
        # Select a visible cube face, then choose the three opposite views.
        click(window, 923, 135)
        verify(window, 'Right view')
        for index, label in [(3, 'Back'), (4, 'Left'), (5, 'Bottom')]:
            click(window, 905, 182)
            click(window, 905, 182 + (index+1)*24)
            verify(window, label+' view')
            click(window, 904, 128)
            verify(window, label+' view')
        click(window, 905, 182)
        click(window, 905, 350)
        verify(window, 'Isometric view')
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'view-cube.png'], check=True)
        key('3')
        verify(window, 'Top view')
        click(window, 480, 320)
        verify(window, 'Selected Upper plate')
        click(window, 397, 320, count=2)
        verify(window, 'Isolated Lower plate')
        click(window, 480, 320)
        verify(window, 'Selected Lower plate')
        # Camera now fits the lower occurrence. Upper plate geometry at the
        # right edge must no longer be visible or selectable.
        click(window, 646, 320)
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'isolated-occurrence.png'], check=True)
        from PIL import Image
        isolated_image = Image.open(output_dir/'isolated-occurrence.png').convert('RGB')
        assert isolated_image.getpixel((646, 320)) == (20, 29, 33)
        assert isolated_image.getpixel((480, 320)) != (20, 29, 33)
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        assert [name for name, _ in package_geometry(path.with_suffix('.3mf'))[1]] == ['Lower plate', 'Upper plate']
        click(window, 55, 20)
        verify(window, 'Showing all parts')
        click(window, 480, 320)
        verify(window, 'Selected Upper plate')
        click(window, 46, 20)
        focus_search(window, 75)
        for stroke in ('Shift_L+l', 'o', 'w'):
            key(stroke)
        # Filtering "Low" leaves one occurrence. It stays selectable even
        # though it is occluded at the centre of the viewport.
        click(window, 92, 163)
        verify(window, 'Selected Lower plate')
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'occurrence-browser.png'], check=True)
        click(window, 58, 615)
        verify(window, 'Isolated Lower plate')
        click(window, 55, 20)
        verify(window, 'Showing all parts')
        focus_search(window, 260)
        key('u')
        key('p')
        click(window, 92, 163)
        verify(window, 'Selected Upper plate')
        key('t')
        xlib.XResizeWindow.argtypes = [pointer, XID, C.c_uint, C.c_uint]
        xlib.XResizeWindow(connection, window, 420, 460)
        xlib.XSync(connection, 0)
        time.sleep(.2)
        click(window, 397, 55)
        click(window, 326, 55)
        verify(window, 'plate / y  30 mm')
        click(window, 206, 55)
        verify(window, 'plate / y  31 mm')
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'compact-objects.png'], check=True)
        assert Image.open(output_dir/'compact-objects.png').size == (420, 460)
        click(window, 397, 55)
        click(window, 206, 55)
        verify(window, 'Exported')
        edited = package_geometry(path.with_name('Pair-2.3mf'))[1]
        assert all(box[1] == (0, 31) for _, (box, _) in edited)
        key('Control_L+z')
        verify(window, 'plate / y  30 mm')
        key('Escape')
        app.wait(3)
        assert path.read_bytes() == original_bytes
        nested = json.loads(original_bytes)
        nested['parts'][0]['name'] = 'Stock'
        module = nested['assembly']
        module.update(id='paired', name='Plate pair')
        nested['assembly_definitions'] = [module]
        nested['assembly'] = dict(instances=[dict(id=i, assembly='paired') for i in ('left', 'right')],
                                  mates=[dict(id='mount-'+i, type='fixed', parent='world', child=i,
                                              parent_frame=dict(position=[x, 0, 0]))
                                         for i, x in [('left', 0), ('right', 75)]])
        path = Path(tmp) / 'Nested pairs.pcad'
        path.write_text(json.dumps(nested))
        app, window = open_app(path, 'PCAD design opened')
        key('t')
        focus_search(window, 75)
        for letter in 'stock':
            key(letter)
        click(window, 75, 165)
        verify(window, 'Occurrence left/lower')
        for item in ('left/upper','right/lower','right/upper'):
            click(window, 157, 617)
            verify(window, 'Occurrence '+item)
        subprocess.run(['magick','import','-window',hex(window),output_dir/'nested-definition.png'],check=True)
        key('Return')
        verify(window, 'Isolated Upper plate')
        key('F11')
        verify(window, 'Showing all parts')
        focus_search(window, 260)
        key('Escape')
        # Fold the left pair, then search through the collapsed branches.
        click(window, 75, 163)
        focus_search(window, 75)
        for stroke in ('r', 'i', 'g', 'h', 't', 'slash', 'u', 'p'):
            key(stroke)
        click(window, 95, 192)
        verify(window, 'Selected Upper plate')
        key('p')
        verify(window, 'Edited')
        key('Control_L+s')
        verify(window, 'Saved')
        pins = json.loads(path.read_bytes())['assembly']['fixed_instances']
        assert list(pins) == ['right/upper']
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'nested-occurrence.png'], check=True)
        focus_search(window, 260)
        key('y')
        click(window, 94, 164)
        verify(window, 'plate / y')
        key('KP_Add')
        verify(window, '31 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        assert json.loads(path.read_bytes())['nodes'][0]['size'] == [40, 31, 5]
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Sensor unit.pcad'
        original = json.loads((root / 'tests/fixtures/sensor-unit.pcad').read_bytes())
        path.write_text(json.dumps(original))
        app, window = open_app(path, 'PCAD design opened')
        for stroke, result in [('KP_Add', '46 deg'), ('Control_L+z', '45 deg'),
                               ('Control_L+y', '46 deg'), ('Control_L+s', 'Saved')]:
            key(stroke)
            verify(window, result)
        key('Escape')
        app.wait(3)
        original['assembly']['mates'][1]['value'] = 46
        assert json.loads(path.read_bytes()) == original
        app, window = open_app(path, 'PCAD design opened')
        verify(window, '46 deg')
        capture(window, 'sensor-orthographic.png', False, surface=(130, 149, 149))
        from PIL import Image
        def viewport_pixels(file):
            with Image.open(file) as image:
                return image.convert('RGB').crop((0, 76, 960, 640)).tobytes()

        def await_view(file, reference, equal):
            for _ in range(60):
                subprocess.run(['magick', 'import', '-window', hex(window), file], check=True)
                if (viewport_pixels(file) == reference) == equal:
                    return
                time.sleep(.05)
            raise AssertionError((file, equal))

        original_view = viewport_pixels(output_dir/'sensor-orthographic.png')
        click(window, 880, 20)
        verify(window, 'Perspective')
        await_view(output_dir/'sensor-unit.png', original_view, False)
        key('o')
        verify(window, 'Orthographic')
        await_view(Path(tmp)/'restored.png', original_view, True)
        click(window, 470, 305)
        verify(window, 'Selected Enclosure lid')
        drag(window, 1, (470, 305), (481, 283), cancel=True)
        verify(window, 'Drag cancelled')
        click(window, 150, 110)
        await_view(Path(tmp)/'cancelled.png', original_view, True)
        key('3')
        verify(window, 'Top view')
        click(window, 465, 335)
        verify(window, 'Selected Enclosure lid')
        for stroke, status in [('1', 'Front view'), ('2', 'Right view'), ('0', 'Isometric view')]:
            key(stroke)
            verify(window, status)
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'PCAD design opened')
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        click(window, 858, 55)
        verify(window, 'Sensor unit.step')
        step = path.with_suffix('.step').read_bytes()
        assert step.count(b'=NEXT_ASSEMBLY_USAGE_OCCURRENCE(') == 6 and step.count(b'=FACETED_BREP(') == 5
        key('Control_L+Shift_L+e')
        verify(window, 'Sensor unit-2.step')
        assert path.with_name('Sensor unit-2.step').read_bytes() == step
        subprocess.run(['magick', 'import', '-window', hex(window), output_dir/'sensor-step-export.png'], check=True)
        sensor_parts = dict(package_geometry(path.with_suffix('.3mf'))[1])
        assert set(sensor_parts) == {'Enclosure base', 'Enclosure lid', 'Controller board', 'Sensor board', 'Controller mounting support', 'Sensor mounting support'}, sensor_parts.keys()
        assert sensor_parts['Enclosure lid'][0] == [(0, 102), (-1.43868, 29.175636), (13, 44.601596)]
        click(window, 470, 305)
        verify(window, 'Selected Enclosure lid')
        drag(window, 1, (470, 305), (481, 283), cancel=True)
        verify(window, 'Drag cancelled')
        drag(window, 1, (470, 305), (481, 283))
        verify(window, 'Hinge moved')
        key('Control_L+z')
        verify(window, '46 deg')
        click(window, 470, 305, button=3)
        verify(window, 'Fix Enclosure lid')
        key('Escape')
        verify(window, 'Menu dismissed')
        click(window, 470, 305, button=3)
        verify(window, 'Fix Enclosure lid')
        menu_image = output_dir/'occurrence-menu.png'
        for _ in range(60):
            subprocess.run(['magick', 'import', '-window', hex(window), menu_image], check=True)
            with Image.open(menu_image) as frame:
                if frame.convert('RGB').getpixel((550, 312)) == (36, 53, 57):
                    break
            time.sleep(.05)
        else:
            raise AssertionError('Fix menu was not drawn over the picked lid')
        click(window, 493, 317)
        verify(window, 'Edited')
        key('KP_Add')
        verify(window, '47 deg')
        key('Control_L+s')
        verify(window, 'Saved')
        saved = json.loads(path.read_bytes())
        assert set(saved['assembly']['fixed_instances']) == {'lid'}
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'PCAD design opened')
        verify(window, '47 deg')
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        assert dict(package_geometry(path.with_name('Sensor unit-2.3mf'))[1]) == sensor_parts
        click(window, 470, 305, button=3)
        verify(window, 'Unfix Enclosure lid')
        click(window, 493, 317)
        verify(window, 'Edited')
        key('Control_L+s')
        verify(window, 'Saved')
        assert 'fixed_instances' not in json.loads(path.read_bytes())['assembly']
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'PCAD design opened')
        click(window, 470, 305, button=3)
        verify(window, 'Fix Enclosure lid')
        key('Escape')
        verify(window, 'Menu dismissed')
        key('Escape')
        app.wait(3)
        # Standalone harness coordinates are solved in assembly space. Open
        # the lid to see inside and compare the routed segments with the
        # outline of a single isolated part.
        path = Path(tmp) / 'Routed sensor.pcad'
        routed = json.loads((root / 'tests/fixtures/sensor-unit.pcad').read_bytes())
        routed['assembly']['mates'][1]['value'] = 90
        wiring = routed['electrical']['harness']
        wiring['cables'][0]['jacket'] = dict(
            breakouts=[dict(id='opening', position=[36,20,9]), dict(id='closing', position=[59,20,9])],
            guides=[dict(id='clip', position=[47,24,9])], lead_mm=2, bend_radius_mm=3,
            allowance_mm=3, diameter_mm=3.4, clearance_mm=.25, cut_length_mm=40)
        wiring['wires'][0]['route'].update(jacket_direction='forward', branches=[[], []])
        wiring['splices'] = [dict(id='inspection-splice', name='Inspection splice',
                                  placement=dict(instance_id='base', datum_id='sensor-seat', position=[-7,17,5]))]
        path.write_text(json.dumps(routed))
        app, window = open_app(path, 'PCAD design opened')
        key('3')
        verify(window, 'Top view')
        def path_colors(output):
            subprocess.run(['magick', 'import', '-window', hex(window), output], check=True)
            image = Image.open(output).convert('RGB').crop((300, 170, 700, 480))
            return {rgb: count for count, rgb in image.getcolors(image.width*image.height)}
        wire, jacket, splice = (224,122,26), (38,122,184), (140,48,173)
        colors = path_colors(output_dir/'sensor-harness.png')
        assert colors.get(wire, 0) >= 15 and colors.get(jacket, 0) >= 25 and colors.get(splice, 0) >= 3, colors
        # The enclosure and both boards carry different authored part colors.
        assert colors.get((103, 118, 122), 0) > 1000 and colors.get((40, 116, 79), 0) > 1000, colors
        click(window, 485, 310, count=2)
        verify(window, 'Isolated')
        colors = path_colors(Path(tmp)/'isolated.png')
        assert all(color not in colors for color in (wire, jacket, splice))
        key('F11')
        verify(window, 'Showing all parts')
        colors = path_colors(Path(tmp)/'restored.png')
        assert all(color in colors for color in (wire, jacket, splice))
        key('Escape')
        app.wait(3)
        # A splice mounted on the moving lid must follow it during preview.
        # Escape should return its marker to precisely the same screen pixels.
        routed['assembly']['mates'][1]['value'] = 46
        wiring['splices'][0]['placement'] = dict(instance_id='lid', position=[50, 28, 4])
        path.write_text(json.dumps(routed))
        app, window = open_app(path, 'PCAD design opened')
        # This off-axis drag was calibrated in the perspective sensor view.
        key('o')
        verify(window, 'Perspective')
        def splice_pixels(name):
            image_path = output_dir / name
            subprocess.run(['magick', 'import', '-window', hex(window), image_path], check=True)
            image = Image.open(image_path).convert('RGB').crop((300, 180, 700, 460))
            points = {(x, y) for y in range(image.height) for x in range(image.width)
                      if image.getpixel((x, y)) == splice}
            assert len(points) >= 3, (name, points)
            return points
        initial_splice = splice_pixels('hinge-harness.png')
        def check_moved_splice():
            verify(window, 'Dragging hinge')
            moved = splice_pixels('hinge-harness-preview.png')
            assert moved.isdisjoint(initial_splice), (initial_splice, moved)
        drag(window, 1, (470, 305), (481, 283), cancel=True, preview=check_moved_splice)
        verify(window, 'Drag cancelled')
        assert splice_pixels('hinge-harness-restored.png') == initial_splice
        assert json.loads(path.read_bytes()) == routed
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'trial.pcad'
        path.write_bytes((root / 'rust/fixtures/extruded-rectangle.pcad').read_bytes())
        app, window = open_app(path)
        for stroke, result in [('Tab', 'width 160 mm'), ('KP_Add', 'width 161 mm'), ('Control_L+z', 'width 160 mm'), ('Control_L+y', 'width 161 mm'), ('Control_L+s', 'Saved')]:
            key(stroke)
            verify(window, result)
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        key('Tab')
        verify(window, 'width 161 mm')
        key('v')
        verify(window, '|  Sketch')
        capture(window, 'reopened-sketch.png', True)
        key('v')
        verify(window, '|  Solid')
        capture(window, 'reopened-solid.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        exported = Path(re.search('Exported (.+?)"', title(window)).group(1))
        rust = Path(tmp) / 'reference.stl'
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, rust], check=True)
        a = triangulation(exported)
        b = triangulation(rust)
        assert a[0] == b[0] == [(-80.5, 80.5), (-50, 50), (0, 60)] and math.isclose(a[1], 966000) and math.isclose(b[1], 966000)
        key('Control_L+Alt_L+e')
        verify(window, 'trial.3mf')
        title_text, objects = package_geometry(path.with_suffix('.3mf'))
        assert title_text == 'Extrusion preview' and objects == [('Block', a)]
        key('Control_L+Alt_L+e')
        verify(window, 'trial-2.3mf')
        assert path.with_suffix('.3mf').read_bytes() == path.with_name('trial-2.3mf').read_bytes()
        print('Reopened native Export control and Rust export: bounds', a[0], 'volume', a[1])
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'segmented.pcad'
        path.write_bytes((root / 'rust/fixtures/segmented-primitives.pcad').read_bytes())
        app, window = open_app(path)
        key('Tab')
        verify(window, 'depth 40 mm')
        key('KP_Add')
        verify(window, 'depth 41 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        verify(window, 'depth 41 mm')
        key('v')
        verify(window, '|  Sketch')
        capture(window, 'segmented-sketch.png', True)
        from PIL import Image
        pixels = Image.open(output_dir / 'segmented-sketch.png').convert('RGB')
        mid = pixels.height // 2
        crossings = [x for x in range(pixels.width) if pixels.getpixel((x, mid)) == (143, 217, 184)]
        assert len(crossings) == 4, crossings
        key('v')
        verify(window, '|  Solid')
        capture(window, 'segmented-solid.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, rust], check=True)
        bounds, volume = triangulation(path.with_suffix('.stl'))
        reference_bounds, reference_volume = triangulation(rust)
        assert bounds == reference_bounds and math.isclose(volume, reference_volume, rel_tol=0.00001)
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'face-sketch.pcad'
        path.write_bytes((root / 'rust/fixtures/face-sketch.pcad').read_bytes())
        app, window = open_app(path)
        key('KP_Add')
        verify(window, 'Block  depth 21 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        verify(window, 'Block  depth 21 mm')
        for target in ['width 80 mm', 'height 60 mm', 'Pocket  depth 8 mm']:
            key('Tab')
            verify(window, target)
        key('KP_Add')
        verify(window, 'Pocket  depth 9 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        capture(window, 'face-sketch-solid.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, rust], check=True)
        exported = path.with_suffix('.stl')
        assert horizontal_surfaces(exported, 1.123, 2.234) == horizontal_surfaces(rust, 1.123, 2.234) == {0, 12}
        bounds, volume = triangulation(exported)
        reference_bounds, reference_volume = triangulation(rust)
        assert bounds == reference_bounds and math.isclose(volume, reference_volume, rel_tol=0.00001)
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Authored plate.pcad'
        app, window = open_app(path, 'Click two rectangle corners')
        assert not path.exists()
        click(window, 320, 458)
        verify(window, 'Click the opposite corner')
        click(window, 640, 258)
        verify(window, 'Sketch edited')
        click(window, 317, 55)
        verify(window, 'Click the centre')
        click(window, 480, 358)
        click(window, 520, 358)
        verify(window, 'Sketch edited')
        capture(window, 'authored-sketch.png', True, crossing_y=350)
        key('Control_L+s')
        verify(window, 'Saved')
        graph = json.loads(path.read_bytes())['items'][0]['data']['cad']['nodes']
        assert graph[-2]['p0'] == {'x': -40, 'y': -25} and graph[-2]['p1'] == {'x': 40, 'y': 25}
        assert graph[-1]['primitive']['radius'] == 10
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'Click two rectangle corners')
        click(window, 402, 55)
        verify(window, 'Extrusion 1  depth 10 mm')
        key('Control_L+z')
        verify(window, '|  Sketch  |  Sketch 1')
        key('Control_L+y')
        verify(window, 'Extrusion 1  depth 10 mm')
        key('KP_Add')
        verify(window, 'depth 11 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        verify(window, 'depth 11 mm')
        capture(window, 'authored-solid.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, rust], check=True)
        bounds, volume = triangulation(path.with_suffix('.stl'))
        ref_bounds, ref_volume = triangulation(rust)
        area = 80*50 - 24*100*math.sin(math.tau/48)
        assert bounds == ref_bounds == [(-40, 40), (-25, 25), (0, 11)]
        assert math.isclose(volume, 11*area, rel_tol=1e-5) and math.isclose(volume, ref_volume, rel_tol=1e-5)
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Bracket.pcad'
        app, window = open_app(path, 'Click two rectangle corners')
        click(window, 590, 55)
        verify(window, 'Click the two endpoints')
        bracket = [(400, 438), (600, 438), (600, 398), (480, 398), (480, 278), (400, 278)]
        for start, end in zip(bracket, bracket[1:] + bracket[:1]):
            click(window, *start)
            click(window, *end)
            verify(window, 'Sketch edited')
        capture(window, 'line-bracket-sketch.png', True)
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'Click two rectangle corners')
        key('e')
        verify(window, 'Extrusion 1  depth 10 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        capture(window, 'line-bracket-solid.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, rust], check=True)
        assert triangulation(path.with_suffix('.stl')) == triangulation(rust) == (
            [(-20, 30), (-20, 20), (0, 10)], 11000)
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Robot arm.pcad'
        assembly = json.loads((root / 'rust/fixtures/assembly.pcad').read_bytes())
        path.write_text(json.dumps(assembly))
        app, window = open_app(path)
        click(window, 900, 21)
        verify(window, 'Arm bar  depth 5 mm')
        key('F6')
        verify(window, 'Arm assembly  |  Assembly')
        for stroke, label in [('KP_Add', 'Elbow  31 deg'), ('Control_L+z', 'Elbow  30 deg'),
                              ('Control_L+y', 'Elbow  31 deg'), ('Control_L+s', 'Saved')]:
            key(stroke)
            verify(window, label)
        key('Escape')
        app.wait(3)
        saved = json.loads(path.read_bytes())
        assert saved['selectedPath'] == [2] and saved['items'][:2] == assembly['items'][:2]
        data = saved['items'][2]['data']
        assert data['actuators'][0]['homeDeg'] == 31
        pose = data['instances'][2]['transform']
        assert abs(pose['rotation']['z'] - 31) < .001
        assert abs(pose['translation']['x'] + 15 * math.sin(math.radians(31))) < .0001
        app, window = open_app(path, 'Assembly opened')
        verify(window, 'Elbow  31 deg')
        capture(window, 'reopened-assembly.png', False)
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        export_doc = package_geometry(path.with_suffix('.3mf'))
        ref_file = Path(tmp) / 'ref-assembly.3mf'
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, ref_file], check=True)
        ref_doc = package_geometry(ref_file)
        assert export_doc[0] == ref_doc[0] == 'Arm assembly'
        assert [p[0] for p in export_doc[1]] == [p[0] for p in ref_doc[1]] == ['Base', 'Upper arm', 'Forearm']
        for (_, (box, volume)), (_, (ref_box, ref_volume)) in zip(export_doc[1], ref_doc[1]):
            assert math.isclose(volume, ref_volume, rel_tol=.00001)
            assert all(abs(x-y)<.0001 for a,b in zip(box,ref_box) for x,y in zip(a,b))
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Drag arm.pcad'
        document = json.loads((root / 'rust/fixtures/assembly.pcad').read_bytes())
        document['selectedPath'] = [2]
        path.write_text(json.dumps(document))
        original_bytes = path.read_bytes()
        app, window = open_app(path, 'Assembly opened')
        def viewport_pixels():
            from PIL import Image
            frame = Path(tmp) / 'view.png'
            subprocess.run(['magick', 'import', '-window', hex(window), frame], check=True)
            return Image.open(frame).convert('RGB').crop((0, 76, 960, 640))
        # Wait for the first shaded frame before using its cube as the oracle.
        capture(window, Path(tmp)/'initial-arm.png', False)
        before = viewport_pixels()
        cube_region = (856, 4, 952, 93)
        drag(window, 2, (200, 160), (265, 193))
        for _ in range(60):
            panned = viewport_pixels()
            if panned.crop((200,100,740,500)).tobytes() != before.crop((200,100,740,500)).tobytes():
                break
            time.sleep(.05)
        else:
            raise AssertionError('Pan did not move the assembly geometry')
        assert panned.crop(cube_region).tobytes() == before.crop(cube_region).tobytes()
        key('Home')
        verify(window, 'View fitted')
        assert viewport_pixels().tobytes() == before.tobytes()
        drag(window, 3, (511, 255), (608, 286))
        click(window, 511, 255)
        verify(window, 'Selected Forearm')
        click(window, 150, 110)
        drag(window, 3, (200, 160), (260, 195))
        assert viewport_pixels().tobytes() != before.tobytes()
        # A tangential drag near the rim rolls the view.
        rolled_from = viewport_pixels()
        drag(window, 3, (934, 320), (924, 415))
        rolled = viewport_pixels()
        assert rolled.crop(cube_region).tobytes() != rolled_from.crop(cube_region).tobytes()
        assert rolled.crop((200, 100, 740, 500)).tobytes() != rolled_from.crop((200, 100, 740, 500)).tobytes()
        key('Escape')
        app.wait(3)
        assert path.read_bytes() == original_bytes
        app, window = open_app(path, 'Assembly opened')
        click(window, 511, 255)
        verify(window, 'Selected Forearm')
        selected = viewport_pixels()
        assert selected.tobytes() != before.tobytes()
        def check_preview():
            verify(window, 'Dragging hinge')
            preview = viewport_pixels()
            assert preview.tobytes() != selected.tobytes()
            # The lower half of the base is outside the moving forearm.
            box = (365, 262, 540, 341)
            assert preview.crop(box).tobytes() == selected.crop(box).tobytes()
        # Project the same top surface point 29 mm from the pivot at 30° and 50°.
        drag(window, 1, (511, 255), (483, 254), cancel=True, preview=check_preview)
        verify(window, 'Drag cancelled')
        assert viewport_pixels().tobytes() == selected.tobytes()
        key('Control_L+s')
        verify(window, 'Saved')
        assert path.read_bytes() == original_bytes
        drag(window, 1, (511, 255), (483, 254))
        verify(window, 'Hinge moved')
        capture(window, 'dragged-hinge.png', False)
        key('Control_L+z')
        verify(window, 'Elbow  30 deg')
        key('Control_L+y')
        verify(window, '|  Elbow')
        key('Control_L+s')
        verify(window, 'Saved')
        edited = json.loads(path.read_bytes())
        assembly = edited['items'][2]['data']
        elbow = assembly['actuators'][0]
        assert elbow['commandRange'] == {'minDeg': -90, 'maxDeg': 90}
        assert 48 < elbow['homeDeg'] < 52, elbow
        assert abs(assembly['instances'][2]['transform']['rotation']['z']-elbow['homeDeg']) < .001
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'Assembly opened')
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        reference = Path(tmp) / 'dragged-arm-ref.3mf'
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, reference], check=True)
        native = package_geometry(path.with_suffix('.3mf'))[1]
        rust_parts = package_geometry(reference)[1]
        assert [r[0] for r in native] == [r[0] for r in rust_parts] == ['Base','Upper arm','Forearm']
        for (_, (box, volume)), (_, (other, rv)) in zip(native, rust_parts):
            assert math.isclose(volume, rv, rel_tol=.00001)
            assert all(abs(a-b)<.0001 for p,q in zip(box,other) for a,b in zip(p,q))
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'Free hinge.pcad'
        original = free_mate_document('revolute')
        path.write_text(json.dumps(original))
        app, window = open_app(path, 'Assembly opened')
        click(window, 326, 55)
        verify(window, 'Solved placements')
        key('Control_L+z')
        key('Control_L+s')
        verify(window, 'Saved')
        assert json.loads(path.read_bytes())['items'] == original['items']
        key('Control_L+y')
        verify(window, '|  Assembly"')
        key('Control_L+s')
        verify(window, 'Saved')
        saved = json.loads(path.read_bytes())
        item = saved['items'][2]['data']
        assert item['actuators'] == [] and item['mates'] == original['items'][2]['data']['mates']
        pose = item['instances'][1]['transform']
        # The B connector is 10 mm down its local Y axis. Its origin must
        # coincide with (0,10,5) on A, and both local Z axes must agree.
        r = pose['rotation']
        assert abs(r['x'])<.001 and abs(r['y'])<.001
        yaw = math.radians(r['z'])
        t = pose['translation']
        assert abs(t['x'] + 10*math.sin(yaw))<.0001 and abs(t['y']-10*math.cos(yaw)-10)<.0001 and abs(t['z']-5)<.0001
        key('Escape')
        app.wait(3)
        app, window = open_app(path, 'Assembly opened')
        capture(window, 'reopened-free-hinge.png', False)
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        assert [p[0] for p in package_geometry(path.with_suffix('.3mf'))[1]] == ['Upper arm','Forearm']
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'picked-face.pcad'
        original = json.loads((root / 'rust/fixtures/extruded-rectangle.pcad').read_bytes())
        path.write_text(json.dumps(original))
        app, window = open_app(path)
        click(window, 503, 55)
        verify(window, 'Click a face for the new sketch')
        click(window, 100, 120)
        verify(window, 'Click a visible extrusion face')
        click(window, 480, 270)
        verify(window, 'Sketch 2  |  Click two rectangle corners')
        click(window, 440, 398)
        click(window, 520, 318)
        verify(window, 'Sketch edited')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        graph = json.loads(path.read_bytes())['items'][0]['data']
        nodes = graph['cad']['nodes']
        assert nodes[:-3] == original['items'][0]['data']['cad']['nodes']
        face, sketch, rectangle = nodes[-3:]
        assert face['sourceId'] == 'extrude-1' and face['faceId'] == 'extrude-1-solid-face-6'
        assert sketch['targetId'] == face['id'] and rectangle['sketchId'] == sketch['id']
        assert graph['tree']['orderedNodeIds'][-1] == graph['cad']['rootNodeIds'][-1] == sketch['id']
        app, window = open_app(path, 'Sketch 2  |  Click two rectangle corners')
        key('v')
        verify(window, '|  Solid')
        key('v')
        verify(window, '|  Sketch  |  Sketch 2')
        capture(window, 'picked-face-sketch.png', True)
        click(window, 664, 55)
        verify(window, '|  cut')
        key('e')
        verify(window, 'depth 10 mm')
        key('Control_L+z')
        verify(window, '|  Sketch  |  Sketch 2')
        key('Control_L+y')
        verify(window, '|  Solid')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        capture(window, 'picked-face-pocket.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        reference = Path(tmp) / 'reference-pocket.stl'
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, reference], check=True)
        for stl in [path.with_suffix('.stl'), reference]:
            bounds, volume = triangulation(stl)
            assert bounds == [(-80, 80), (-50, 50), (0, 60)] and math.isclose(volume, 956000)
            assert horizontal_surfaces(stl, 1.25, 2.5) == {0, 50}
        key('Left')
        key('f')
        verify(window, 'Click a face for the new sketch')
        click(window, 565, 330)
        verify(window, 'Sketch 3  |  Click two rectangle corners')
        # The right wall starts at (80,-50,0). Its U follows Y and its V follows Z.
        click(window, 560, 278)
        click(window, 640, 198)
        verify(window, 'Sketch edited')
        key('e')
        verify(window, 'depth 10 mm')
        key('Control_L+s')
        verify(window, 'Saved')
        saved_nodes = json.loads(path.read_bytes())['items'][0]['data']['cad']['nodes']
        assert saved_nodes[-4]['faceId'] == 'extrude-1-solid-face-2'
        assert saved_nodes[-1]['operation'] == 'join'
        key('Escape')
        app.wait(3)
        app, window = open_app(path)
        capture(window, 'picked-side-boss.png', False)
        key('Control_L+e')
        verify(window, 'Exported')
        export_path = Path(re.search('Exported (.+?)"', title(window)).group(1))
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, reference], check=True)
        for stl in [export_path, reference]:
            bounds, volume = triangulation(stl)
            assert bounds == [(-80, 90), (-50, 50), (0, 60)] and math.isclose(volume, 960000)
            assert horizontal_surfaces(stl, 85, -21) == {20, 40}
        key('Escape')
        app.wait(3)
        path = Path(tmp) / 'authored-assembly.pcad'
        document = json.loads((root / 'rust/fixtures/extruded-rectangle.pcad').read_bytes())
        base = document['items'][0]
        base['name'] = 'Base'
        path.write_text(json.dumps(document))
        app, window = open_app(path)
        click(window, 665, 21)
        verify(window, 'Click two rectangle corners')
        click(window, 440, 398)
        click(window, 520, 318)
        verify(window, 'Sketch edited')
        key('e')
        verify(window, 'depth 10 mm')
        click(window, 780, 21)
        verify(window, 'Choose a part to insert')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        empty = json.loads(path.read_bytes())
        assert empty['selectedPath'] == [2] and empty['items'][0] == base
        assert empty['items'][2]['data']['instances'] == []
        app, window = open_app(path, 'Assembly opened')
        click(window, 484, 55)
        verify(window, 'Inserted Base')
        click(window, 570, 55)
        verify(window, 'Insert part: Part 2')
        for _ in range(2):
            click(window, 484, 55)
            verify(window, 'Inserted Part 2')
        key('Control_L+z')
        key('Control_L+s')
        verify(window, 'Saved')
        assert len(json.loads(path.read_bytes())['items'][2]['data']['instances']) == 2
        key('Control_L+y')
        verify(window, '|  Assembly"')
        key('Control_L+s')
        verify(window, 'Saved')
        key('Escape')
        app.wait(3)
        saved = json.loads(path.read_bytes())
        assert saved['items'][:2] == empty['items'][:2] and saved['selectedPath'] == [2]
        instances = saved['items'][2]['data']['instances']
        assert len({row['id'] for row in instances}) == 3
        assert [row['partId'] for row in instances] == ['native-part-1', 'native-part-2', 'native-part-2']
        assert [row['transform']['translation']['x'] for row in instances] == [0, 100, 130]
        app, window = open_app(path, 'Assembly opened')
        capture(window, 'authored-assembly.png', False)
        key('Control_L+Alt_L+e')
        verify(window, 'Exported')
        reference = Path(tmp) / 'reference-authored.3mf'
        subprocess.run([root / 'rust/target/debug/puppycad', '--export', path, reference], check=True)
        title_text, objects = package_geometry(path.with_suffix('.3mf'))
        assert (title_text, objects) == package_geometry(reference)
        assert title_text == 'Assembly 1'
        assert [name for name, _ in objects] == ['Base 1', 'Part 2 1', 'Part 2 2']
        assert [box[0] for _, (box, _) in objects] == [(-80, 80), (90, 110), (120, 140)]
        assert [round(volume) for _, (_, volume) in objects] == [960000, 4000, 4000]
        key('Escape')
        app.wait(3)
        # A second process may create the requested path before the editor's first save.
        collision = Path(tmp) / 'race.pcad'
        app, window = open_app(collision, 'Click two rectangle corners')
        collision.write_text('external content')
        key('Control_L+s')
        verify(window, 'File changed on disk')
        assert collision.read_text() == 'external content'
        key('Escape')
        app.wait(3)
    finally:
        if app and app.poll() is None:
            app.terminate()
            try:
                app.wait(3)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait()
        server.terminate()
        server.wait()
print("Headless native editor regression passed", flush=True)
