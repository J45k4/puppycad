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
libs = Path(os.environ.get("FLEXSCRIPT_LIB", root.parent / "flexscript/libs"))
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
        xlib.XResizeWindow.argtypes = [pointer, XID, C.c_uint, C.c_uint]
        xlib.XStringToKeysym.argtypes = [C.c_char_p]
        xlib.XStringToKeysym.restype = XID
        xlib.XKeysymToKeycode.argtypes = [pointer, XID]
        xlib.XSync.argtypes = [pointer, integer]
        xtest.XTestFakeKeyEvent.argtypes = [pointer, C.c_uint, integer, XID]

        def title(window):
            return subprocess.run(['xprop', '-id', hex(window), '_NET_WM_NAME'], text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout

        def open_app(path, ready="selects a dimension", *, design_tree=False):
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
                        if not design_tree:
                            key("F3")  # Exercise unobstructed viewport gestures in the existing cases.
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

        def focus_search(window, x, *, design=False):
            from PIL import Image
            click(window, x, 126 if design else 100)
            focus_y=139 if design else 113
            # SDL must have started text input before X sends the first key.
            frame = Path(tmp) / 'search-focus.png'
            for _ in range(60):
                subprocess.run(['magick', 'import', '-window', hex(window), frame], check=True)
                with Image.open(frame) as img:
                    if img.convert('RGB').getpixel((20, focus_y)) == (143, 217, 184):
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

        def breadcrumb_frame(window, name):
            """Capture the toolbar and locate its visible breadcrumb links."""
            from PIL import Image

            output=output_dir/name
            subprocess.run(["magick","import","-window",hex(window),output],check=True)
            with Image.open(output) as image:
                rgb=image.convert("RGB")
            spans=[]
            for x in range(rgb.width):
                if rgb.getpixel((x,29)) != (143,217,184):
                    continue
                if spans and spans[-1][1]+1==x:
                    spans[-1][1]=x
                else:
                    spans.append([x,x])
            return rgb, [(left,right) for left,right in spans if right-left>=6]

        def breadcrumb_links(window, name):
            return [(left+right)//2 for left,right in breadcrumb_frame(window,name)[1]]

        # One drawing owns all three workspaces and the sketch history.
        workspace_path=Path(tmp)/"Workspaces.pcad"
        workspace_doc=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        other_assembly=json.loads(json.dumps(workspace_doc["items"][2]))
        other_assembly.update(id="inspection-assembly",name="Inspection assembly")
        other_assembly["data"].update(id="inspection-assembly",name="Inspection assembly")
        workspace_doc["items"].insert(2,other_assembly)
        workspace_doc["selectedPath"]=[3]
        workspace_path.write_text(json.dumps(workspace_doc))
        app,window=open_app(workspace_path,"Assembly opened",design_tree=True)
        assert "  |  Arm assembly  |  Assembly" in title(window)
        click(window,55,160)  # Open Base directly from the assembly workspace.
        verify(window,"Opened Base")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"workspace-part.png"],check=True)
        click(window,26,212)  # Expand Base sketch without opening it.
        assert "Opened Base" in title(window)
        click(window,110,238)
        verify(window,"Selected Corner rectangle")
        click(window,85,212)  # The sketch label opens the editor in one click.
        verify(window,"Editing Base sketch")
        key("Shift_L+period")
        click(window,490,340)
        verify(window,"Point added")
        key("Control_L+z")
        verify(window,"|  Sketch")
        key("Control_L+y")
        verify(window,"|  Sketch")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"workspace-sketch.png"],check=True)
        key("F3")  # Breadcrumb navigation also works with the sidebar hidden.
        wide,wide_links=breadcrumb_frame(window,"workspace-nested-sketch-breadcrumbs.png")
        assembly_link,part_link=[(left+right)//2 for left,right in wide_links]
        assert assembly_link<part_link
        click(window,part_link,20)
        verify(window,"Part workspace")
        assert "  |  Base  |  Solid" in title(window)
        click(window,43,92)
        click(window,229,125)
        for letter in "part-arm":
            key("minus" if letter=="-" else letter)
        key("Return")
        click(window,85,160)
        verify(window,"Opened Arm")
        assert "  |  Arm  |  Solid" in title(window)
        key("F3")  # Arm is another part in the same assembly.
        assembly_link,=breadcrumb_links(window,"workspace-nested-part-breadcrumbs.png")
        click(window,assembly_link,20)
        verify(window,"Opened Arm assembly")
        assert "  |  Arm assembly  |  Assembly" in title(window)
        key("Control_L+z")
        verify(window,"  |  Arm  |  Solid")
        assert len(breadcrumb_links(window,"workspace-breadcrumb-after-undo.png"))==1
        key("Control_L+y")
        verify(window,"  |  Arm assembly  |  Assembly")
        click(window,43,92)  # Show the Design tree after testing the hidden sidebar.
        click(window,55,160)
        verify(window,"Opened Base")
        click(window,85,212)
        verify(window,"Editing Base sketch")
        click(window,85,212,button=3)
        verify(window,"Selected Base sketch")
        menu,[_,(left,right)]=breadcrumb_frame(window,"workspace-breadcrumb-with-design-menu.png")
        assert menu.getpixel((85,214))==(89,111,109), "The Design context menu should be open"
        click(window,(left+right)//2,20)
        verify(window,"Part workspace")
        assert "  |  Base  |  Solid" in title(window)
        click(window,85,212)
        verify(window,"Editing Base sketch")
        click(window,900,20)
        verify(window,"Part workspace")
        key("F12")
        verify(window,"Editing Base sketch")
        key("F12")
        verify(window,"Part workspace")
        click(window,325,55)
        verify(window,"Editing Base sketch")
        key("F3")
        xlib.XResizeWindow(connection,window,812,640)
        xlib.XSync(connection,0)
        time.sleep(.22)
        narrow,narrow_links=breadcrumb_frame(window,"workspace-narrow-sketch-breadcrumbs.png")
        assert narrow.size==(812,640) and len(narrow_links)==2
        from PIL import Image, ImageChops

        # Compare the current Sketch word with its wide rendering, aligned to
        # the end of the Part link. The sketch name can be omitted at this width.
        wide_left=wide_links[-1][1]+1
        fill=wide.getpixel((0,8))
        wide_right=next(x for x in range(wide_left,wide.width) if wide.getpixel((x,8))!=fill)
        wide_caption=wide.crop((wide_left,8,wide_right,29))
        ink=ImageChops.difference(wide_caption,Image.new("RGB",wide_caption.size,fill))
        words=[]
        for x in range(ink.width):
            if ink.crop((x,0,x+1,ink.height)).getbbox():
                if words and x<=words[-1][1]+3:
                    words[-1][1]=x
                else:
                    words.append([x,x])
        assert len(words)>=3, "Expected the separator, Sketch and its name in the wide breadcrumb"
        sketch_start,sketch_end=words[1]
        narrow_left=narrow_links[-1][1]+1
        narrow_caption=narrow.crop((narrow_left+sketch_start,8,narrow_left+sketch_end+1,29))
        wide_sketch=wide_caption.crop((sketch_start,0,sketch_end+1,wide_caption.height))
        assert ImageChops.difference(wide_sketch,narrow_caption).getbbox() is None, \
            "The current Sketch label should remain intact near the toolbar breakpoint"

        part_link=sum(narrow_links[-1])//2
        xlib.XWarpPointer(connection,0,window,0,0,0,0,part_link,20)
        xlib.XSync(connection,0)
        time.sleep(.15)
        hover,_=breadcrumb_frame(window,"workspace-narrow-breadcrumb-hover.png")
        assert hover.getpixel((part_link,8))==(45,64,67), "The Part breadcrumb should highlight on hover"
        click(window,part_link,20)
        verify(window,"Part workspace")
        assert "  |  Base  |  Solid" in title(window)
        click(window,325,55)
        verify(window,"Editing Base sketch")
        assembly_link,part_link=breadcrumb_links(window,Path(tmp)/"nested-sketch-revisited.png")
        click(window,assembly_link,20)  # Skip Part and go straight to the original assembly.
        verify(window,"Opened Arm assembly")
        assert "  |  Arm assembly  |  Assembly" in title(window)
        xlib.XResizeWindow(connection,window,960,640)
        xlib.XSync(connection,0)
        time.sleep(.22)
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(workspace_path.read_bytes())["selectedPath"]==[3]
        click(window,43,92)
        click(window,229,125)
        for letter in "assembly-1":
            key("minus" if letter=="-" else letter)
        key("Return")
        click(window,85,160)
        verify(window,"Opened Arm assembly")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"workspace-assembly.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(workspace_path.read_bytes())
        assert saved["selectedPath"]==[3] and saved["items"][1:]==workspace_doc["items"][1:]
        base=saved["items"][0]["data"]["cad"]["nodes"]
        assert sum(n.get("primitive",{}).get("construction",False) for n in base)==1
        assert len(base)==len(workspace_doc["items"][0]["data"]["cad"]["nodes"])+1
        key("Escape")
        app.wait(3)

        crowded_path=Path(tmp)/"Crowded workspace.pcad"
        base=workspace_doc["items"][0]["data"]
        graph=base["cad"]
        sketch=next(n for n in graph["nodes"] if n["type"]=="sketch")
        outline=next(n for n in graph["nodes"] if n["type"]=="sketchCornerRectangle")
        extrusion=next(n for n in graph["nodes"] if n["type"]=="extrude")
        graph["nodes"].extend([
            dict(sketch,id="extra-sketch",name="Extra sketch"),
            dict(outline,id="extra-outline",sketchId="extra-sketch",p0={"x":30,"y":-10},p1={"x":50,"y":10}),
            dict(extrusion,id="extra-block",name="Extra block",sketchId="extra-sketch",profileId="extra-sketch-profile-1"),
        ])
        graph["rootNodeIds"].extend(["extra-sketch","extra-block"])
        base["tree"]["orderedNodeIds"].extend(["extra-sketch","extra-block"])
        crowded_path.write_text(json.dumps(workspace_doc))
        app,window=open_app(crowded_path,"Assembly opened",design_tree=True)
        click(window,55,160)
        verify(window,"Opened Base")
        key("t")
        focus_search(window,75)
        for letter in "extra":
            key(letter)
        click(window,92,163)
        verify(window,"Selected Extra block")
        click(window,58,615)
        verify(window,"Isolated Extra block")
        key("t")
        xlib.XResizeWindow(connection,window,800,640)
        xlib.XSync(connection,0)
        time.sleep(.22)
        key("v")
        verify(window,"  |  Base  |  Sketch")
        preview,preview_links=breadcrumb_frame(window,"workspace-isolated-preview-breadcrumb.png")
        assert preview.width==800 and len(preview_links)==2
        left,right=preview_links[1]
        assert right-left>=28
        click(window,(left+right)//2,20)
        verify(window,"Part workspace")
        crowded,[(left,right)]=breadcrumb_frame(window,"workspace-isolated-part-breadcrumb.png")
        assert crowded.width==800 and right-left>=65
        click(window,(left+right)//2,20)
        verify(window,"Opened Arm assembly")
        click(window,55,160)
        verify(window,"Opened Base")
        click(window,229,125)
        for letter in "inspection-assembly":
            key("minus" if letter=="-" else letter)
        key("Return")
        click(window,85,160)
        verify(window,"Opened Inspection assembly")
        key("Control_L+z")
        verify(window,"  |  Base  |  Solid")
        assembly_link,=breadcrumb_links(window,"workspace-original-parent-after-undo.png")
        click(window,assembly_link,20)
        verify(window,"Opened Arm assembly")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)

        standalone_part=Path(tmp)/"Direct Part workspace.pcad"
        saved["selectedPath"]=[0]
        standalone_part.write_text(json.dumps(saved))
        app,window=open_app(standalone_part)
        assert breadcrumb_links(window,Path(tmp)/"standalone-part.png")==[]
        click(window,325,55)
        verify(window,"Editing Base sketch")
        part_link,=breadcrumb_links(window,"workspace-standalone-sketch-breadcrumb.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        assert "  |  Base  |  Solid" in title(window)
        key("Escape")
        app.wait(3)

        dimensions_path=Path(tmp)/"Workspace dimensions.pcad"
        dimensions_doc=json.loads(workspace_path.read_bytes())
        base=dimensions_doc["items"][0]["data"]
        sketch_id="breadcrumb-dimension-sketch"
        base["cad"]["nodes"].extend([
            dict(id=sketch_id,type="sketch",name="Guide sketch",targetId="plane-top",relations=[]),
            *[dict(id=f"guide-{index}",type="sketchLine",sketchId=sketch_id,construction=True,
                   p0=dict(x=-10,y=y),p1=dict(x=10,y=y)) for index,y in enumerate((-10,10),1)],
        ])
        base["cad"]["rootNodeIds"].append(sketch_id)
        base["tree"]["orderedNodeIds"].append(sketch_id)
        dimensions_path.write_text(json.dumps(dimensions_doc))
        app,window=open_app(dimensions_path,"Assembly opened",design_tree=True)
        click(window,55,160)
        verify(window,"Opened Base")
        key("F3")
        key("F8")
        verify(window,"guide-1   20 mm   DOF")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,68,55)
        verify(window,"Pick the source line for Parallel")
        _,part_link=breadcrumb_links(window,"workspace-relations-breadcrumbs.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        assert "  |  Base  |  Solid" in title(window)
        key("F8")
        verify(window,"guide-1   20 mm   DOF")
        assert len(breadcrumb_links(window,"workspace-dimensions-breadcrumbs.png"))==2
        key("F2")
        verify(window,"Set value")
        type_value("30")
        _,part_link=breadcrumb_links(window,"workspace-dimension-entry-breadcrumbs.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        key("F8")
        verify(window,"guide-1   20 mm   DOF")
        key("F2")
        verify(window,"Set value")
        type_value("35")
        assembly_link,_=breadcrumb_links(window,"workspace-dimension-entry-revisited.png")
        click(window,assembly_link,20)
        verify(window,"Opened Arm assembly")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(dimensions_path.read_bytes())["items"]==dimensions_doc["items"]
        key("Escape")
        app.wait(3)

        preview_path=Path(tmp)/"Workspace feature preview.pcad"
        preview_doc=json.loads(workspace_path.read_bytes())
        preview_part=preview_doc["items"][0]["data"]
        preview_part["cad"]["nodes"].extend([
            dict(id="preview-sketch",type="sketch",name="Preview sketch",targetId="plane-top",relations=[]),
            dict(id="preview-outline",type="sketchCornerRectangle",sketchId="preview-sketch",
                 p0=dict(x=34,y=35),p1=dict(x=45,y=49)),
        ])
        preview_part["cad"]["rootNodeIds"].append("preview-sketch")
        preview_part["tree"]["orderedNodeIds"].insert(1,"preview-sketch")
        preview_path.write_text(json.dumps(preview_doc))
        app,window=open_app(preview_path,"Assembly opened",design_tree=True)
        click(window,55,160)
        verify(window,"Opened Base")
        click(window,85,238)
        verify(window,"Editing Preview sketch")
        key("F3")
        click(window,400,55)
        verify(window,"Extrusion preview")
        _,part_link=breadcrumb_links(window,"workspace-extrusion-breadcrumbs.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        assert "  |  Base  |  Solid" in title(window)
        key("F12")
        verify(window,"Editing Preview sketch")
        key("8")
        verify(window,"choose X or Y")
        key("x")
        verify(window,"Revolve preview")
        _,part_link=breadcrumb_links(window,"workspace-revolve-breadcrumbs.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        key("F4")
        verify(window,"Extrusion preview")
        click(window,45,55)
        verify(window,"Set value")
        type_value("24")
        xlib.XResizeWindow(connection,window,520,460)
        xlib.XSync(connection,0)
        time.sleep(.22)
        assembly_link,=breadcrumb_links(window,"workspace-extrusion-depth-breadcrumb.png")
        click(window,assembly_link,20)
        verify(window,"Opened Arm assembly")
        assert "  |  Arm assembly  |  Assembly" in title(window)
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(preview_path.read_bytes())["items"]==preview_doc["items"]
        key("Escape")
        app.wait(3)

        pattern_path=Path(tmp)/"Workspace pattern.pcad"
        pattern_path.write_bytes((root/"rust/fixtures/patterns.pcad").read_bytes())
        pattern_original=pattern_path.read_bytes()
        app,window=open_app(pattern_path,"Click two rectangle corners")
        key("F5")
        verify(window,"linear-pattern-1   Columns   3")
        part_link,=breadcrumb_links(window,"workspace-pattern-breadcrumb.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        key("F12")
        verify(window,"Editing Pattern sketch")
        key("F5")
        verify(window,"linear-pattern-1   Columns   3")
        key("F2")
        verify(window,"Set value")
        type_value("5")
        part_link,=breadcrumb_links(window,"workspace-pattern-value-breadcrumb.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        key("F12")
        verify(window,"Editing Pattern sketch")
        key("F5")
        verify(window,"linear-pattern-1   Columns   3")
        key("Escape")
        key("Control_L+s")
        verify(window,"Saved")
        assert pattern_path.read_bytes()==pattern_original
        key("Escape")
        app.wait(3)

        blank=Path(tmp)/"Workspace blank.pcad"
        app,window=open_app(blank,"Click two rectangle corners",design_tree=True)
        click(window,65,160)  # The default Part row is independent of the filename.
        verify(window,"Opened Part 1")

        from PIL import Image

        def toolbar_frame(name):
            path=output_dir/name
            subprocess.run(["magick","import","-window",hex(window),path],check=True)
            with Image.open(path) as image:
                return image.convert("RGB")

        def arrow_tones(frame,undo_x,redo_x):
            return tuple(frame.crop((x,48,x+16,64)).getextrema()[0][1] for x in (undo_x,redo_x))

        def part_sketch_pixels(name):
            image=toolbar_frame(name)
            canvas=image.crop((254,76,image.width,image.height))
            return sum(count for count,color in canvas.getcolors(canvas.width*canvas.height) if color==(138,215,232))

        toolbar=toolbar_frame("toolbar-sketch-icons.png")
        assert arrow_tones(toolbar,30,88)==(126,126)
        for x in (293,457):
            assert toolbar.getpixel((x,62))==(143,217,184)

        xlib.XResizeWindow(connection,window,420,460)
        xlib.XSync(connection,0)
        time.sleep(.22)
        compact=toolbar_frame("toolbar-compact-history.png")
        assert compact.size==(420,460)
        assert arrow_tones(compact,84,200)==(126,126)
        click(window,395,55)
        compact=toolbar_frame("toolbar-compact-sketch.png")
        for x in (41,157):
            assert compact.getpixel((x,62))==(143,217,184)
        click(window,80,55)
        verify(window,"Editing Sketch 1")
        part_link,=breadcrumb_links(window,"workspace-sketch-breadcrumb-compact.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        xlib.XResizeWindow(connection,window,960,640)
        xlib.XSync(connection,0)
        time.sleep(.22)

        click(window,325,55)
        verify(window,"Editing Sketch 1")
        click(window,440,378)
        click(window,520,338)
        verify(window,"Sketch edited")
        assert arrow_tones(toolbar_frame("toolbar-undo.png"),30,88)==(236,126)
        click(window,38,55)
        assert arrow_tones(toolbar_frame("toolbar-redo.png"),30,88)==(126,236)
        click(window,96,55)
        key("Control_L+s")
        verify(window,"Saved")
        graph=json.loads(blank.read_bytes())["items"][0]["data"]["cad"]
        assert sum(n["type"]=="sketchCornerRectangle" for n in graph["nodes"])==1
        click(window,900,20)
        verify(window,"Part workspace")
        assert part_sketch_pixels("part-sketch-visible.png")>200
        xlib.XWarpPointer(connection,0,window,0,0,0,0,232,212)
        xlib.XSync(connection,0)
        time.sleep(.15)
        assert toolbar_frame("part-sketch-visibility-tooltip.png").getpixel((136,226))==(89,111,109)
        drag(window,3,(560,320),(646,296))
        assert part_sketch_pixels(Path(tmp)/"part-sketch-orbit.png")>150
        key("3")
        verify(window,"Top view")
        key("Home")
        verify(window,"View fitted")
        assert part_sketch_pixels("part-sketch-top.png")>200
        click(window,232,212)  # The eye on Sketch 1.
        verify(window,"Hidden Sketch 1")
        assert part_sketch_pixels("part-sketch-hidden.png")==0
        key("Control_L+z")
        assert part_sketch_pixels("part-sketch-visibility-undone.png")>200
        key("Control_L+y")
        assert part_sketch_pixels("part-sketch-visibility-redone.png")==0
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(blank.read_bytes())["items"][0]["data"]
        assert saved["tree"]["sketchVisibility"]=={"native-sketch-1":False}
        assert saved["cad"]==graph
        key("Escape")
        app.wait(3)
        app,window=open_app(blank,"Click two rectangle corners",design_tree=True)
        click(window,900,20)
        verify(window,"Part workspace")
        assert part_sketch_pixels(Path(tmp)/"part-sketch-reopened.png")==0
        click(window,232,212)
        verify(window,"Shown Sketch 1")
        assert part_sketch_pixels(Path(tmp)/"part-sketch-shown.png")>200
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)

        plane_path=Path(tmp)/"Front plane.pcad"
        app,window=open_app(plane_path,"Click two rectangle corners",design_tree=True)
        click(window,900,20)
        verify(window,"Part workspace")
        click(window,503,55)
        verify(window,"Choose a plane for the new sketch")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"new-sketch-planes.png"],check=True)
        key("Escape")
        verify(window,"New sketch cancelled")
        key("Shift_L+s")
        verify(window,"Choose a plane for the new sketch")
        click(window,180,55)  # Front (XZ)
        verify(window,"Editing Sketch 2")
        click(window,440,378)
        click(window,520,338)
        verify(window,"Sketch edited")
        click(window,900,20)
        verify(window,"Part workspace")
        key("1")
        verify(window,"Front view")
        assert part_sketch_pixels("part-front-plane-sketch.png")>100
        key("Control_L+s")
        verify(window,"Saved")
        document=json.loads(plane_path.read_bytes())
        graph=document["items"][0]["data"]["cad"]
        new_sketch=next(n for n in graph["nodes"] if n["id"]=="native-sketch-2")
        assert new_sketch["targetId"]=="plane-front"
        assert graph["rootNodeIds"].count("plane-front")==1 and graph["rootNodeIds"][-1]==new_sketch["id"]
        key("Escape")
        app.wait(3)
        app,window=open_app(plane_path,"Click two rectangle corners",design_tree=True)
        click(window,228,125)
        for letter in "sketch 2":
            key("space" if letter==" " else letter)
        key("Return")
        click(window,80,186)
        verify(window,"Editing Sketch 2")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"front-reference.stl"
        subprocess.run([root/"rust/target/debug/puppycad","--export",plane_path,reference],check=True)
        assert triangulation(plane_path.with_suffix(".stl"))==triangulation(reference)==(
            [(-10,10),(0,10),(-5,5)],2000)
        shutil.copyfile(plane_path,output_dir/"front-plane.pcad")
        key("Escape")
        app.wait(3)

        # Reattach a consumed sketch to a reference plane and then to an earlier cap.
        support_path=Path(tmp)/"Sketch support.pcad"
        support_doc=json.loads((root/"rust/fixtures/extruded-rectangle.pcad").read_bytes())
        support_part=support_doc["items"][0]["data"]
        sk="native-sketch-2"
        outline=dict(id="support-tab",type="sketchCornerRectangle",sketchId=sk,p0=dict(x=-12,y=-8),p1=dict(x=12,y=8))
        tab=dict(id="support-extrude",name="Tab",type="extrude",sketchId=sk,profileId=f"{sk}-profile-1",operation="newBody",depth=10)
        sketch=dict(id=sk,type="sketch",name="Tab sketch",targetId="plane-right",relations=[],dimensions=[],variables=[],referenceNote="keep")
        support_part["cad"]["nodes"].extend([sketch,outline,tab])
        support_part["cad"]["rootNodeIds"].extend([sk,tab["id"]])
        support_part["tree"]["orderedNodeIds"].extend([sk,tab["id"]])
        support_path.write_text(json.dumps(support_doc))
        shutil.copyfile(support_path,output_dir/"sketch-support-input.pcad")
        app,window=open_app(support_path,design_tree=True)

        def open_support(row_y):
            click(window,83,row_y,button=3)
            key("Up")  # Sketch support is the last action in the sketch context menu.
            key("Return")
            verify(window,"Choose a sketch support")

        def saved_support():
            key("Control_L+s")
            verify(window,"Saved")
            return {node["id"]:node for node in json.loads(support_path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}

        open_support(212)
        click(window,410,55)
        verify(window,"No earlier extrusion faces")
        key("Escape")
        verify(window,"Sketch support edit cancelled")
        assert support_path.read_bytes()==(output_dir/"sketch-support-input.pcad").read_bytes()
        open_support(264)
        click(window,180,55)
        verify(window,"Sketch support updated")
        assert "  |  Sketch  |  Tab sketch" in title(window)
        assert saved_support()[sk]==dict(sketch,targetId="plane-front")
        key("Control_L+z")
        assert saved_support()[sk]==sketch
        key("Control_L+y")
        assert saved_support()[sk]==dict(sketch,targetId="plane-front")
        key("F12")
        verify(window,"Part workspace")
        open_support(264)
        click(window,410,55)
        verify(window,"Click an earlier extrusion face")
        key("3")
        verify(window,"Click an earlier extrusion face")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-support-picker.png"],check=True)
        click(window,480,320)
        verify(window,"Sketch support updated")
        nodes=saved_support()
        face_id="face-extrude-1-extrude-1-solid-face-6"
        assert nodes[sk]==dict(sketch,targetId=face_id)
        assert nodes[outline["id"]]==outline and nodes[tab["id"]]==tab
        assert nodes[face_id]==dict(id=face_id,type="face",sourceId="extrude-1",faceId="extrude-1-solid-face-6")
        key("Escape")
        app.wait(3)
        app,window=open_app(support_path)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"sketch-support-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",support_path,reference],check=True)
        compare_solid_packages(support_path.with_suffix(".3mf"),reference)
        tab_bounds,tab_volume=package_geometry(support_path.with_suffix(".3mf"))[1][1][1]
        assert tab_bounds==[(-12,12),(-8,8),(60,70)] and math.isclose(tab_volume,3840,rel_tol=1e-6)
        shutil.copyfile(support_path,output_dir/"sketch-support-edited.pcad")
        key("Control_L+e")
        verify(window,"Exported")
        for export in support_path.parent.glob("Sketch support*.stl"):
            shutil.copyfile(export,output_dir/export.name)
        key("Escape")
        app.wait(3)

        # Depth edits are temporary until Apply commits one feature history step.
        extrusion_path=Path(tmp)/"Extrusion options.pcad"
        app,window=open_app(extrusion_path,"Click two rectangle corners")
        click(window,440,378)
        click(window,520,338)
        verify(window,"Sketch edited")

        def saved_extrusion_graph():
            key("Control_L+s")
            verify(window,"Saved")
            return json.loads(extrusion_path.read_bytes())["items"][0]["data"]["cad"]

        original_graph=saved_extrusion_graph()

        def preview_symmetric_extrusion():
            click(window,400,55)  # Extrude
            verify(window,"Extrusion preview")
            key("e")
            verify(window,"Symmetric")
            key("d")
            key("Control_L+a")
            type_value("12")
            key("Return")
            verify(window,"Extrusion preview")
            verify(window,"12 mm")

        preview_symmetric_extrusion()
        capture(window,"extrusion-symmetric-preview.png",False,surface=(76,167,192))
        key("Escape")
        verify(window,"Extrusion cancelled")
        assert saved_extrusion_graph()==original_graph
        preview_symmetric_extrusion()
        key("Return")
        verify(window,"Extruded")
        symmetric_graph=saved_extrusion_graph()
        symmetric=next(n for n in symmetric_graph["nodes"] if n["type"]=="extrude")
        assert (symmetric["operation"],symmetric["endCondition"],symmetric["depth"])==("newBody","symmetric",12)
        key("Control_L+z")
        verify(window,"|  Sketch")
        assert saved_extrusion_graph()==original_graph
        key("Control_L+y")
        verify(window,"|  Solid")
        assert saved_extrusion_graph()==symmetric_graph
        key("Escape")
        app.wait(3)

        def compare_saved_extrusion(flavor,expected_bodies):
            key("Control_L+Alt_L+e")
            verify(window,"Exported")
            native=extrusion_path.with_suffix(".3mf")
            reference=Path(tmp)/f"{flavor}-rust.3mf"
            subprocess.run([root/"rust/target/debug/puppycad","--export",extrusion_path,reference],check=True)
            compare_solid_packages(native,reference)
            assert [geometry for _,geometry in package_geometry(native)[1]]==expected_bodies
            shutil.copyfile(extrusion_path,output_dir/f"{flavor}.pcad")
            shutil.move(native,output_dir/f"{flavor}.3mf")

        app,window=open_app(extrusion_path)
        compare_saved_extrusion("extrusion-symmetric",[([(-10,10),(-5,5),(-6,6)],2400)])
        key("F4")
        verify(window,"Extrusion preview")
        key("Return")
        verify(window,"Extrusion unchanged")
        key("F4")
        verify(window,"Extrusion preview")
        verify(window,"Symmetric")
        key("e")
        verify(window,"Blind")
        key("f")
        verify(window,"Flipped")
        key("Return")
        verify(window,"Extrusion edited")
        edited_graph=saved_extrusion_graph()
        edited=[n for n in edited_graph["nodes"] if n["type"]=="extrude"]
        assert len(edited)==1 and edited[0]["id"]==symmetric["id"]
        assert "endCondition" not in edited[0] and edited[0]["flip"] is True and edited[0]["depth"]==12
        compare_saved_extrusion("extrusion-flipped",[([(-10,10),(-5,5),(-12,0)],2400)])
        key("Escape")
        app.wait(3)

        # The cut acts on the latest body, with its reach set by an earlier body.
        extrusion_path=Path(tmp)/"Extrusion target.pcad"
        target_document=json.loads((root/"rust/fixtures/extrude-options.pcad").read_bytes())
        target_part=target_document["items"][0]["data"]
        graph=target_part["cad"]
        nodes={n["id"]:n for n in graph["nodes"]}
        nodes["native-entity-1"].update(p0={"x":100,"y":-10},p1={"x":130,"y":10})
        nodes["extrude-1"]["name"]="Limit block"
        nodes["native-entity-3"].update(p0={"x":-80,"y":-50},p1={"x":80,"y":50})
        nodes["extrude-3"].update(name="Main block",operation="newBody",depth=40)
        for field in ("endCondition","targetId","flip"):
            del nodes["extrude-3"][field]
        graph["nodes"]=[n for n in graph["nodes"] if n["id"]!="extrude-2"]
        graph["rootNodeIds"].remove("extrude-2")
        target_part["tree"]["orderedNodeIds"].remove("extrude-2")
        extrusion_path.write_text(json.dumps(target_document))
        app,window=open_app(extrusion_path,design_tree=True)
        click(window,80,125)
        for letter in "hole sketch":
            key("space" if letter==" " else letter)
        key("Return")
        click(window,90,186)
        key("Return")
        verify(window,"Editing Hole sketch")
        click(window,400,55)
        verify(window,"Extrusion preview")
        key("o")
        verify(window,"Cut")
        for end in ("Symmetric","Through all","Up to body"):
            key("e")
            verify(window,end)
        verify(window,"Target Main block")
        key("Tab")
        verify(window,"Target Limit block")
        key("Tab")
        verify(window,"Target Main block")
        key("u")
        verify(window,"Click a target body")
        click(window,30,120)
        verify(window,"Click a valid earlier target body")
        click(window,550,362)
        verify(window,"Target Limit block")
        capture(window,"extrusion-target-preview.png",False,surface=(76,167,192))
        key("Return")
        verify(window,"Extruded")
        cut=next(n for n in saved_extrusion_graph()["nodes"] if n["type"]=="extrude" and n["operation"]=="cut")
        assert (cut["sketchId"],cut["endCondition"],cut["targetId"])==("native-sketch-2","up-to-body","extrude-1")
        key("Escape")
        app.wait(3)
        app,window=open_app(extrusion_path)
        compare_saved_extrusion("extrusion-up-to-body",[
            ([(100,130),(-10,10),(-30,30)],36000),
            ([(-80,80),(-50,50),(0,40)],592000),
        ])
        key("Escape")
        app.wait(3)

        # A side-face outline remains linked when its source extrusion changes.
        projected_path=Path(tmp)/"Projected side outline.pcad"
        shutil.copyfile(root/"rust/fixtures/extruded-rectangle.pcad",projected_path)
        app,window=open_app(projected_path)
        key("2")
        verify(window,"Right view")
        key("f")
        verify(window,"Click a face for the new sketch")
        click(window,480,358)
        verify(window,"Sketch 2  |  Click two rectangle corners")

        def save_projected_data():
            key("Control_L+s")
            verify(window,"Saved")
            return json.loads(projected_path.read_bytes())["items"][0]["data"]

        before_projection=save_projected_data()
        key("u")
        verify(window,"Click a body edge guide to project it")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"projected-edge-guides.png"],check=True)
        click(window,680,358)
        verify(window,"Projected body edge")
        key("Escape")
        verify(window,"Edge projection cancelled")
        single=save_projected_data()
        assert len(single["cad"]["nodes"])==len(before_projection["cad"]["nodes"])+1
        side=next(n for n in single["cad"]["nodes"] if n["id"]=="native-sketch-2")
        link,=side["relations"]
        assert link["type"]=="projected" and "sourceFaceId" not in link
        line=next(n for n in single["cad"]["nodes"] if n["id"]==link["entityId"])
        assert line["type"]=="sketchLine" and line["sketchId"]==side["id"]
        assert {(line[end]["x"],line[end]["y"]) for end in ("p0","p1")}=={(0,0),(100,0)}
        edge=link["sourceEdge"]
        assert {tuple(edge["a"]),tuple(edge["b"])}=={(80,-50,0),(80,50,0)}
        assert edge["via"]==[80,0,0] and {tuple(n) for n in edge["normals"]}=={(1,0,0),(0,0,-1)}
        key("Control_L+z")
        verify(window,"|  Sketch  |  Sketch 2")
        assert save_projected_data()==before_projection
        key("Shift_L+u")
        verify(window,"Projected face edges")

        def projected_boundary(data,depth):
            nodes={n["id"]:n for n in data["cad"]["nodes"]}
            sketch=nodes["native-sketch-2"]
            face=nodes[sketch["targetId"]]
            assert (face["sourceId"],face["faceId"])==("extrude-1","extrude-1-solid-face-2")
            lines={n["id"]:n for n in nodes.values()
                   if n["type"].startswith("sketch") and n.get("sketchId")==sketch["id"]}
            relations=[r for r in sketch["relations"] if r["type"]=="projected"]
            assert len(lines)==len(relations)==4 and {r["entityId"] for r in relations}==set(lines)
            assert all(n["type"]=="sketchLine" for n in lines.values())
            outline={frozenset((n[end]["x"],n[end]["y"]) for end in ("p0","p1")) for n in lines.values()}
            assert outline=={
                frozenset(((0,0),(100,0))),frozenset(((100,0),(100,depth))),
                frozenset(((100,depth),(0,depth))),frozenset(((0,depth),(0,0))),
            }, outline
            assert all(r["sourceFaceId"]==face["faceId"] for r in relations)
            return {(r["id"],r["entityId"],r["sourceFaceId"]) for r in relations}

        created=save_projected_data()
        links=projected_boundary(created,60)
        capture(window,"projected-side-sketch.png",False,surface=(112,191,245))
        click(window,576,20)  # More tools → Curves
        click(window,576,20)  # Conics
        click(window,715,55)  # Use edges
        verify(window,"Face edges already projected")
        assert save_projected_data()==created
        projected_outline=projected_path.read_bytes()
        key("o")
        verify(window,"|  cut")
        key("e")
        verify(window,"Extruded")
        original_cut=save_projected_data()
        assert projected_boundary(original_cut,60)==links
        cut=next(n for n in original_cut["cad"]["nodes"] if n["type"]=="extrude" and n["operation"]=="cut")
        assert (cut["sketchId"],cut["profileId"],cut["depth"])==("native-sketch-2","native-sketch-2-profile-1",10)
        key("Escape")
        app.wait(3)
        app,window=open_app(projected_path)
        key("F2")
        verify(window,"Set value")
        type_value("74")
        key("Return")
        verify(window,"Block  depth 74 mm")
        reshaped=save_projected_data()
        assert projected_boundary(reshaped,74)==links
        assert next(n for n in reshaped["cad"]["nodes"] if n["id"]==cut["id"])==cut
        key("Control_L+z")
        verify(window,"Block  depth 60 mm")
        assert save_projected_data()==original_cut
        key("Control_L+y")
        verify(window,"Block  depth 74 mm")
        assert save_projected_data()==reshaped
        key("Escape")
        app.wait(3)
        app,window=open_app(projected_path)
        key("Control_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"projected-side-rust.stl"
        subprocess.run([root/"rust/target/debug/puppycad","--export",projected_path,reference],check=True)
        expected=([(-80,70),(-50,50),(0,74)],1110000)
        assert triangulation(projected_path.with_suffix(".stl"))==triangulation(reference)==expected
        shutil.copyfile(projected_path,output_dir/"projected-side-outline.pcad")
        key("Escape")
        app.wait(3)

        stale=json.loads(projected_path.read_bytes())
        for node in stale["items"][0]["data"]["cad"]["nodes"]:
            if node["type"]=="sketchLine" and node.get("sketchId")=="native-sketch-2":
                for end in ("p0","p1"):
                    if node[end]["y"]==74:
                        node[end]["y"]=60
        projected_path=Path(tmp)/"Stale projected outline.pcad"
        cached=json.dumps(stale).encode()
        projected_path.write_bytes(cached)
        app,window=open_app(projected_path)
        assert projected_path.read_bytes()==cached
        assert projected_boundary(save_projected_data(),74)==links
        key("Escape")
        app.wait(3)

        # Draw a divider between projected edges. Both clicks miss the lines by
        # three pixels, so snapped endpoints are required for two closed regions.
        divider_path=Path(tmp)/"Projected divider.pcad"
        divider_path.write_bytes(projected_outline)
        app,window=open_app(divider_path,"Sketch 2  |  Click two rectangle corners")
        key("l")
        verify(window,"Click the two endpoints")
        click(window,620,355)
        verify(window,"Click the line endpoint")
        click(window,620,121)
        verify(window,"Sketch edited")
        key("Control_L+s")
        verify(window,"Saved")
        divider_source=divider_path.read_bytes()
        divider_graph=json.loads(divider_source)["items"][0]["data"]["cad"]
        before_ids={n["id"] for n in created["cad"]["nodes"]}
        divider,=[n for n in divider_graph["nodes"] if n["id"] not in before_ids]
        assert (divider["type"],divider["sketchId"])==("sketchLine","native-sketch-2")
        assert (divider["p0"],divider["p1"])==({"x":35,"y":0},{"x":35,"y":60})
        key("Escape")
        app.wait(3)

        # Each region makes a different cut, according to the stored profile ID.
        from PIL import Image

        for profile,volume in ((1,921000),(2,939000)):
            cut_path=Path(tmp)/f"Projected divider profile {profile}.pcad"
            cut_path.write_bytes(divider_source)
            app,window=open_app(cut_path,"Sketch 2  |  Click two rectangle corners")
            key("p")
            verify(window,"Profile 2")
            if profile==1:
                key("p")
                verify(window,"Profile 1")
            outline=output_dir/f"projected-divider-outline-{profile}.png"
            subprocess.run(["magick","import","-window",hex(window),outline],check=True)
            with Image.open(outline) as image:
                rgb=image.convert("RGB")
                left=rgb.crop((470,77,617,362))
                right=rgb.crop((623,77,884,362))
                counts=[dict((color,count) for count,color in region.getcolors(region.width*region.height))
                        .get((255,176,69),0) for region in (right,left)]
                assert counts[profile-1]>300 and counts[2-profile]==0, counts
            key("o")
            verify(window,"|  cut")
            click(window,400,55)
            verify(window,"Extrusion preview / Cut / Blind")
            capture(window,f"projected-divider-preview-{profile}.png",False,surface=(76,167,192))
            key("Return")
            verify(window,"Extruded")
            key("Control_L+s")
            verify(window,"Saved")
            cut_nodes=json.loads(cut_path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
            divider_cut,=[n for n in cut_nodes if n.get("operation")=="cut"]
            assert divider_cut["profileId"]==f"native-sketch-2-profile-{profile}" and divider_cut["depth"]==10
            assert next(n for n in cut_nodes if n["id"]==divider["id"])==divider
            key("Escape")
            app.wait(3)
            app,window=open_app(cut_path)
            key("Control_L+e")
            verify(window,"Exported")
            ref=Path(tmp)/f"projected-divider-rust-{profile}.stl"
            subprocess.run([root/"rust/target/debug/puppycad","--export",cut_path,ref],check=True)
            native_stl=cut_path.with_suffix(".stl")
            assert triangulation(native_stl)==triangulation(ref)==([(-80,80),(-50,50),(0,60)],volume)
            shutil.copyfile(cut_path,output_dir/f"projected-divider-profile-{profile}.pcad")
            shutil.copyfile(native_stl,output_dir/f"projected-divider-profile-{profile}.stl")
            key("Escape")
            app.wait(3)

        # A projected circle stays linked when a hole in its support changes.
        curved_path=Path(tmp)/"Projected rim.pcad"
        ring=json.loads((root/"rust/fixtures/face-sketch.pcad").read_bytes())
        ring_part=ring["items"][0]["data"]
        ring_graph=ring_part["cad"]
        source_circle=next(n for n in ring_graph["nodes"] if n["id"]=="native-entity-2")
        source_circle["sketchId"]="native-sketch-1"
        corners=[(-40,-30),(40,-30),(40,30),(-40,30)]
        source_outline=[
            dict(id=f"ring-edge-{i}",type="sketchLine",sketchId="native-sketch-1",
                 p0=dict(zip("xy",a)),p1=dict(zip("xy",b)))
            for i,(a,b) in enumerate(zip(corners,corners[1:]+corners[:1]),1)
        ]
        ring_graph["nodes"]=ring_graph["nodes"][:4]+source_outline+[source_circle,ring_graph["nodes"][5]]
        ring_graph["rootNodeIds"]=ring_graph["rootNodeIds"][:5]
        ring_part["tree"]["orderedNodeIds"]=["native-sketch-1","extrude-1"]
        curved_path.write_text(json.dumps(ring))
        app,window=open_app(curved_path)
        key("3")
        verify(window,"Top view")
        key("f")
        verify(window,"Click a face for the new sketch")
        click(window,555,358)  # Material outside the through-hole.
        verify(window,"Sketch 2  |  Click two rectangle corners")

        def saved_rim_graph():
            key("Control_L+s")
            verify(window,"Saved")
            return json.loads(curved_path.read_bytes())["items"][0]["data"]["cad"]

        def projected_rim(graph,radius,sketch_id="native-sketch-2",face_id="extrude-1-solid-face-38"):
            nodes={n["id"]:n for n in graph["nodes"]}
            side=nodes[sketch_id]
            face=nodes[side["targetId"]]
            assert (face["sourceId"],face["faceId"])==("extrude-1",face_id)
            relations=side["relations"]
            assert len(relations)==5 and all(r["type"]=="projected" and r["sourceFaceId"]==face["faceId"]
                                             for r in relations)
            assert sum(nodes[r["entityId"]]["type"]=="sketchLine" for r in relations)==4
            rim,=[r for r in relations if nodes[r["entityId"]]["type"]=="sketchPrimitive"]
            primitive=nodes[rim["entityId"]]["primitive"]
            assert primitive["type"]=="circle" and primitive["segments"]==32
            assert math.isclose(primitive["radius"],radius,abs_tol=.0001), primitive
            assert all(abs(value)<.0001 for value in primitive["center"].values())
            return rim

        before_rim=saved_rim_graph()
        key("Shift_L+u")
        verify(window,"Projected face edges")
        curved=saved_rim_graph()
        rim_link=projected_rim(curved,10)
        key("Control_L+z")
        verify(window,"|  Sketch  |  Sketch 2")
        assert saved_rim_graph()==before_rim
        key("Control_L+y")
        verify(window,"|  Sketch  |  Sketch 2")
        assert saved_rim_graph()==curved
        key("Escape")
        app.wait(3)
        app,window=open_app(curved_path,"Sketch 2  |  Click two rectangle corners")
        capture(window,"projected-circular-rim.png",False,surface=(112,191,245))
        from PIL import Image

        with Image.open(output_dir/"projected-circular-rim.png") as image:
            circle=image.convert("RGB").crop((427,305,533,411))
            assert sum(n for n,color in circle.getcolors(circle.width*circle.height)
                       if color==(112,191,245))>150
        key("o")
        verify(window,"|  cut")
        click(window,400,55)
        verify(window,"Extrusion preview / Cut / Blind")
        key("d")
        key("Control_L+a")
        type_value("5")
        key("Return")
        verify(window,"Blind / 5 mm")
        key("Return")
        verify(window,"Extruded")
        old_cut_graph=saved_rim_graph()
        cut,=[n for n in old_cut_graph["nodes"] if n.get("operation")=="cut"]
        assert (cut["profileId"],cut["depth"])==("native-sketch-2-profile-1",5)
        key("Escape")
        app.wait(3)

        app,window=open_app(curved_path)
        key("F12")
        verify(window,"Editing Block sketch")
        key("F8")
        for _ in source_outline:
            key("Tab")
        verify(window,"Diameter 20 mm")
        key("F2")
        type_value("26")
        key("Return")
        verify(window,"Edited")
        resized=saved_rim_graph()
        assert next(n for n in resized["nodes"] if n["id"]==source_circle["id"])["primitive"]["radius"]==13
        assert projected_rim(resized,13)==rim_link
        assert next(n for n in resized["nodes"] if n["id"]==cut["id"])==cut
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(curved_path)
        key("Control_L+e")
        verify(window,"Exported")
        curved_ref=Path(tmp)/"projected-rim-rust.stl"
        subprocess.run([root/"rust/target/debug/puppycad","--export",curved_path,curved_ref],check=True)
        expected_volume=15*(80*60-16*13**2*math.sin(math.tau/32))
        native_stl=curved_path.with_suffix(".stl")
        native_bounds,native_volume=triangulation(native_stl)
        rust_bounds,rust_volume=triangulation(curved_ref)
        assert native_bounds==rust_bounds==[(-40,40),(-30,30),(0,15)]
        assert math.isclose(native_volume,rust_volume,rel_tol=.00001)
        assert math.isclose(native_volume,expected_volume,rel_tol=.00001)
        shutil.copyfile(curved_path,output_dir/"projected-circular-rim.pcad")
        shutil.copyfile(native_stl,output_dir/"projected-circular-rim.stl")
        key("Escape")
        app.wait(3)

        # An earlier circular pocket contributes an inner loop on the top face.
        curved_path=Path(tmp)/"Projected pocket rim.pcad"
        shutil.copyfile(root/"rust/fixtures/face-sketch.pcad",curved_path)
        app,window=open_app(curved_path)
        key("3")
        verify(window,"Top view")
        key("f")
        verify(window,"Click a face for the new sketch")
        click(window,555,358)
        verify(window,"Sketch 4  |  Click two rectangle corners")
        key("Shift_L+u")
        verify(window,"Projected face edges")
        projected_rim(saved_rim_graph(),10,"native-sketch-4","extrude-1-solid-face-6")
        key("Escape")
        app.wait(3)

        # Edit a straight segment with the sketch toolbar and persist both operations.
        line_edit_path=Path(tmp)/"Trim and extend.pcad"
        line_edit_doc=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        line_edit_graph=line_edit_doc["items"][0]["data"]["cad"]
        line_edit_graph["nodes"]=line_edit_graph["nodes"][:4]
        for index,(a,b) in enumerate([((-40,0),(40,0)),((-20,-10),(-20,10)),((20,-10),(20,10))],1):
            line_edit_graph["nodes"].append({"id":f"native-entity-{index}","type":"sketchLine",
                "sketchId":"native-sketch-1","p0":dict(zip(("x","y"),a)),"p1":dict(zip(("x","y"),b))})
        line_edit_path.write_text(json.dumps(line_edit_doc))

        def saved_line_endpoints(path=line_edit_path):
            nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
            return {n["id"]:tuple((n[p]["x"],n[p]["y"]) for p in ("p0","p1"))
                    for n in nodes if n["type"]=="sketchLine"}

        original_lines=saved_line_endpoints()
        trimmed_lines=original_lines | {"native-entity-1":((-40,0),(-20,0)),"native-entity-4":((20,0),(40,0))}
        app,window=open_app(line_edit_path,"Click two rectangle corners")
        for _ in range(4):
            click(window,580,20)  # More tools → Curves → Conics → Profiles → Edit
        click(window,235,55)
        verify(window,"Click the curve segment to trim")
        click(window,480,358)
        verify(window,"Line trimmed")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-trim-toolbar.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_line_endpoints()==trimmed_lines
        for stroke,expected in [("Control_L+z",original_lines),("Control_L+y",trimmed_lines)]:
            key(stroke)
            key("Control_L+s")
            verify(window,"Saved")
            assert saved_line_endpoints()==expected
        click(window,330,55)
        verify(window,"Click near the end to extend")
        click(window,391,358)
        verify(window,"Line extended")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_line_endpoints()==trimmed_lines | {"native-entity-1":((-40,0),(20,0))}
        key("Escape")
        verify(window,"Curve edit cancelled")
        key("Escape")
        key("Escape")
        app.wait(3)

        curved_line_path=Path(tmp)/"Curved sketch boundaries.pcad"
        curved_doc=json.loads(line_edit_path.read_bytes())
        curved_part=curved_doc["items"][0]
        curved_part["name"]="Curved boundaries"
        curved_graph=curved_part["data"]["cad"]
        curved_graph["nodes"]=curved_graph["nodes"][:4]
        curved_graph["nodes"][3]["name"]="Line and circle"
        curved_graph["nodes"] += [{"id":"native-entity-1","type":"sketchLine","sketchId":"native-sketch-1",
            "p0":{"x":-30,"y":12},"p1":{"x":30,"y":12}},
            {"id":"native-entity-2","type":"sketchPrimitive","sketchId":"native-sketch-1",
             "primitive":{"type":"circle","center":{"x":0,"y":0},"radius":20,"segments":32}}]
        curved_line_path.write_text(json.dumps(curved_doc))

        def saved_curved_lines():
            graph=json.loads(curved_line_path.read_bytes())["items"][0]["data"]["cad"]
            return [(node["id"],node["p0"]["x"],node["p1"]["x"]) for node in graph["nodes"] if node["type"]=="sketchLine"]

        app,window=open_app(curved_line_path,"Click two rectangle corners")
        key("q")
        verify(window,"Click the curve segment to trim")
        click(window,480,310)
        verify(window,"Line trimmed")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_curved_lines()==[("native-entity-1",-30,-16),("native-entity-3",16,30)]
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-trim-circle.png"],check=True)
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(curved_line_path,"Click two rectangle corners")
        key("Shift_L+q")
        verify(window,"Click near the end to extend")
        click(window,400,310)
        verify(window,"Line extended")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_curved_lines()==[("native-entity-1",-30,16),("native-entity-3",16,30)]
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)

        spline_line_path=Path(tmp)/"Spline sketch boundary.pcad"
        spline_doc=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        spline_graph=spline_doc["items"][0]["data"]["cad"]
        spline_graph["nodes"]=spline_graph["nodes"][:4]
        spline_graph["nodes"][3]["name"]="Line and spline"
        # x = 30t-15 and y = 48t(1-t), so y = 9 cuts exactly at x = ±7.5.
        spline_id="native-entity-2"
        spline_node={"id":spline_id,"type":"sketchSpline","sketchId":"native-sketch-1",
                     "spline":{"id":spline_id,"type":"spline","mode":"control","closed":False,
                               "points":[{"x":x,"y":y} for x,y in [(-15,0),(-5,16),(5,16),(15,0)]]}}
        target={"id":"native-entity-1","type":"sketchLine","sketchId":"native-sketch-1",
                "p0":{"x":-25,"y":9},"p1":{"x":25,"y":9}}
        spline_graph["nodes"].extend([target,spline_node])
        spline_line_path.write_text(json.dumps(spline_doc))
        app,window=open_app(spline_line_path,"Click two rectangle corners")
        key("q")
        verify(window,"Click the curve segment to trim")
        click(window,480,322)
        verify(window,"Line trimmed")
        key("Control_L+s")
        verify(window,"Saved")
        trimmed={"native-entity-1":((-25,9),(-7.5,9)),"native-entity-3":((7.5,9),(25,9))}
        assert saved_line_endpoints(spline_line_path)==trimmed
        capture(window,"sketch-trim-spline.png",True)
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)

        app,window=open_app(spline_line_path,"Click two rectangle corners")
        key("Shift_L+q")
        verify(window,"Click near the end to extend")
        click(window,440,322)
        verify(window,"Line extended")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_line_endpoints(spline_line_path)==trimmed | {"native-entity-1":((-25,9),(7.5,9))}
        saved=json.loads(spline_line_path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        assert next(node for node in saved if node["id"]==spline_id)==spline_node
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)

        # Contacts at the two spline endpoints can also close an extrudable loop.
        spline_profile=Path(tmp)/"Spline boundary solid.pcad"
        target["p0"]["y"]=target["p1"]["y"]=0
        spline_profile.write_text(json.dumps(spline_doc))
        app,window=open_app(spline_profile,"Click two rectangle corners")
        key("q")
        click(window,400,358)
        verify(window,"Line trimmed")
        click(window,560,358)
        verify(window,"Line trimmed")
        key("Escape")
        verify(window,"Curve edit cancelled")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_line_endpoints(spline_profile)=={"native-entity-1":((-15,0),(15,0))}
        key("Escape")
        app.wait(3)
        app,window=open_app(spline_profile)
        capture(window,"sketch-spline-boundary-solid.png",False)
        key("Control_L+e")
        verify(window,"Exported")
        spline_stl=spline_profile.with_suffix(".stl")
        bounds,volume=triangulation(spline_stl)
        assert bounds==[(-15,15),(0,12),(0,10)] and math.isclose(volume,2400,abs_tol=6)
        shutil.copyfile(spline_profile,output_dir/"spline-boundary-authoring.pcad")
        shutil.copyfile(spline_stl,output_dir/"spline-boundary-authoring.stl")
        key("Escape")
        app.wait(3)

        arc_path=Path(tmp)/"Circular arc editing.pcad"
        arc_doc=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        arc_graph=arc_doc["items"][0]["data"]["cad"]
        sketch_nodes=arc_graph["nodes"][:4]
        arc_graph["nodes"]=sketch_nodes+[
            {"id":"native-entity-1","type":"sketchArc","sketchId":"native-sketch-1",
             "arc":{"id":"native-entity-1","type":"arc","center":{"x":0,"y":0},"radius":20,
                    "startAngle":0,"sweep":math.pi/2,"segments":128}},
            {"id":"native-entity-2","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":-10,"y":14},"p1":{"x":-10,"y":20}},
            {"id":"native-entity-3","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":10,"y":14},"p1":{"x":10,"y":20}},
        ]
        arc_path.write_text(json.dumps(arc_doc))

        def saved_arc(path, start, sweep):
            nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
            node=next(row for row in nodes if row["id"]=="native-entity-1")
            assert node["type"]=="sketchArc" and node["sketchId"]=="native-sketch-1"
            arc=node["arc"]
            assert arc["id"]==node["id"] and arc["center"]=={"x":0,"y":0} and arc["radius"]==20
            assert math.isclose(arc["startAngle"],start,abs_tol=2e-8)
            assert math.isclose(arc["sweep"],sweep,abs_tol=2e-8)
            return arc

        def rust_sketch_export(path):
            reference=path.with_name(path.stem+"-rust.stl")
            subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
            return triangulation(reference)

        app,window=open_app(arc_path,"Click two rectangle corners")
        key("Shift_L+q")
        click(window,501,281)
        verify(window,"Arc extended")
        key("Control_L+s")
        verify(window,"Saved")
        saved_arc(arc_path,0,2*math.pi/3)
        capture(window,"sketch-extend-arc.png",True)
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(arc_path,"Click two rectangle corners")
        key("q")
        click(window,466,279)
        verify(window,"Arc trimmed")
        key("Control_L+s")
        verify(window,"Saved")
        saved_arc(arc_path,0,math.pi/3)
        assert saved_line_endpoints(arc_path)=={"native-entity-2":((-10,14),(-10,20)),
                                                "native-entity-3":((10,14),(10,20))}
        key("Escape")
        key("Escape")
        key("Escape")
        app.wait(3)

        circle_profile=Path(tmp)/"Trimmed circle profile.pcad"
        arc_graph["nodes"]=sketch_nodes+[
            {"id":"native-entity-1","type":"sketchPrimitive","sketchId":"native-sketch-1",
             "primitive":{"type":"circle","center":{"x":0,"y":0},"radius":20,"segments":128}},
            {"id":"native-entity-2","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":-20,"y":0},"p1":{"x":20,"y":0}},
        ]
        circle_profile.write_text(json.dumps(arc_doc))
        app,window=open_app(circle_profile,"Click two rectangle corners")
        key("q")
        click(window,480,278)
        verify(window,"Circle trimmed")
        capture(window,"sketch-trim-circle-target.png",True)
        key("Escape")
        verify(window,"Curve edit cancelled")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        circle_arc=saved_arc(circle_profile,math.pi,math.pi)
        key("Escape")
        app.wait(3)
        app,window=open_app(circle_profile)
        key("Control_L+e")
        verify(window,"Exported")
        exported=circle_profile.with_suffix(".stl")
        bounds,volume=triangulation(exported)
        assert bounds==[(-20,20),(-20,0),(0,10)]
        assert math.isclose(volume,64*2000*math.sin(math.pi/64),abs_tol=.02)
        reference_bounds,reference_volume=rust_sketch_export(circle_profile)
        # Rust rounds the saved picoradian sweep up to its next sample count.
        reference_count=math.ceil(circle_arc["sweep"]/math.tau*circle_arc["segments"])
        assert all(math.isclose(a,b,abs_tol=.01) for interval,expected in zip(reference_bounds,bounds,strict=True)
                   for a,b in zip(interval,expected,strict=True)), (reference_bounds,bounds)
        assert math.isclose(reference_volume,reference_count*2000*math.sin(math.pi/reference_count),abs_tol=.02)
        shutil.copyfile(circle_profile,output_dir/"circle-trim-authoring.pcad")
        shutil.copyfile(exported,output_dir/"circle-trim-authoring.stl")
        key("Escape")
        app.wait(3)

        ellipse_path=Path(tmp)/"Elliptic curve editing.pcad"
        ellipse={"center":{"x":0,"y":0},"width":40,"height":20,"rotation":0,"segments":128}
        elliptic_node={"id":"native-entity-1","type":"sketchPrimitive","sketchId":"native-sketch-1",
                       "primitive":{**ellipse,"type":"ellipticArc","startAngle":0,"sweep":math.pi/2}}
        arc_graph["nodes"]=sketch_nodes+[elliptic_node]+[
            {"id":f"native-entity-{i}","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":x,"y":7},"p1":{"x":x,"y":10}}
            for i,x in enumerate((-10,10),2)
        ]
        ellipse_path.write_text(json.dumps(arc_doc))

        def saved_nodes(path):
            return json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]

        def saved_elliptic_arc(path,start,sweep):
            node=next(row for row in saved_nodes(path) if row["id"]=="native-entity-1")
            primitive=node["primitive"]
            assert node["type"]=="sketchPrimitive" and node["sketchId"]=="native-sketch-1"
            assert primitive["type"]=="ellipticArc" and all(primitive[key]==value for key,value in ellipse.items())
            assert math.isclose(primitive["startAngle"],start,abs_tol=2e-8)
            assert math.isclose(primitive["sweep"],sweep,abs_tol=2e-8)

        app,window=open_app(ellipse_path,"Click two rectangle corners")
        key("Shift_L+q")
        click(window,501,319)
        verify(window,"Elliptic arc extended")
        key("Control_L+s")
        verify(window,"Saved")
        saved_elliptic_arc(ellipse_path,0,2*math.pi/3)
        capture(window,"sketch-extend-elliptic-arc.png",True)
        key("Escape")
        verify(window,"Curve edit cancelled")
        key("Escape")
        app.wait(3)

        app,window=open_app(ellipse_path,"Click two rectangle corners")
        key("q")
        click(window,480,318)
        verify(window,"Elliptic arc trimmed")
        key("Control_L+s")
        verify(window,"Saved")
        saved_elliptic_arc(ellipse_path,0,math.pi/3)
        assert saved_line_endpoints(ellipse_path)=={"native-entity-2":((-10,7),(-10,10)),
                                                    "native-entity-3":((10,7),(10,10))}
        key("Escape")
        verify(window,"Curve edit cancelled")
        key("Escape")
        app.wait(3)

        ellipse_profile=Path(tmp)/"Trimmed ellipse profile.pcad"
        elliptic_node["primitive"]={**ellipse,"type":"ellipse"}
        arc_graph["nodes"]=sketch_nodes+[elliptic_node,
            {"id":"native-entity-2","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":-20,"y":0},"p1":{"x":20,"y":0}}]
        ellipse_profile.write_text(json.dumps(arc_doc))
        app,window=open_app(ellipse_profile,"Click two rectangle corners")
        key("q")
        click(window,480,318)
        verify(window,"Ellipse trimmed")
        capture(window,"sketch-trim-ellipse-target.png",True)
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        saved_elliptic_arc(ellipse_profile,math.pi,math.pi)
        key("Escape")
        app.wait(3)
        app,window=open_app(ellipse_profile)
        key("Control_L+e")
        verify(window,"Exported")
        exported=ellipse_profile.with_suffix(".stl")
        bounds,volume=triangulation(exported)
        assert bounds==[(-20,20),(-10,0),(0,10)]
        assert math.isclose(volume,64*1000*math.sin(math.pi/64),abs_tol=.02)
        reference_bounds,reference_volume=rust_sketch_export(ellipse_profile)
        assert all(math.isclose(a,b,abs_tol=.01) for pair,other in zip(reference_bounds,bounds,strict=True)
                   for a,b in zip(pair,other,strict=True)), (reference_bounds,bounds)
        assert math.isclose(reference_volume,volume,abs_tol=.05), (reference_volume,volume)
        shutil.copyfile(ellipse_profile,output_dir/"ellipse-trim-authoring.pcad")
        shutil.copyfile(exported,output_dir/"ellipse-trim-authoring.stl")
        key("Escape")
        app.wait(3)

        spline_target=Path(tmp)/"Spline target trimming.pcad"
        authored_spline={"id":"native-entity-1","type":"sketchSpline","sketchId":"native-sketch-1",
                         "spline":{"id":"native-entity-1","type":"spline","mode":"control","closed":False,
                                   "points":[{"x":x,"y":y} for x,y in [(0,0),(10,20),(20,20),(30,0)]]}}
        arc_graph["nodes"]=sketch_nodes+[authored_spline]+[
            {"id":f"native-entity-{i}","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":x,"y":8},"p1":{"x":x,"y":12}}
            for i,x in enumerate((6,24),2)
        ]
        spline_target.write_text(json.dumps(arc_doc))

        def saved_spline_pieces(path):
            result={}
            for node in saved_nodes(path):
                if node["type"]!="sketchSpline":
                    continue
                spline=node["spline"]
                assert spline["id"]==node["id"] and spline["mode"]=="control" and not spline["closed"]
                result[node["id"]]=[(point["x"],point["y"]) for point in spline["points"]]
            return result

        tied_path=Path(tmp)/"Line on spline tangent.pcad"
        tied_doc=json.loads(spline_target.read_bytes())
        tied_nodes=tied_doc["items"][0]["data"]["cad"]["nodes"]
        tied_nodes[5].update(p0={"x":15,"y":15},p1={"x":20,"y":15})
        tied_nodes[6].update(p0={"x":8,"y":13},p1={"x":8,"y":17})
        tied_path.write_text(json.dumps(tied_doc))
        app,window=open_app(tied_path,"Click two rectangle corners")
        key("Shift_L+q")
        click(window,540,298)  # The spline and line share the pixel; only the line can extend.
        verify(window,"Line extended")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_line_endpoints(tied_path)["native-entity-2"]==((8,15),(20,15))
        assert next(node for node in saved_nodes(tied_path) if node["id"]=="native-entity-1")==authored_spline
        key("Escape")
        key("Escape")
        app.wait(3)

        app,window=open_app(spline_target,"Click two rectangle corners")
        key("q")
        click(window,540,298)
        verify(window,"Spline trimmed")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_spline_pieces(spline_target)=={
            "native-entity-1":[(0,0),(2,4),(4,7.2),(6,9.6)],
            "native-entity-4":[(24,9.6),(26,7.2),(28,4),(30,0)],
        }
        assert saved_line_endpoints(spline_target)=={"native-entity-2":((6,8),(6,12)),
                                                      "native-entity-3":((24,8),(24,12))}
        capture(window,"sketch-trim-spline-target.png",True)
        key("Escape")
        key("Escape")
        app.wait(3)

        periodic_path=Path(tmp)/"Opened periodic spline.pcad"
        periodic={**authored_spline["spline"],"mode":"fit","closed":True,
                  "points":[{"x":x,"y":y} for x,y in [(-20,0),(0,20),(20,0),(0,-20)]]}
        arc_graph["nodes"]=sketch_nodes+[{**authored_spline,"spline":periodic}]
        periodic_path.write_text(json.dumps(arc_doc))
        app,window=open_app(periodic_path,"Click two rectangle corners")
        key("Shift_L+x")
        click(window,400,358)
        verify(window,"Spline split")
        key("Control_L+s")
        verify(window,"Saved")
        pieces=saved_spline_pieces(periodic_path)
        assert list(pieces)==["native-entity-1"]
        opened=pieces["native-entity-1"]
        assert len(opened)==13 and opened[0]==opened[-1]==(-20,0)
        assert opened[3::3]==[(0,20),(20,0),(0,-20),(-20,0)]
        key("Escape")
        app.wait(3)

        split_profile=Path(tmp)/"Split spline profile.pcad"
        arc_graph["nodes"]=sketch_nodes+[authored_spline,
            {"id":"native-entity-2","type":"sketchLine","sketchId":"native-sketch-1",
             "p0":{"x":0,"y":0},"p1":{"x":30,"y":0}}]
        split_profile.write_text(json.dumps(arc_doc))
        app,window=open_app(split_profile,"Click two rectangle corners")
        key("Shift_L+x")
        click(window,540,298)
        verify(window,"Spline split")
        key("Control_L+s")
        verify(window,"Saved")
        divided={"native-entity-1":[(0,0),(5,10),(10,15),(15,15)],
                 "native-entity-3":[(15,15),(20,15),(25,10),(30,0)]}
        assert saved_spline_pieces(split_profile)==divided
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_spline_pieces(split_profile)=={"native-entity-1":[(0,0),(10,20),(20,20),(30,0)]}
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        assert saved_spline_pieces(split_profile)==divided
        capture(window,"sketch-split-spline-target.png",True)
        key("Escape")
        app.wait(3)
        app,window=open_app(split_profile,"Click two rectangle corners")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)
        app,window=open_app(split_profile)
        key("Control_L+e")
        verify(window,"Exported")
        exported=split_profile.with_suffix(".stl")
        bounds,volume=triangulation(exported)
        assert bounds==[(0,30),(0,15),(0,10)] and math.isclose(volume,3000,abs_tol=6)
        reference_bounds,reference_volume=rust_sketch_export(split_profile)
        assert reference_bounds==bounds and math.isclose(reference_volume,volume,abs_tol=.02)
        shutil.copyfile(split_profile,output_dir/"spline-split-authoring.pcad")
        shutil.copyfile(exported,output_dir/"spline-split-authoring.stl")
        key("Escape")
        app.wait(3)

        # The first launch shows the authored tree before the sketch has a solid.
        path=Path(tmp)/"Design tree.pcad"
        app,window=open_app(path,"Click two rectangle corners",design_tree=True)
        click(window,64,212,button=3)  # Sketch 1
        verify(window,"Selected Sketch 1")
        frame=output_dir/"design-context-menu.png"
        subprocess.run(["magick","import","-window",hex(window),frame],check=True)
        with Image.open(frame) as image:
            assert image.convert("RGB").getpixel((70,214)) == (89,111,109)
        key("Escape")
        dismissed=Path(tmp)/"dismissed-menu.png"
        subprocess.run(["magick","import","-window",hex(window),dismissed],check=True)
        with Image.open(dismissed) as image:
            assert image.convert("RGB").getpixel((70,214)) != (89,111,109)
        click(window,64,212,button=3)
        click(window,84,227)  # Edit sketch
        verify(window,"Editing Sketch 1")
        click(window,400,398)
        click(window,560,318)
        click(window,64,212,button=3)
        click(window,84,253)  # Expand
        click(window,110,238)
        verify(window,"Selected Corner rectangle")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"design-sketch-tree.png"],check=True)
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        assert part_sketch_pixels(Path(tmp)/"consumed-default.png")==0
        click(window,232,212)
        verify(window,"Shown Sketch 1")
        key("6")  # Bottom view looks directly at the consumed sketch's plane.
        verify(window,"Bottom view")
        assert part_sketch_pixels("part-consumed-sketch.png")>100
        click(window,64,212,button=3)
        click(window,84,279)  # Hide sketch
        verify(window,"Hidden Sketch 1")
        assert part_sketch_pixels(Path(tmp)/"consumed-hidden.png")==0
        click(window,64,212,button=3)
        verify(window,"Selected Sketch 1")
        assert "|  Solid" in title(window)
        click(window,84,227)  # Re-enter Sketch from the reopened Part.
        verify(window,"Editing Sketch 1")
        key("F12")
        verify(window,"Part workspace")
        click(window,55,125)
        for letter in "extrude":
            key(letter)
        key("Return")
        click(window,75,185)
        verify(window,"Selected Extrusion 1")
        key("Return")  # Feature history
        verify(window,"Extrusion 1   active")
        key("Escape")
        verify(window,"Feature history closed")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)

        # Two parts deliberately share their local sketch and entity IDs.
        # Search can reach the other part directly and editing must stay in that part.
        twins=Path(tmp)/"Two parts.pcad"
        document=json.loads(path.read_bytes())
        second=json.loads(json.dumps(document["items"][0]))
        second.update(id="second",name="Cover")
        for n in second["data"]["cad"]["nodes"]:
            if n["type"]=="sketch":
                n["name"]="Cover outline"
        document["items"].append(second)
        twins.write_text(json.dumps(document))
        app,window=open_app(twins,design_tree=True)
        click(window,55,125)
        for letter in "outline":
            key(letter)
        key("Return")
        key("Down")
        verify(window,"Selected Cover")
        key("Right")
        verify(window,"Selected Cover outline")
        key("Return")
        verify(window,"Editing Cover outline")
        key("period")
        click(window,490,345)
        verify(window,"Point added")
        key("Control_L+s")
        verify(window,"Saved")
        edited=json.loads(twins.read_bytes())
        assert edited["selectedPath"]==[1] and edited["items"][0]==document["items"][0]
        assert len(edited["items"][1]["data"]["cad"]["nodes"])==len(second["data"]["cad"]["nodes"])+1
        key("Escape")
        app.wait(3)

        shell_path=Path(tmp)/"Shell authoring.pcad"
        stock={"format":"puppycorp.pcad","schema_version":1,"units":"mm",
               "nodes":[{"id":"stock","type":"box","size":[24,16,9]}],
               "parts":[{"id":"enclosure","name":"Enclosure","node":"stock"}],"feature_order":["stock"]}
        shell_path.write_text(json.dumps(stock))

        def select_shell_menu(row_y):
            click(window,85,row_y,button=3)
            click(window,109,row_y+15)

        app,window=open_app(shell_path,"PCAD design opened",design_tree=True)
        select_shell_menu(186)
        verify(window,"Shell part / Enclosure / +Z / 2 mm")
        key("t")
        verify(window,"Wall thickness")
        type_value("32")
        key("Return")
        key("e")
        verify(window,"Shell thickness leaves no cavity")
        assert json.loads(shell_path.read_bytes())==stock
        key("Escape")
        verify(window,"Shell cancelled")
        select_shell_menu(186)
        verify(window,"Shell part / Enclosure / +Z / 2 mm")
        key("e")
        verify(window,"Shell preview ready / +Z / 2 mm")
        capture(window,"shell-part-preview.png",False,surface=(76,167,192))
        assert json.loads(shell_path.read_bytes())==stock
        key("Return")
        verify(window,"Shell applied")
        for stroke in ("Control_L+z","Control_L+y"):
            key(stroke)
            key("Control_L+s")
            verify(window,"Saved")
            saved_shell=json.loads(shell_path.read_bytes())
            if stroke=="Control_L+z":
                assert saved_shell==stock
        shell=saved_shell["nodes"][1]
        assert shell=={"id":"shell-1","type":"shell","source":"stock","open_side":"z","thickness":2}
        assert saved_shell["parts"]==[{**stock["parts"][0],"node":shell["id"]}]
        assert saved_shell["feature_order"]==["stock",shell["id"]]
        key("F2")
        verify(window,"Set value")
        type_value("30")
        key("Return")
        verify(window,"stock / x")
        click(window,26,186)  # Expand Enclosure and expose its saved shell feature.
        select_shell_menu(212)
        verify(window,"Edit shell / shell-1 / +Z / 2 mm")
        key("4")
        key("t")
        verify(window,"Wall thickness")
        type_value("1.5")
        key("Return")
        key("e")
        verify(window,"Shell preview ready / −X / 1.5 mm")
        key("Return")
        verify(window,"Shell updated")
        key("Control_L+s")
        verify(window,"Saved")
        final_shell=json.loads(shell_path.read_bytes())
        assert final_shell["nodes"]==[{**stock["nodes"][0],"size":[30,16,9]},
                                     {**shell,"open_side":"-x","thickness":1.5}]
        assert final_shell["parts"]==saved_shell["parts"] and final_shell["feature_order"]==saved_shell["feature_order"]
        key("Escape")
        app.wait(3)
        app,window=open_app(shell_path,"PCAD design opened")
        key("5")
        verify(window,"Left view")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"shell-part-reopened.png"],check=True)
        key("Control_L+e")
        verify(window,"Exported")
        shell_stl=shell_path.with_suffix(".stl")
        bounds,volume=triangulation(shell_stl)
        assert bounds==[(0,30),(0,16),(0,9)] and math.isclose(volume,2097,abs_tol=.02)
        shutil.copyfile(shell_path,output_dir/"shell-authoring.pcad")
        shutil.copyfile(shell_stl,output_dir/"shell-authoring.stl")
        key("Escape")
        app.wait(3)

        # Edit the original TypeScript PartBuilder records through native History.
        legacy_path=Path(tmp)/"Legacy PartBuilder.pcad"
        initial=(root/"tests/fixtures/legacy-partbuilder.pcad").read_bytes()
        legacy_path.write_bytes(initial)
        app,window=open_app(legacy_path,design_tree=True)
        click(window,89,212)  # The second solid step in Design.
        key("Return")
        verify(window,"2 / 2   Intersect window   active   Depth  mm  8")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"legacy-partbuilder-history.png"],check=True)

        def legacy_number(text):
            key("F2")
            verify(window,"Set value")
            key("Control_L+a")
            type_value(text)
            key("Return")

        legacy_number("6")
        verify(window,"Depth  mm  6")
        key("Tab")
        verify(window,"Top scale  1")
        key("Tab")
        verify(window,"Translate X  mm  8")
        legacy_number("240")
        verify(window,"Legacy solid operations produced an empty part")
        key("Escape")
        assert legacy_path.read_bytes()==initial
        legacy_number("6")
        verify(window,"Translate X  mm  6")
        key("Up")
        verify(window,"The first legacy solid operation must join")
        key("Control_L+z")
        verify(window,"Translate X  mm  8")
        key("Control_L+y")
        verify(window,"Translate X  mm  6")
        key("Control_L+s")
        verify(window,"Saved")
        expected=json.loads(initial)
        expected["revision"]+=1
        data=expected["items"][0]["data"]
        data["solidSteps"][1]["translation"]["x"]=6
        data["features"][-1]["depth"]=6
        assert json.loads(legacy_path.read_bytes())==expected
        shutil.copyfile(legacy_path,output_dir/"legacy-partbuilder-edited.pcad")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)
        app,window=open_app(legacy_path)
        key("Control_L+e")
        verify(window,"Exported")
        assert triangulation(legacy_path.with_suffix(".stl"))==([(0,10),(-6,6),(2,8)],720)
        shutil.copyfile(legacy_path.with_suffix(".stl"),output_dir/"legacy-partbuilder-edited.stl")
        key("Escape")
        app.wait(3)

        # Legacy shell and loft parameters keep their source reference after editing.
        oracle=root/"tests/fixtures/legacy-flared-shell.pcad"
        path=Path(tmp)/"Legacy loft shell.pcad"
        original=oracle.read_bytes()
        path.write_bytes(original)
        app,window=open_app(path,design_tree=True)
        click(window,89,212)
        key("Return")
        verify(window,"Wall thickness  mm  2.5")
        legacy_number("18")
        verify(window,"Shell thickness consumes the loft interior")
        key("Escape")
        assert path.read_bytes()==original
        legacy_number("3.25")
        verify(window,"Wall thickness  mm  3.25")
        key("Control_L+z")
        verify(window,"Wall thickness  mm  2.5")
        key("Control_L+y")
        verify(window,"Wall thickness  mm  3.25")
        key("Left")
        verify(window,"Bottom radius  mm  12")
        key("Tab")
        verify(window,"Top radius  mm  18")
        key("Tab")
        verify(window,"Height  mm  30")
        legacy_number("32")
        verify(window,"Height  mm  32")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"legacy-loft-edited.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        expected=json.loads(original)
        expected["revision"]+=1
        steps=expected["items"][0]["data"]["solidSteps"]
        steps[0]["loft"]["height"]=32
        steps[1]["thickness"]=3.25
        assert json.loads(path.read_bytes())==expected
        shutil.copyfile(path,output_dir/"legacy-loft-edited.pcad")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+e")
        verify(window,"Exported")
        bounds,volume=triangulation(path.with_suffix(".stl"))
        assert bounds==[(-14,22),(-25,11),(3,35)] and math.isclose(volume,9638.160617,abs_tol=.003)
        shutil.copyfile(path.with_suffix(".stl"),output_dir/"legacy-loft-edited.stl")
        key("Escape")
        app.wait(3)

        # Edit a saved selected corner without changing the TypeScript loop or index.
        path=Path(tmp)/"Legacy selected corner.pcad"
        oracle=libs/"cad/tests/fixtures/legacy-typescript-selected-corner-chamfer.pcad"
        original=oracle.read_bytes()
        path.write_bytes(original)
        app,window=open_app(path,design_tree=True)
        key("F7")
        verify(window,"Depth  mm  8")
        for _ in range(5):
            key("Tab")
        verify(window,"Chamfer 1 distance  mm  3")
        legacy_number("22")
        verify(window,"Finish radius is too large for adjacent profile edges")
        key("Escape")
        assert path.read_bytes()==original
        legacy_number("4")
        verify(window,"Chamfer 1 distance  mm  4")
        key("Control_L+z")
        verify(window,"Chamfer 1 distance  mm  3")
        key("Control_L+y")
        verify(window,"Chamfer 1 distance  mm  4")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"legacy-selected-corner.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        expected=json.loads(original)
        expected["revision"]+=1
        finish=expected["items"][0]["data"]["solidSteps"][0]["finishes"][0]
        assert finish["vertices"]==[3]
        finish["radius"]=4
        assert json.loads(path.read_bytes())==expected
        shutil.copyfile(path,output_dir/"legacy-selected-corner-edited.pcad")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        bounds,volume=triangulation(stl)
        assert bounds==[(-10,10),(-6,6),(0,8)] and math.isclose(volume,1856,abs_tol=.001)
        from struct import iter_unpack
        vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
        assert {(6,6,8),(10,2,8)}<=vertices and (10,6,8) not in vertices
        shutil.copyfile(stl,output_dir/"legacy-selected-corner-edited.stl")
        key("Escape")
        app.wait(3)

        # Saved topology finishes keep their original edge ID and record family.
        edge_cases=(
            ("feature-chamfer","Saved chamfer 1 d1  mm","Saved chamfer 1 d2  mm",2,3,3.25,1.75,3943.125),
            ("topology-fillet","Edge fillet 1 radius  mm","Edge fillet 1 segments",2,32,3,16,3961.485339),
        )
        for kind,first_label,second_label,initial_first,initial_second,first,second,expected_volume in edge_cases:
            artifact=f"legacy-{kind}"
            path=Path(tmp)/f"{artifact}.pcad"
            initial=(libs/"cad/tests/fixtures"/f"legacy-typescript-{kind}.pcad").read_bytes()
            path.write_bytes(initial)
            shutil.copyfile(path,output_dir/f"{artifact}-input.pcad")
            app,window=open_app(path,design_tree=True)
            key("F7")
            verify(window,"Depth  mm  10")
            for _ in range(5):
                key("Tab")
            verify(window,f"{first_label}  {initial_first}")
            legacy_number(str(first))
            verify(window,f"{first_label}  {first}")
            key("Tab")
            verify(window,f"{second_label}  {initial_second}")
            if kind=="topology-fillet":
                legacy_number("65")
                verify(window,"Edge fillet segments must be between 2 and 64")
                key("Escape")
                assert path.read_bytes()==initial
            legacy_number(str(second))
            verify(window,f"{second_label}  {second}")
            key("Control_L+z")
            verify(window,f"{second_label}  {initial_second}")
            key("Control_L+y")
            verify(window,f"{second_label}  {second}")
            subprocess.run(["magick","import","-window",hex(window),output_dir/f"{artifact}.png"],check=True)
            key("Control_L+s")
            verify(window,"Saved")
            expected=json.loads(initial)
            expected["revision"]+=1
            part=expected["items"][0]["data"]
            if kind=="feature-chamfer":
                chamfer=next(row for row in part["features"] if row["type"]=="chamfer")
                chamfer.update(d1=first,d2=second)
            else:
                part["solidSteps"][0]["edgeFinishes"][0].update(radius=first,segments=second)
            assert json.loads(path.read_bytes())==expected
            shutil.copyfile(path,output_dir/f"{artifact}-edited.pcad")
            key("Escape")
            verify(window,"Feature history closed")
            key("Escape")
            app.wait(3)
            app,window=open_app(path)
            key("Control_L+e")
            verify(window,"Exported")
            stl=path.with_suffix(".stl");bounds,volume=triangulation(stl)
            assert bounds==[(-10,10),(-10,10),(0,10)] and math.isclose(volume,expected_volume,abs_tol=.002),(bounds,volume)
            shutil.copyfile(stl,output_dir/f"{artifact}-edited.stl")
            key("Escape")
            app.wait(3)

        # The extra filled regions share the extrusion tool's extent and preserve its face IDs.
        path=Path(tmp)/"Legacy regions.pcad"
        fixture=json.loads((libs/"cad/tests/fixtures/legacy-typescript-filled-regions.pcad").read_bytes())
        original=dict(fixture,items=[next(item for item in fixture["items"] if item["id"]=="regions-face-attachment")],selectedPath=[0])
        path.write_text(json.dumps(original))
        shutil.copyfile(path,output_dir/"legacy-filled-region-input.pcad")
        app,window=open_app(path,design_tree=True)
        key("F7")
        verify(window,"Depth  mm  8")
        legacy_number("10")
        verify(window,"Depth  mm  10")
        key("Tab")
        key("Tab")
        verify(window,"Region 2 center X  mm  28")
        legacy_number("32")
        verify(window,"Region 2 center X  mm  32")
        key("Control_L+z")
        verify(window,"Region 2 center X  mm  28")
        key("Control_L+y")
        verify(window,"Region 2 center X  mm  32")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"legacy-filled-region.png"],check=True)
        key("Control_L+s")
        verify(window,"Saved")
        expected=json.loads(json.dumps(original))
        expected["revision"]+=1
        data=expected["items"][0]["data"]
        data["features"][1]["depth"]=10
        data["solidSteps"][0]["regions"][0]["center"]["x"]=32
        assert data["features"][2]["target"]["face"]["faceId"]=="base-solid-face-6"
        assert json.loads(path.read_bytes())==expected
        shutil.copyfile(path,output_dir/"legacy-filled-region-edited.pcad")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        assert triangulation(stl)==([(-8,37),(-2,8),(4,17)],2384)
        shutil.copyfile(stl,output_dir/"legacy-filled-region-edited.stl")
        key("Escape")
        app.wait(3)

        # Edit a TypeScript PartBuilder sketch and rebuild its original extrusion.
        path=Path(tmp)/"Legacy editable sketch.pcad"
        initial=(libs/"cad/tests/fixtures/legacy-typescript-editable-sketch.pcad").read_bytes()
        path.write_bytes(initial)
        shutil.copyfile(path,output_dir/"legacy-sketch-input.pcad")
        app,window=open_app(path,design_tree=True)
        click(window,94,212)
        verify(window,"Editing Editable profile")
        capture(window,"legacy-sketch-open.png",sketch=True)
        key("F8")
        verify(window,"plate/sketch/0/0   20 mm   DOF")
        legacy_number("4")
        verify(window,"Sketch edit changes a profile used by plate")
        key("Escape")
        assert path.read_bytes()==initial
        legacy_number("24")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"plate/sketch/0/0   20 mm   DOF")
        key("Control_L+y")
        verify(window,"plate/sketch/0/0   24 mm   DOF")
        key("Control_L+s")
        verify(window,"Saved")
        original=json.loads(initial)["items"][0]["data"]
        saved=json.loads(path.read_bytes())["items"][0]["data"]
        before,sketch=original["features"][0],saved["features"][0]
        assert "cad" not in saved and original["solidSteps"]==saved["solidSteps"]
        assert saved["features"][1:]==original["features"][1:] and sketch["target"]==before["target"]
        assert [row["id"] for row in sketch["entities"]]==[row["id"] for row in before["entities"]]
        assert sketch["relations"]==[dict(row,value=24) if row["id"]=="width-drive" else row for row in before["relations"]]
        assert sketch["profiles"]==before["profiles"] and sketch["loops"]==before["loops"]
        assert math.isclose(max(point["x"] for point in sketch["vertices"]),14,abs_tol=.000002)
        shutil.copyfile(path,output_dir/"legacy-sketch-edited.pcad")
        part_link,=breadcrumb_links(window,"legacy-sketch-edited.png")
        click(window,part_link,20)
        verify(window,"Part workspace")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl");bounds,volume=triangulation(stl)
        assert bounds==[(-10,14),(-6,6),(0,8)] and math.isclose(volume,2206.03304,abs_tol=.002),(bounds,volume)
        shutil.copyfile(stl,output_dir/"legacy-sketch-edited.stl")
        key("Escape")
        app.wait(3)

        # A PartBuilder sketch without any solid can open and save its own editing state.
        path=Path(tmp)/"Legacy sketch only.pcad"
        initial=(libs/"cad/tests/fixtures/legacy-typescript-sketch-only.pcad").read_bytes()
        path.write_bytes(initial)
        app,window=open_app(path,"Click two rectangle corners")
        key("Shift_L+period")
        verify(window,"Click to place a construction point")
        click(window,635,411)
        verify(window,"Point added")
        key("Control_L+s")
        verify(window,"Saved")
        original=json.loads(initial)["items"][0]["data"]
        saved=json.loads(path.read_bytes())["items"][0]["data"]
        before,sketch=original["features"][0],saved["features"][0]
        assert "cad" not in saved and len(saved["features"])==1
        assert sketch["profiles"]==before["profiles"] and sketch["entities"][:-1]==before["entities"]
        added=sketch["entities"][-1]
        assert added["id"]=="native-entity-1" and added["type"]=="point" and added["construction"] is True
        key("Escape")
        app.wait(3)

        # The original Biscuit standalone feature-history plate, with a
        # through cut and a dependent transform of the result.
        history_path=Path(tmp)/"Standalone feature history.pcad"
        history_doc={"format":"puppycorp.pcad","schema_version":1,"units":"mm",
                     "nodes":[{"id":"stock","type":"box","size":[20,10,6]},
                              {"id":"punch","type":"box","size":[3,3,10],"position":[5,4,-2]},
                              {"id":"cut","type":"subtract","a":"stock","b":"punch"},
                              {"id":"finish","type":"transform","source":"cut","position":[2,0,0]}],
                     "parts":[{"id":"plate","name":"Cut plate","node":"finish"}]}
        original=(json.dumps(history_doc,indent=2)+"\n").encode()
        history_path.write_bytes(original)
        app,window=open_app(history_path,"PCAD design opened",design_tree=True)

        def history_frame(name):
            from PIL import Image
            frame=output_dir/f"standalone-history-{name}.png"
            subprocess.run(["magick","import","-window",hex(window),frame],check=True)
            with Image.open(frame) as image:
                return image.convert("RGB").crop((289,80,image.width,image.height)).tobytes()

        def history_export(name,volume):
            key("Control_L+e")
            verify(window,"Exported")
            mesh=history_path.with_suffix(".stl")
            assert triangulation(mesh)==([(2,22),(0,10),(0,6)],volume)
            shutil.copyfile(history_path,output_dir/f"standalone-history-{name}.pcad")
            shutil.copyfile(mesh,output_dir/f"standalone-history-{name}.stl")
            mesh.unlink()

        click(window,645,21)  # History toolbar button.
        verify(window,"finish   active")
        key("Escape")
        verify(window,"Feature history closed")
        click(window,19,212)  # Features group.
        click(window,96,290,button=3)
        verify(window,"Selected cut")
        click(window,120,306)  # Feature history context action.
        verify(window,"cut   active")
        active_frame=history_frame("active")
        history_export("authored",1146)
        key("s")
        verify(window,"Suppress preview / cut")
        assert active_frame!=history_frame("suppress-preview")
        key("Control_L+s")
        verify(window,"Saved")
        assert history_path.read_bytes()==original
        history_export("while-previewing",1146)
        click(window,170,54)  # Cancel button.
        verify(window,"Feature preview cancelled")
        assert history_frame("cancelled")==active_frame
        key("s")
        verify(window,"Suppress preview / cut")
        click(window,78,54)  # Apply button.
        verify(window,"cut   suppressed")
        key("Control_L+s")
        verify(window,"Saved")
        suppressed=json.loads(history_path.read_bytes())
        assert suppressed=={**history_doc,"nodes":[*history_doc["nodes"][:2],
                                                    {**history_doc["nodes"][2],"suppressed":True},history_doc["nodes"][3]]}
        history_export("suppressed",1200)
        key("Control_L+z")
        verify(window,"cut   active")
        key("Control_L+y")
        verify(window,"cut   suppressed")
        key("s")
        verify(window,"Restore preview / cut")
        key("Return")
        verify(window,"cut   active")
        key("Up")
        verify(window,"Node references must name an earlier node in the design: punch")
        key("Left")
        verify(window,"punch   active")
        key("Up")
        verify(window,"Move earlier preview / punch")
        key("Return")
        verify(window,"1 / 4   punch   active")
        key("Control_L+s")
        verify(window,"Saved")
        reordered={**history_doc,"feature_order":["punch","stock","cut","finish"]}
        assert json.loads(history_path.read_bytes())==reordered
        history_export("reordered",1146)
        key("Right")
        verify(window,"stock   active")
        key("a")
        verify(window,"Earlier preview / stock")
        assert active_frame!=history_frame("earlier-preview")
        key("Return")  # An earlier-state preview has no Apply operation.
        verify(window,"Earlier preview / stock")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(history_path.read_bytes())==reordered
        key("Escape")
        verify(window,"Feature preview cancelled")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)
        app,window=open_app(history_path,"PCAD design opened")
        key("F7")
        verify(window,"4 / 4   finish   active")
        history_export("reopened",1146)
        key("Escape")
        verify(window,"Feature history closed")
        xlib.XResizeWindow(connection,window,420,460)
        xlib.XSync(connection,0)
        time.sleep(.22)
        click(window,20,55)  # Wrap to the final toolbar page.
        subprocess.run(["magick","import","-window",hex(window),output_dir/"standalone-history-compact.png"],check=True)
        click(window,206,55)
        verify(window,"4 / 4   finish   active")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)

        multi=Path(tmp)/"Standalone history with two parts.pcad"
        second_part={"id":"spacer","name":"Spacer","node":"spacer-stock"}
        multi.write_text(json.dumps({**history_doc,
            "nodes":[*history_doc["nodes"],{"id":"spacer-stock","type":"box","size":[2,4,6],"position":[26,0,0]}],
            "parts":[*history_doc["parts"],second_part]}))
        app,window=open_app(multi,"PCAD design opened",design_tree=True)
        click(window,88,186,button=3)  # The Cut plate part definition.
        verify(window,"Selected Cut plate")
        key("Down")  # Feature history follows the existing Shell part action.
        key("Return")
        verify(window,"4 / 5   finish   active")
        key("Left")
        verify(window,"cut   active")
        key("a")
        verify(window,"Earlier preview / cut")
        key("Escape")
        verify(window,"Feature preview cancelled")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)

        damaged=Path(tmp)/"Recover standalone feature.pcad"
        broken={**history_doc,"nodes":[*history_doc["nodes"][:-1],
                                       {**history_doc["nodes"][-1],"position":[501,0,0]}]}
        source=(json.dumps(broken,indent=2)+"\n").encode()
        damaged.write_bytes(source)
        app,window=open_app(damaged,"finish   error",design_tree=True)
        verify(window,"CAD dimensions and coordinates must be within +/-500 mm")
        key("Control_L+s")
        verify(window,"Apply a repair before saving or exporting")
        key("Control_L+e")
        verify(window,"Apply a repair before saving or exporting")
        mesh=damaged.with_suffix(".stl")
        assert damaged.read_bytes()==source and not mesh.exists()
        click(window,650,54)  # Preview earlier directly from the failed row.
        verify(window,"Earlier preview / cut")
        capture(window,"standalone-history-recovery-earlier.png",False,surface=(124,163,136))
        key("Return")
        verify(window,"Earlier preview / cut")
        key("Control_L+Alt_L+e")
        verify(window,"Apply a repair before saving or exporting")
        assert damaged.read_bytes()==source and not damaged.with_suffix(".3mf").exists()
        key("Escape")
        verify(window,"Feature preview cancelled")
        click(window,75,180)  # Re-select the failed row.
        verify(window,"finish   error")
        key("s")
        verify(window,"Suppress preview / finish")
        key("Control_L+s")
        verify(window,"Apply a repair before saving or exporting")
        assert damaged.read_bytes()==source
        key("Escape")
        verify(window,"Feature preview cancelled")
        key("s")
        verify(window,"Suppress preview / finish")
        key("Return")
        verify(window,"finish   suppressed")
        key("Control_L+z")
        verify(window,"finish   error")
        key("Control_L+s")
        verify(window,"Apply a repair before saving or exporting")
        assert damaged.read_bytes()==source
        key("Control_L+y")
        verify(window,"finish   suppressed")
        key("Control_L+s")
        verify(window,"Saved")
        repaired={**broken,"nodes":[*broken["nodes"][:-1],{**broken["nodes"][-1],"suppressed":True}]}
        assert json.loads(damaged.read_bytes())==repaired
        key("Control_L+e")
        verify(window,"Exported")
        assert triangulation(mesh)==([(0,20),(0,10),(0,6)],1146)
        shutil.copyfile(damaged,output_dir/"standalone-history-repaired.pcad")
        shutil.copyfile(mesh,output_dir/"standalone-history-repaired.stl")
        key("Escape")
        verify(window,"Feature history closed")
        key("Escape")
        app.wait(3)
        app,window=open_app(damaged,"PCAD design opened",design_tree=True)
        key("Escape")
        app.wait(3)

        # Both a recognized but unrepairable graph and a graph containing an
        # unknown record still get the byte-preserving source reader.
        for name, doc, reason in (
            ("Missing input",{**broken,"nodes":[*broken["nodes"][:-1],
                                                {**broken["nodes"][-1],"source":"missing-stock"}]},"missing-stock"),
            ("Future feature",{**broken,"nodes":[*broken["nodes"],{"id":"future","type":"futureBlend"}]},
             "Unsupported standalone feature: futureBlend"),
        ):
            path=Path(tmp)/(name+".pcad")
            original=(json.dumps(doc,indent=2)+"\n").encode()
            path.write_bytes(original)
            app,window=open_app(path,"Read-only  |  File source",design_tree=True)
            verify(window,reason)
            key("F7")
            key("s")
            key("Control_L+s")
            key("Control_L+e")
            verify(window,"Read-only  |  File source")
            assert path.read_bytes()==original and not path.with_suffix(".stl").exists()
            key("Escape")
            app.wait(3)

        sheets=Path(tmp)/"Sensor sheets.pcad"
        shutil.copy(root/"tests/fixtures/sensor-unit.pcad",sheets)
        app,window=open_app(sheets,"PCAD design opened",design_tree=True)
        click(window,26,185)  # Expand the first part to expose its feature and datums.
        subprocess.run(["magick","import","-window",hex(window),output_dir/"design-tree.png"],check=True)
        def open_sheet(query, name):
            click(window,229,125)
            for letter in query:
                key("minus" if letter=="-" else letter)
            key("Return")
            click(window,80,185)
            verify(window,"Selected "+name)
            click(window,72,615)
            verify(window,"Drawing sheet  "+name)

        open_sheet("enclosure-plan","Sensor enclosure base")
        plan=output_dir/"drawing-sheet-plan.png"
        subprocess.run(["magick","import","-window",hex(window),plan],check=True)
        from PIL import Image
        with Image.open(plan) as image:
            viewport=image.convert("RGB").crop((260,76,960,550))
            colors=viewport.getcolors(viewport.width*viewport.height)
            counts={rgb:n for n,rgb in colors}
            assert counts.get((246,247,245),0)>100000 and counts.get((49,57,61),0)>200
        click(window,177,54)  # Export from the preview.
        verify(window,"Exported")
        assert b"/Count 4" in sheets.with_suffix(".pdf").read_bytes()
        # Select a different sheet while the current one is open.
        open_sheet("enclosure-section","Sensor enclosure section")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"drawing-sheet-section.png"],check=True)
        open_sheet("hinge-detail","Base hinge detail")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"drawing-sheet-detail.png"],check=True)
        open_sheet("exploded","Sensor unit exploded assembly")
        exploded=output_dir/"drawing-sheet-exploded.png"
        subprocess.run(["magick","import","-window",hex(window),exploded],check=True)
        with Image.open(exploded) as image:
            # A BOM balloon has a different stroke and background from the face edges.
            colors=image.convert("RGB").getcolors(image.width*image.height)
            assert dict((rgb,n) for n,rgb in colors)[(67,90,107)]>800
        key("F3")
        xlib.XResizeWindow.argtypes = [pointer, XID, C.c_uint, C.c_uint]
        xlib.XResizeWindow(connection, window, 420, 460)
        xlib.XSync(connection, 0)
        time.sleep(.22)
        compact=output_dir/"drawing-sheet-compact.png"
        subprocess.run(["magick","import","-window",hex(window),compact],check=True)
        with Image.open(compact) as image:
            assert image.size==(420,460)
            assert image.convert("RGB").getpixel((102,118))==(246,247,245)
        xlib.XResizeWindow(connection, window, 960, 640)
        xlib.XSync(connection, 0)
        key("Escape")
        verify(window,"Model view")
        from PIL import ImageChops
        returned=Path(tmp)/"returned-model.png"
        subprocess.run(["magick","import","-window",hex(window),returned],check=True)
        with Image.open(output_dir/"design-tree.png") as before, Image.open(returned) as after:
            area=(270,76,960,640)
            assert ImageChops.difference(before.convert("RGB").crop(area),after.convert("RGB").crop(area)).getbbox() is None
        assert sheets.read_bytes()==(root/"tests/fixtures/sensor-unit.pcad").read_bytes()
        key("Escape")
        app.wait(3)

        authored=Path(tmp)/"Authored sheet.pcad"
        model={
            "format":"puppycorp.pcad","schema_version":1,"units":"mm",
            "nodes":[{"id":"cube","type":"box","size":[24,12,6]}],
            "parts":[{"id":"block","node":"cube"}],
        }
        authored.write_text(json.dumps(model))
        app,window=open_app(authored,"PCAD design opened",design_tree=True)
        click(window,80,124)
        for letter in "new sheet":
            key("space" if letter==" " else letter)
        key("Return")
        click(window,80,185)
        verify(window,"Selected New sheet")
        key("Return")
        verify(window,"Added drawing sheet  Sheet 1")
        key("F2")
        verify(window,"Sheet name")
        key("Return")
        key("Shift_L+i")
        type_value("nspection")
        key("Return")
        verify(window,"Updated drawing sheet  Inspection")
        key("Right")
        verify(window,"Orientation")
        key("Return")
        type_value("front")
        key("Return")
        verify(window,"Updated drawing sheet  Inspection")
        key("Right")
        verify(window,"Scale denominator")
        key("Return")
        type_value("3")
        key("Return")
        verify(window,"Drawing scale must be Auto")
        key("Control_L+a")
        type_value("2")
        key("Return")
        verify(window,"Updated drawing sheet  Inspection")
        key("Right")
        verify(window,"Occurrence ID")
        key("Right")
        verify(window,"Section depth")
        key("Return")
        type_value("-6")
        key("Return")
        verify(window,"Updated drawing sheet  Inspection")
        key("Control_L+z")
        verify(window,"Drawing sheet  Inspection")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(authored.read_bytes())
        assert "section_mm" not in saved["drawings"][0]
        key("Control_L+y")
        verify(window,"Drawing sheet  Inspection")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(authored.read_bytes())
        assert saved.pop("drawings")==[dict(id="sheet-1",name="Inspection",axis="front",
                                           target="",scale=2,visibility="visible",dimensions=[],section_mm=-6)]
        assert saved==model
        key("Escape")
        verify(window,"Model view")
        key("Escape")
        app.wait(3)

        app,window=open_app(authored,"PCAD design opened",design_tree=True)
        open_sheet("sheet-1","Inspection")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"drawing-sheet-authored.png"],check=True)
        key("Control_L+p")
        verify(window,"Exported")
        pdf=authored.with_suffix(".pdf").read_bytes()
        assert b"(Inspection)" in pdf and b"Scale 1:2" in pdf and b"-6.000000 mm" in pdf
        shutil.copyfile(authored,output_dir/"drawing-sheet-authored.pcad")
        key("Escape")
        key("Escape")
        app.wait(3)

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

        path=Path(tmp)/"Centered plate.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        key("Shift_L+t")
        verify(window,"Click the rectangle center")
        click(window,480,358)
        verify(window,"Click a side midpoint")
        click(window,540,278)
        verify(window,"Click to set the perpendicular height")
        xlib.XWarpPointer(connection,0,window,0,0,0,0,448,334)
        xlib.XSync(connection,0)
        time.sleep(.18)
        capture(window,"centered-rectangle-preview.png",True)
        click(window,510,318)
        verify(window,"nonzero width and height")
        click(window,448,334)
        verify(window,"Curve added")
        click(window,255,55)  # Diameter circle on the Profiles toolbar.
        verify(window,"Click one end of the diameter")
        click(window,464,358)
        verify(window,"Click the opposite end of the diameter")
        click(window,464,358)
        verify(window,"Choose distinct ends of the diameter")
        click(window,496,358)
        verify(window,"Curve added")
        key("F8")
        verify(window,"Width 50 mm")
        key("F2")
        type_value("60")
        key("Return")
        verify(window,"Edited")
        key("Escape")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        rect,circle=(n["primitive"] for n in nodes if n["type"]=="sketchPrimitive")
        assert (rect["width"],rect["height"],rect["rotation"])==(60,20,53.130102)
        assert circle["center"]==rect["center"]==dict(x=0,y=0) and circle["radius"]==4
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"centered-rectangle-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        shutil.copyfile(path,output_dir/"centered-plate.pcad")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Polygon and slot.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        key("Shift_L+g")
        verify(window,"Click the polygon center")
        click(window,480,358)
        verify(window,"Click a vertex")
        click(window,540,278)
        verify(window,"Curve added")
        key("F2")
        verify(window,"Set value")
        type_value("2")
        key("Return")
        verify(window,"Enter a whole number of sides from 3 to 64")
        key("Control_L+a")
        type_value("8")
        key("Return")
        verify(window,"Profile updated")
        key("Shift_L+s")
        verify(window,"Click the first cap center")
        click(window,456,358)
        verify(window,"Click the second cap center")
        click(window,504,358)
        verify(window,"Click a side wall")
        xlib.XWarpPointer(connection,0,window,0,0,0,0,480,346)
        xlib.XSync(connection,0)
        time.sleep(.15)
        capture(window,"polygon-slot-preview.png",True)
        click(window,480,358)
        verify(window,"Choose a side wall away from the slot axis")
        click(window,480,346)
        verify(window,"Curve added")
        key("F8")
        verify(window,"Radius 25 mm   DOF 9")
        key("F2")
        type_value("24")
        key("Return")
        verify(window,"Edited")
        key("Escape")
        key("Control_L+s")
        verify(window,"Saved")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners",design_tree=True)
        click(window,50,125)
        for letter in "native-entity-2":
            key("minus" if letter=="-" else letter)
        key("Return")
        click(window,99,211)
        verify(window,"Selected Slot")
        click(window,76,615)
        verify(window,"Set value")
        type_value("8")
        key("Return")
        verify(window,"Profile updated")
        key("Control_L+z")
        key("Control_L+y")
        verify(window,"|  Sketch")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        nodes=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        polygon,slot=(n["primitive"] for n in nodes if n["type"]=="sketchPrimitive")
        assert (polygon["radius"],polygon["sides"],polygon["rotation"])==(24,8,53.130102)
        assert (slot["width"],slot["arcSegments"],slot["from"],slot["to"])==(8,48,dict(x=-6,y=0),dict(x=6,y=0))
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"polygon-slot-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        shutil.copyfile(path,output_dir/"polygon-slot.pcad")
        key("Escape")
        app.wait(3)

        path=Path(tmp)/"Split ellipse.pcad"
        app,window=open_app(path,"Click two rectangle corners")
        key("Shift_L+e")
        click(window,480,358)
        click(window,560,358)
        click(window,480,318)
        verify(window,"Curve added")
        click(window,474,55)  # Split
        verify(window,"Pick a cut on an ellipse, elliptic arc or spline")
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

        # A driving line carries the tangent arc while a separate circle stays dimensioned.
        path=Path(tmp)/"Driving tangent.pcad"
        document=json.loads((root/"rust/fixtures/tangent-arc.pcad").read_bytes())
        doc_part=document["items"][0]
        doc_part["name"]="Driving tangent"
        cad=doc_part["data"]["cad"]
        nodes={node["id"]:node for node in cad["nodes"]}
        sketch=nodes["native-sketch-1"]
        line=nodes["native-entity-1"]
        arc=nodes["native-entity-2"]
        line.update(p0=dict(x=-40,y=0),p1=dict(x=0,y=0))
        arc["arc"].update(center=dict(x=0,y=10),radius=10)
        join=sketch["relations"][0]
        sketch["relations"]=[join,
                             dict(id="line-horizontal",type="horizontal",entityId=line["id"]),
                             dict(id="line-length",type="length",entityId=line["id"],value=40),
                             dict(id="circle-diameter",type="diameter",entityId="native-entity-3",value=12)]
        circle=dict(id="native-entity-3",type="sketchPrimitive",sketchId=sketch["id"],
                    primitive=dict(id="native-entity-3",type="circle",center=dict(x=0,y=40),radius=6,segments=128))
        cad["nodes"]=list(nodes.values())[:-1]+[circle]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"native-entity-1   40 mm   DOF <=")
        key("F2")
        verify(window,"Set value")
        type_value("50 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"40 mm   DOF <=")
        key("Control_L+y")
        verify(window,"50 mm   DOF <=")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        saved={node["id"]:node for node in saved}
        assert saved[line["id"]]["p0"]==line["p0"] and saved[line["id"]]["p1"]==dict(x=10,y=0)
        assert saved[arc["id"]]["arc"]==dict(arc["arc"],center=dict(x=10,y=10))
        assert saved[circle["id"]]==circle and saved[sketch["id"]]["relations"][0]==join
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"native-entity-1   50 mm   DOF <=")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"driving-tangent.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)
        # Reopen the sketch at its default zoom so the clicks land at exact coordinates.
        app,window=open_app(path,"Click two rectangle corners")
        key("l")
        verify(window,"Click the two endpoints")
        click(window,560,318)  # The arc's moving endpoint (20, 10).
        click(window,320,358)  # The line's retained start (-40, 0).
        verify(window,"Sketch edited")
        key("p")
        verify(window,"Profile 2")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        ref=Path(tmp)/"driving-tangent-rust.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,ref],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),ref)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        print("Mixed smooth export",bounds,volume,flush=True)
        assert bounds==[(-40.0,20.0),(0.0,10.0),(0.0,10.0)] and volume>0
        key("Escape")
        app.wait(3)

        # Parallel and Perpendicular coexist with a driving length and a shared corner.
        path=Path(tmp)/"Right triangle.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        item=document["items"][0]
        item["name"]="Right triangle"
        cad=item["data"]["cad"]
        sketch,source=cad["nodes"][3:5]
        source.update(p0=dict(x=0,y=0),p1=dict(x=20,y=0))
        follower=dict(source,id="native-entity-2",p0=dict(x=20,y=0),p1=dict(x=26,y=8))
        cad["nodes"]=cad["nodes"][:5]+[follower]
        source_id,follower_id=source["id"],follower["id"]
        sketch["relations"]=[
            dict(id="keep-start",type="fixed",anchor=dict(entityId=source_id,point="p0"),position=source["p0"]),
            dict(id="keep-corner",type="fixed",anchor=dict(entityId=source_id,point="p1"),position=source["p1"]),
            dict(id="corner",type="coincident",a=dict(entityId=source_id,point="p1"),b=dict(entityId=follower_id,point="p0")),
            dict(id="side-length",type="length",entityId=follower_id,value=10,expression="10 mm"),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        def pick_direction(label,button_x):
            click(window,890,20)
            verify(window,"Choose a relation")
            click(window,button_x,54)
            verify(window,f"Pick the source line for {label}")
            click(window,384,486)
            verify(window,f"Pick the follower line for {label}")
        pick_direction("Parallel",70)
        click(window,800,358)
        verify(window,"Parallel updated")
        key("Control_L+z")
        verify(window,"native-entity-2   10 mm   DOF 1")
        pick_direction("Perpendicular",190)
        click(window,800,358)
        verify(window,"Perpendicular updated")
        key("F2")
        verify(window,"Set value")
        key("Control_L+a")
        type_value("15 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())["items"][0]["data"]["cad"]
        nodes={node["id"]:node for node in saved["nodes"]}
        assert nodes[source_id]==source and nodes[follower_id]==dict(follower,p1=dict(x=20,y=15))
        records={rel["id"]:rel for rel in nodes[sketch["id"]]["relations"]}
        relation_id=f"native-perpendicular-{source_id}-{follower_id}"
        assert records[relation_id]==dict(id=relation_id,type="perpendicular",a=source_id,b=follower_id)
        assert records["corner"]==sketch["relations"][2]
        assert records["side-length"]["expression"]=="15 mm" and records["side-length"]["value"]==15
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"line-perpendicular.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("l")
        verify(window,"Click the two endpoints")
        click(window,560,298)  # (20, 15)
        click(window,480,358)  # (0, 0)
        verify(window,"Sketch edited")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"right-triangle-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        assert bounds==[(0.0,20.0),(0.0,15.0),(0.0,10.0)] and math.isclose(volume,1500,rel_tol=1e-6)
        key("Escape")
        app.wait(3)

        # Either line can drive Equal without detaching the other profile edges.
        path=Path(tmp)/"Equal profile.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        part=document["items"][0]
        part["name"]="Equal profile"
        graph=part["data"]["cad"]
        sketch,source=graph["nodes"][3:5]
        sk=sketch["id"]
        first,second,datum,chord,closing=(f"native-entity-{i}" for i in range(1,6))
        origin=dict(x=0,y=0)
        start=dict(x=0,y=30)
        source.update(p0=origin,p1=dict(x=20,y=0))
        follower=dict(source,id=second,p0=start,p1=dict(x=0,y=40))
        anchor=dict(id=datum,type="sketchPrimitive",sketchId=sk,primitive=dict(id=datum,type="point",center=start,construction=True))
        diagonal=dict(source,id=chord,p0=source["p1"],p1=follower["p1"])
        tail=dict(source,id=closing,p0=start,p1=origin)
        graph["nodes"]=graph["nodes"][:5]+[follower,anchor,diagonal,tail]
        def equal_anchor(entity,point):
            return dict(entityId=entity,point=point)
        sketch["relations"]=[
            dict(id="left-start",type="fixed",anchor=equal_anchor(first,"p0"),position=origin),
            dict(id="right-start",type="fixed",anchor=equal_anchor(datum,"center"),position=start),
            dict(id="horizontal",type="horizontal",entityId=first),
            dict(id="upright",type="perpendicular",a=first,b=second),
            dict(id="at-datum",type="coincident",a=equal_anchor(datum,"center"),b=equal_anchor(second,"p0")),
            dict(id="diagonal-start",type="coincident",a=equal_anchor(first,"p1"),b=equal_anchor(chord,"p0")),
            dict(id="diagonal-end",type="coincident",a=equal_anchor(second,"p1"),b=equal_anchor(chord,"p1")),
            dict(id="tail-start",type="coincident",a=equal_anchor(second,"p0"),b=equal_anchor(closing,"p0")),
            dict(id="tail-end",type="coincident",a=equal_anchor(first,"p0"),b=equal_anchor(closing,"p1")),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"native-entity-1   20 mm   DOF 2")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,310,54)
        verify(window,"Pick the source line for Equal")
        click(window,480,616)
        verify(window,"Pick the follower line for Equal")
        click(window,351,164)
        verify(window,"Equal updated")

        def drive_equal(side,length):
            key("F2")
            verify(window,"Set value")
            type_value(f"{length} mm")
            key("Return")
            verify(window,"Edited")
            key("Control_L+s")
            verify(window,"Saved")
            rows=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
            nodes={node["id"]:node for node in rows}
            a,b,c=nodes[first],nodes[second],nodes[chord]
            assert a["p0"]==origin and b["p0"]==start
            assert math.isclose(a["p1"]["x"],length,abs_tol=.0004) and a["p1"]["y"]==0
            assert math.isclose(b["p1"]["y"],30+length,abs_tol=.0004) and b["p1"]["x"]==0
            assert c["p0"]==a["p1"] and c["p1"]==b["p1"]
            rel=f"native-equal-{first}-{second}"
            relations={row["id"]:row for row in nodes[sk]["relations"]}
            assert relations[rel]==dict(id=rel,type="equal",a=first,b=second)
            lengths=[row for row in relations.values() if row["type"]=="length"]
            assert len(lengths)==1 and lengths[0]["entityId"]==side and lengths[0]["value"]==length

        drive_equal(second,25)
        key("Control_L+z")
        verify(window,"native-entity-2   20 mm   DOF 1")
        click(window,480,616)
        verify(window,"native-entity-1   20 mm   DOF 1")
        drive_equal(first,30)
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"line-equal.png"],check=True)
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
        reference=Path(tmp)/"equal-profile-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        assert bounds[2]==(0.0,10.0) and math.isclose(volume,9000,rel_tol=5e-5)
        key("Escape")
        app.wait(3)

        # The named datum dimension moves the concentric seat and bore together.
        path=Path(tmp)/"Concentric seat.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        item=document["items"][0]
        item["name"]="Concentric seat"
        graph=item["data"]["cad"]
        sketch=graph["nodes"][3]
        sk=sketch["id"]
        bore,seat,datum="outer-bore","ellipse-seat","station-datum"
        def concentric_node(id,kind,center,**fields):
            return dict(id=id,sketchId=sk,type="sketchPrimitive",primitive=dict(id=id,type=kind,center=center,**fields))
        graph["nodes"]=graph["nodes"][:4]+[
            concentric_node(bore,"circle",dict(x=10,y=0),radius=15,segments=64),
            concentric_node(seat,"ellipse",dict(x=13,y=2),width=12,height=6,rotation=30,segments=64),
            concentric_node(datum,"point",dict(x=0,y=0),construction=True),
        ]
        sketch["relations"]=[
            dict(id="datum-origin",type="fixed",anchor=dict(entityId=datum,point="center"),position=dict(x=0,y=0)),
            dict(id="station-center-x",type="distance",axis="x",a=dict(entityId=datum,point="center"),b=dict(entityId=bore,point="center"),value=10),
            dict(id="outer-diameter",type="diameter",entityId=bore,value=30),
            dict(id="seat-width",type="width",entityId=seat,value=12),
            dict(id="seat-height",type="height",entityId=seat,value=6),
            dict(id="seat-angle",type="rotation",entityId=seat,value=30),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"outer-bore   Diameter 30 mm   DOF 3")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,70,54)
        verify(window,"Pick the source curve for Concentric")
        click(window,738,358)
        verify(window,"Pick the follower curve for Concentric")
        click(window,621,272)
        verify(window,"Concentric updated")
        key("d")
        verify(window,"station-center-x   X distance   10 mm   DOF 1")
        key("F2")
        verify(window,"Set value")
        key("Control_L+a")
        type_value("1.6 cm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"X distance   10 mm")
        key("Control_L+y")
        verify(window,"X distance   16 mm")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        nodes={node["id"]:node for node in saved}
        center=dict(x=16,y=0)
        assert nodes[bore]["primitive"]==dict(id=bore,type="circle",center=center,radius=15,segments=64)
        assert nodes[seat]["primitive"]==dict(id=seat,type="ellipse",center=center,width=12,height=6,rotation=30,segments=64)
        relations={row["id"]:row for row in nodes[sk]["relations"]}
        relation_id=f"native-concentric-{bore}-{seat}"
        assert relations[relation_id]==dict(id=relation_id,type="concentric",a=bore,b=seat)
        assert relations["station-center-x"]["expression"]=="1.6 cm" and relations["station-center-x"]["value"]==16
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"concentric-seat.png"],check=True)
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
        reference=Path(tmp)/"concentric-seat-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        area=(15**2-6*3)*32*math.sin(math.tau/64)
        assert bounds==[(1.0,31.0),(-15.0,15.0),(0.0,10.0)] and math.isclose(volume,area*10,rel_tol=1e-6)
        key("Escape")
        app.wait(3)

        # Whole-curve tangency follows a finite construction arc without an endpoint join.
        path=Path(tmp)/"Circular tangent stop.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        item=document["items"][0]
        item["name"]="Circular tangent stop"
        graph=item["data"]["cad"]
        sketch=graph["nodes"][3]
        sk=sketch["id"]
        guide,stop="guide-arc","stop-circle"
        graph["nodes"]=graph["nodes"][:4]+[
            dict(id=guide,type="sketchArc",sketchId=sk,arc=dict(id=guide,type="arc",center=dict(x=0,y=0),
                 radius=20,startAngle=-math.pi/4,sweep=math.pi/2,segments=64,construction=True)),
            dict(id=stop,type="sketchPrimitive",sketchId=sk,primitive=dict(id=stop,type="circle",center=dict(x=40,y=0),radius=10,segments=64)),
        ]
        sketch["relations"]=[
            dict(id="guide-center",type="fixed",anchor=dict(entityId=guide,point="center"),position=dict(x=0,y=0)),
            dict(id="guide-radius",type="radius",entityId=guide,value=20),
            dict(id="guide-sweep",type="arcSweep",entityId=guide,value=90),
            dict(id="stop-diameter",type="diameter",entityId=stop,value=20),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"guide-arc   Radius 20 mm   DOF 3")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,310,54)
        verify(window,"Pick the outer curve for Internal tangent")
        click(window,211,176)
        verify(window,"Pick the inner curve for Internal tangent")
        click(window,807,358)
        verify(window,"Internal tangent updated")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        assert saved[-1]["primitive"]["center"]==dict(x=10,y=0)
        assert saved[3]["relations"][-1]==dict(id=f"native-internal-tangent-{guide}-{stop}",type="internalTangent",a=guide,b=stop)
        key("Control_L+z")
        verify(window,"stop-circle   Diameter 20 mm   DOF 3")

        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,190,54)
        verify(window,"Pick the source curve for Tangent")
        click(window,211,176)
        verify(window,"Pick the follower curve for Tangent")
        click(window,807,358)
        verify(window,"Tangent updated")
        click(window,211,176)
        verify(window,"guide-arc   Radius 20 mm   DOF 2")
        key("F2")
        verify(window,"Set value")
        key("Control_L+a")
        type_value("25 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]
        nodes={node["id"]:node for node in saved}
        assert nodes[guide]["arc"]==dict(graph["nodes"][4]["arc"],radius=25)
        assert nodes[stop]["primitive"]==dict(graph["nodes"][5]["primitive"],center=dict(x=35,y=0))
        relation_id=f"native-external-tangent-{guide}-{stop}"
        assert nodes[sk]["relations"][-1]==dict(id=relation_id,type="tangent",a=guide,b=stop)
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"circular-tangent.png"],check=True)
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
        reference=Path(tmp)/"circular-tangent-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        assert bounds==[(25.0,45.0),(-10.0,10.0),(0.0,10.0)] and math.isclose(volume,32000*math.sin(math.tau/64),rel_tol=1e-6)
        key("Escape")
        app.wait(3)

        # A construction line and one circle can use the same Relations > Tangent command.
        path=Path(tmp)/"Line tangent roller.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        item=document["items"][0]
        item["name"]="Line tangent roller"
        graph=item["data"]["cad"]
        sketch=graph["nodes"][3]
        sk=sketch["id"]
        guide,roller="guide-line","roller-circle"
        left,right=dict(x=-20,y=0),dict(x=20,y=0)
        graph["nodes"]=graph["nodes"][:4]+[
            dict(id=guide,type="sketchLine",sketchId=sk,p0=left,p1=right,construction=True),
            dict(id=roller,type="sketchPrimitive",sketchId=sk,
                 primitive=dict(id=roller,type="circle",center=dict(x=0,y=20),radius=10,segments=64)),
        ]
        sketch["relations"]=[
            dict(id="guide-start",type="fixed",anchor=dict(entityId=guide,point="p0"),position=left),
            dict(id="guide-end",type="fixed",anchor=dict(entityId=guide,point="p1"),position=right),
            dict(id="roller-diameter",type="diameter",entityId=roller,value=20),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"guide-line   40 mm   DOF 2")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,46,54)
        verify(window,"Pick the source line or curve for Tangent")
        click(window,480,616)
        verify(window,"Pick the follower line or curve for Tangent")
        click(window,652,272)
        verify(window,"Tangent updated")
        key("F2")
        verify(window,"Set value")
        key("Control_L+a")
        type_value("24 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"roller-circle   Diameter 20 mm   DOF 1")
        key("Control_L+y")
        verify(window,"roller-circle   Diameter 24 mm   DOF 1")
        key("Control_L+s")
        verify(window,"Saved")
        saved={node["id"]:node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        link_id=f"native-external-tangent-{guide}-{roller}"
        assert saved[guide]==graph["nodes"][4]
        assert saved[roller]["primitive"]==dict(graph["nodes"][5]["primitive"],center=dict(x=0,y=12),radius=12)
        assert saved[sk]["relations"][-1]==dict(id=link_id,type="tangent",a=guide,b=roller)
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"line-tangent.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"guide-line   40 mm   DOF 1")
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"line-tangent-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        assert bounds==[(-12.0,12.0),(0.0,24.0),(0.0,10.0)] and math.isclose(volume,46080*math.sin(math.tau/64),rel_tol=1e-6)
        key("Escape")
        app.wait(3)

        # A rotated ellipse follows a fixed tangent line through axis and rotation edits.
        path=Path(tmp)/"Line tangent ellipse.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        item=document["items"][0]
        item["name"]="Line tangent ellipse"
        graph=item["data"]["cad"]
        sketch=graph["nodes"][3]
        sk=sketch["id"]
        guide,roller="guide-line","roller-ellipse"
        left,right=dict(x=-40,y=0),dict(x=40,y=0)
        oval=dict(id=roller,type="ellipse",center=dict(x=0,y=30),width=40,height=20,rotation=30,segments=128)
        graph["nodes"]=graph["nodes"][:4]+[
            dict(id=guide,type="sketchLine",sketchId=sk,p0=left,p1=right,construction=True),
            dict(id=roller,type="sketchPrimitive",sketchId=sk,primitive=oval),
        ]
        sketch["relations"]=[
            dict(id="guide-start",type="fixed",anchor=dict(entityId=guide,point="p0"),position=left),
            dict(id="guide-end",type="fixed",anchor=dict(entityId=guide,point="p1"),position=right),
            *(dict(id=f"roller-{field}",type=field,entityId=roller,value=oval[field]) for field in ("width","height","rotation")),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"guide-line   80 mm   DOF 2")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,46,54)
        verify(window,"Pick the source line or curve for Tangent")
        click(window,480,604)
        verify(window,"Pick the follower line or curve for Tangent")
        click(window,677,148)
        verify(window,"Tangent updated")

        def edit_ellipse_tangent_dimension(text):
            key("F2")
            verify(window,"Set value")
            key("Control_L+a")
            type_value(text)
            key("Return")
            verify(window,"Edited")

        def saved_ellipse_tangent(rotation):
            key("Control_L+s")
            verify(window,"Saved")
            saved={node["id"]:node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
            link_id=f"native-external-tangent-{guide}-{roller}"
            assert saved[guide]==graph["nodes"][4]
            assert saved[sk]["relations"][-1]==dict(id=link_id,type="tangent",a=guide,b=roller)
            primitive=saved[roller]["primitive"]
            assert primitive["width"]==48 and primitive["height"]==20 and primitive["rotation"]==rotation
            angle=math.radians(rotation)
            assert primitive["center"]["x"]==0 and math.isclose(primitive["center"]["y"],math.hypot(24*math.sin(angle),10*math.cos(angle)),abs_tol=0.000002)

        edit_ellipse_tangent_dimension("48 mm")
        saved_ellipse_tangent(30)
        key("Tab")
        verify(window,"roller-ellipse   Height 20 mm   DOF 1")
        key("Tab")
        verify(window,"roller-ellipse   Rotation 30 deg   DOF 1")
        edit_ellipse_tangent_dimension("45 deg")
        key("Control_L+z")
        verify(window,"roller-ellipse   Rotation 30 deg   DOF 1")
        key("Control_L+y")
        verify(window,"roller-ellipse   Rotation 45 deg   DOF 1")
        saved_ellipse_tangent(45)
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"ellipse-tangent.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"guide-line   80 mm   DOF 1")
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"ellipse-tangent-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        assert bounds[2]==(0.0,10.0) and math.isclose(volume,153600*math.sin(math.tau/128),rel_tol=1e-6)
        key("Escape")
        app.wait(3)

        # A standalone point follows a driven construction line's midpoint.
        path=Path(tmp)/"Midpoint guide.pcad"
        document=json.loads((root/"rust/fixtures/graph-constrained-line.pcad").read_bytes())
        item=document["items"][0]
        item["name"]="Midpoint guide"
        graph=item["data"]["cad"]
        sketch=graph["nodes"][3]
        sk=sketch["id"]
        guide,datum="layout-guide","center-mark"
        zero=dict(x=0,y=0)
        graph["nodes"]=graph["nodes"][:4]+[
            dict(id=guide,type="sketchLine",sketchId=sk,p0=zero,p1=dict(x=20,y=0),construction=True),
            dict(id=datum,type="sketchPrimitive",sketchId=sk,
                 primitive=dict(id=datum,type="point",center=dict(x=5,y=5),construction=True)),
            dict(id="block-outline",type="sketchPrimitive",sketchId=sk,
                 primitive=dict(id="block-outline",type="rectangle",center=dict(x=35,y=0),width=10,height=10,rotation=0)),
        ]
        sketch["relations"]=[
            dict(id="guide-origin",type="fixed",anchor=dict(entityId=guide,point="p0"),position=zero),
            dict(id="guide-level",type="horizontal",entityId=guide),
            dict(id="guide-span",type="length",entityId=guide,value=20),
        ]
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"layout-guide   20 mm   DOF 7")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,46,54)
        verify(window,"Pick the source line or arc for Midpoint")
        click(window,190,358)
        verify(window,"Pick the follower anchor for Midpoint")
        click(window,138,244)
        verify(window,"Midpoint updated")

        def saved_midpoint(length):
            key("Control_L+s")
            verify(window,"Saved")
            nodes={node["id"]:node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
            relations={rel["id"]:rel for rel in nodes[sk]["relations"]}
            link_id=f"native-midpoint-{datum}-{guide}"
            assert relations[link_id]==dict(id=link_id,type="midpoint",a=dict(entityId=datum,point="center"),b=guide)
            assert relations["guide-span"]["value"]==length
            assert nodes[guide]["p0"]==zero and nodes[guide]["p1"]==dict(x=length,y=0)
            assert nodes[datum]["primitive"]["center"]==dict(x=length/2,y=0)

        saved_midpoint(20)
        key("F2")
        verify(window,"Set value")
        key("Control_L+a")
        type_value("30 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"layout-guide   20 mm   DOF 5")
        key("Control_L+y")
        verify(window,"layout-guide   30 mm   DOF 5")
        saved_midpoint(30)
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"midpoint-guide.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"layout-guide   30 mm   DOF 5")
        key("Escape")
        key("e")
        verify(window,"Extruded")
        saved_midpoint(30)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"midpoint-guide-reference.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        bounds,volume=package_geometry(path.with_suffix(".3mf"))[1][0][1]
        assert bounds==[(30.0,40.0),(-5.0,5.0),(0.0,10.0)] and math.isclose(volume,1000,rel_tol=1e-8)
        key("Escape")
        app.wait(3)

        # An anchor on a finite guide keeps its freedom along the segment.
        path=Path(tmp)/"Point on guide.pcad"
        path.write_text(json.dumps(document))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"layout-guide   20 mm   DOF 7")
        click(window,890,20)
        verify(window,"Choose a relation")
        click(window,185,54)
        verify(window,"Pick the source line, circle or circular arc for Point on curve")
        click(window,190,358)
        verify(window,"Pick the follower anchor for Point on curve")
        click(window,138,244)
        verify(window,"Point on curve updated")
        key("F2")
        verify(window,"Set value")
        type_value("4 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        nodes={node["id"]:node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        link_id=f"native-point-on-curve-{datum}-{guide}"
        assert next(rel for rel in nodes[sk]["relations"] if rel["id"]==link_id)==dict(
            id=link_id,type="pointOnCurve",a=dict(entityId=datum,point="center"),b=guide)
        center=nodes[datum]["primitive"]["center"]
        end=nodes[guide]["p1"]
        for position in (center,end):
            assert math.dist((position["x"],position["y"]),(4,0))<=.000002,(center,end)
        key("Escape")
        key("Escape")
        app.wait(3)

        # A saved circular contact follows its driving diameter through solid rebuild.
        path=Path(tmp)/"Point on circle.pcad"
        path.write_bytes((libs/"cad/tests/fixtures/point-on-circle.pcad").read_bytes())
        shutil.copyfile(path,output_dir/"point-on-circle-input.pcad")
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"rim   Diameter 40 mm   DOF 1")
        key("F2")
        verify(window,"Set value")
        type_value("50 mm")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"rim   Diameter 40 mm   DOF 1")
        key("Control_L+y")
        verify(window,"rim   Diameter 50 mm   DOF 1")
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"point-on-circle.png"],check=True)
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        nodes={node["id"]:node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        relation=next(rel for rel in nodes["native-sketch-1"]["relations"] if rel["id"]=="ring-contact-from-web")
        assert relation==dict(id="ring-contact-from-web",type="pointOnCurve",a=dict(entityId="probe",point="center"),b="rim")
        assert nodes["probe"]["primitive"]["center"]==dict(x=25,y=0)
        shutil.copyfile(path,output_dir/"point-on-circle-edited.pcad")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+e")
        verify(window,"Exported")
        bounds,volume=triangulation(path.with_suffix(".stl"))
        assert bounds==[(-25,25),(-25,25),(0,10)] and math.isclose(volume,320*25**2*math.sin(math.tau/64),abs_tol=.008),(bounds,volume)
        shutil.copyfile(path.with_suffix(".stl"),output_dir/"point-on-circle-edited.stl")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=path.with_name("Point on circle reference.3mf")
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        key("Escape")
        app.wait(3)

        # A circular contact clamps to the end of a shortened clockwise arc.
        path=Path(tmp)/"Point on arc.pcad"
        path.write_bytes((libs/"cad/tests/fixtures/point-on-arc.pcad").read_bytes())
        shutil.copyfile(path,output_dir/"point-on-arc-input.pcad")
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"sweep-guide   Radius 20 mm")
        key("F2")
        verify(window,"Set value")
        type_value("25 mm")
        key("Return")
        verify(window,"Edited")
        key("Tab")
        verify(window,"sweep-guide   Sweep -90 deg")
        key("F2")
        verify(window,"Set value")
        type_value("-30 deg")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"sweep-guide   Sweep -90 deg")
        key("Control_L+y")
        verify(window,"sweep-guide   Sweep -30 deg")
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"point-on-arc.png"],check=True)
        key("Escape")
        key("e")
        verify(window,"Extruded")
        key("Control_L+s")
        verify(window,"Saved")
        nodes={node["id"]:node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]}
        relation=next(rel for rel in nodes["native-sketch-1"]["relations"] if rel["id"]=="probe-on-sweep-from-web")
        assert relation==dict(id="probe-on-sweep-from-web",type="pointOnCurve",a=dict(entityId="probe",point="center"),b="sweep-guide")
        assert nodes["probe"]["primitive"]["center"]==dict(x=21.650635,y=-12.5)
        shutil.copyfile(path,output_dir/"point-on-arc-edited.pcad")
        key("Escape")
        app.wait(3)
        app,window=open_app(path)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        bounds,volume=triangulation(stl)
        assert bounds==[(40,50),(-5,5),(0,10)] and math.isclose(volume,784.1372615,abs_tol=.008),(bounds,volume)
        shutil.copyfile(stl,output_dir/"point-on-arc-edited.stl")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=path.with_name("Point on arc reference.3mf")
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        key("Escape")
        app.wait(3)

        # A TypeScript elliptic arc keeps its authored wrapper and the adaptive profile.
        path=Path(tmp)/"Web elliptic arc.pcad"
        path.write_bytes((libs/"cad/tests/fixtures/web-elliptic-arc.pcad").read_bytes())
        shutil.copyfile(path,output_dir/"web-elliptic-arc-input.pcad")
        app,window=open_app(path)
        key("F8")
        verify(window,"closing-chord   40 mm   DOF 2")
        for _ in range(3):
            key("Tab")
        verify(window,"web-profile-arc   Height 10 mm   DOF 2")
        legacy_number("12 mm")
        verify(window,"Edited")
        key("Tab")
        key("Tab")
        verify(window,"web-guide-arc   Sweep -90 deg   DOF 2")
        legacy_number("-60 deg")
        verify(window,"Edited")
        key("Tab")
        verify(window,"web-guide-arc   Width 20 mm   DOF 2")
        legacy_number("24 mm")
        verify(window,"Edited")
        key("Control_L+z")
        verify(window,"web-guide-arc   Width 20 mm   DOF 2")
        key("Control_L+y")
        verify(window,"web-guide-arc   Width 24 mm   DOF 2")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        nodes={node["id"]:node for node in saved["items"][0]["data"]["cad"]["nodes"]}
        for arc_id in ("web-profile-arc","web-guide-arc"):
            node=nodes[arc_id]
            assert node["type"]=="sketchArc" and "primitive" not in node
            assert node["arc"]["id"]==arc_id and node["arc"]["type"]=="ellipticArc"
        assert nodes["web-profile-arc"]["arc"]["height"]==12
        guide=nodes["web-guide-arc"]["arc"]
        assert guide["width"]==24 and guide["height"]==10 and guide["rotation"]==-20 and guide["construction"]
        assert math.isclose(guide["sweep"],-math.pi/3,rel_tol=0,abs_tol=1e-12)
        shutil.copyfile(path,output_dir/"web-elliptic-arc-edited.pcad")
        key("Home")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"web-elliptic-arc.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)

        app,window=open_app(path)
        key("F8")
        verify(window,"closing-chord   40 mm   DOF 2")
        for _ in range(5):
            key("Tab")
        verify(window,"web-guide-arc   Sweep -60 deg   DOF 2")
        key("Tab")
        verify(window,"web-guide-arc   Width 24 mm   DOF 2")
        key("Escape")
        key("Control_L+e")
        verify(window,"Exported")
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl");package=path.with_suffix(".3mf")
        _,bodies=package_geometry(package)
        assert len(bodies)==1
        expected_bounds=[(-17.575858203,17.320508076),(-10,11.266332321),(0,10)]
        for bounds,volume in (triangulation(stl),bodies[0][1]):
            assert all(math.isclose(value,expected,abs_tol=.000003)
                       for pair,reference in zip(bounds,expected_bounds)
                       for value,expected in zip(pair,reference)),bounds
            assert math.isclose(volume,1883.715585879,abs_tol=.0001),volume
        shutil.copyfile(stl,output_dir/"web-elliptic-arc-edited.stl")
        shutil.copyfile(package,output_dir/"web-elliptic-arc-edited.3mf")
        for arc_id in ("web-profile-arc","web-guide-arc"):
            nodes[arc_id]["type"]="sketchPrimitive"
            nodes[arc_id]["primitive"]=nodes[arc_id].pop("arc")
        alias=Path(tmp)/"Rust elliptic arc wrapper.pcad"
        alias.write_text(json.dumps(saved))
        reference=alias.with_suffix(".3mf")
        native_alias=alias.with_name("Native Rust elliptic arc wrapper.3mf")
        subprocess.run([output_dir/"puppycad","--export",alias,native_alias],check=True)
        subprocess.run([root/"rust/target/debug/puppycad","--export",alias,reference],check=True)
        compare_solid_packages(native_alias,reference)
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
        verify(window,"Pick the point, line, circle or arc to mirror")
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
        verify(window,"Pick the point, line, circle or arc to mirror")
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

        path=Path(tmp)/"Arc mirror.pcad"
        drawing=json.loads(fixture.read_bytes())
        cad=drawing["items"][0]["data"]["cad"]
        nodes=[node for node in cad["nodes"] if node["type"] in {"referencePlane","sketch"}]
        sk="native-sketch-1"
        next(node for node in nodes if node["id"]==sk)["relations"]=[]
        axis,source,reflected=("native-entity-"+str(i) for i in (1,2,3))
        cad["nodes"]=nodes+[
            dict(id=axis,type="sketchLine",sketchId=sk,construction=True,p0=dict(x=0,y=-40),p1=dict(x=0,y=40)),
            dict(id=source,type="sketchArc",sketchId=sk,arc=dict(id=source,type="arc",center=dict(x=30,y=10),
                 radius=10,startAngle=0,sweep=math.pi/2,segments=64)),
        ]
        path.write_text(json.dumps(drawing))
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"DOF 9")
        key("m")
        verify(window,"Pick the mirror line")
        click(window,351,358)
        mirror_hint="Pick the point, line, circle or arc to mirror"
        verify(window,mirror_hint)
        click(window,499,339)  # On the supporting circle, outside the finite arc.
        verify(window,mirror_hint)
        click(window,590,248)
        verify(window,"Mirror copy added")
        for field,value in (("Radius","10 mm"),("Sweep","90 deg")):
            key("Tab")
            verify(window,f"{source}   {field} {value}")
            key("r")
            verify(window,f"Reference {field} {value}")
        for field,value,edited in (("Radius","10 mm","12 mm"),("Sweep","-90 deg","-110 deg")):
            key("Tab")
            verify(window,f"{reflected}   {field} {value}")
            key("F2")
            type_value(edited)
            key("Return")
            verify(window,"Edited")
        key("Control_L+z")
        verify(window,"Sweep -90 deg")
        key("Control_L+y")
        verify(window,"Sweep -110 deg")
        key("Control_L+s")
        verify(window,"Saved")
        graph=mirror_nodes()
        first,second=graph[source]["arc"],graph[reflected]["arc"]
        assert first["center"]==dict(x=30,y=10) and second["center"]==dict(x=-30,y=10)
        assert first["radius"]==second["radius"]==12
        assert math.isclose(first["sweep"],math.radians(110),abs_tol=1e-12) and first["sweep"]==-second["sweep"]
        assert math.isclose(abs(second["startAngle"]),math.pi,abs_tol=1e-12)
        relations=graph[sk]["relations"]
        assert relations[0]==dict(id="mirror-1",type="mirror",symmetryLine=axis,a=source,b=reflected)
        assert {r["type"]:r["value"] for r in relations if r.get("reference")}==dict(radius=12,arcSweep=110)
        key("Escape")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Click two rectangle corners")
        key("F8")
        verify(window,"DOF 7")
        key("Tab")
        verify(window,"Reference Radius 12 mm")
        key("Tab")
        verify(window,"Reference Sweep 110 deg")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"sketch-arc-mirror.png"],check=True)
        key("Escape")
        key("Escape")
        app.wait(3)

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

        # History deletion removes the feature and both saved registrations.
        path = Path(tmp)/"Delete block.pcad"
        path.write_bytes((root/"rust/fixtures/extruded-rectangle.pcad").read_bytes())
        app, window = open_app(path, design_tree=True)
        key("F7")
        verify(window, "1 / 1   Block   active")
        capture(window, "feature-history-delete.png", False)
        click(window, 420, 55)  # Delete feature in the History toolbar.
        verify(window, "Deleted Block")
        key("Control_L+z")
        key("F7")
        verify(window, "1 / 1   Block   active")
        key("Control_L+y")
        verify(window, "This part has no solid features")
        key("Control_L+s")
        verify(window, "Saved")
        data = json.loads(path.read_bytes())["items"][0]["data"]
        assert all(node["id"] != "extrude-1" for node in data["cad"]["nodes"])
        assert "extrude-1" not in data["cad"]["rootNodeIds"]
        assert data["tree"]["orderedNodeIds"] == ["native-sketch-1"]
        key("Escape")
        app.wait(3)
        app, window = open_app(path, "Click two rectangle corners", design_tree=True)
        key("F7")
        verify(window, "This part has no solid features")
        key("Escape")
        app.wait(3)

        # Suppressed dependents still block deletion of their source feature.
        path = Path(tmp)/"Delete blocked solid.pcad"
        path.write_bytes((root/"rust/fixtures/feature-tree.pcad").read_bytes())
        app, window = open_app(path, design_tree=True)
        key("F7")
        verify(window, "1 / 3   Extrusion 1   active")
        key("Delete")
        verify(window, "Later cut or join features depend on this extrusion")
        key("Right")
        verify(window, "2 / 3   Extrusion 3   suppressed")
        key("Right")
        verify(window, "3 / 3   Extrusion 2   active")
        key("Delete")
        verify(window, "2 / 2   Extrusion 3   suppressed")
        key("Control_L+z")
        verify(window, "2 / 3   Extrusion 3   suppressed")
        key("Control_L+y")
        verify(window, "2 / 2   Extrusion 3   suppressed")
        key("Control_L+s")
        verify(window, "Saved")
        data = json.loads(path.read_bytes())["items"][0]["data"]
        assert [node["id"] for node in data["cad"]["nodes"] if node["type"] == "extrude"] == ["native-extrude-1", "native-extrude-3"]
        key("Escape")
        key("Escape")
        app.wait(3)
        app, window = open_app(path)
        key("Control_L+e")
        verify(window, "Exported")
        native_stl = path.with_suffix(".stl")
        reference = Path(tmp)/"deleted-feature-rust.stl"
        subprocess.run([root/"rust/target/debug/puppycad", "--export", path, reference], check=True)
        bounds, volume = triangulation(native_stl)
        ref_bounds, ref_volume = triangulation(reference)
        assert bounds == ref_bounds and math.isclose(volume, ref_volume, abs_tol=.01)
        shutil.copyfile(path, output_dir/"feature-history-deleted.pcad")
        shutil.copyfile(native_stl, output_dir/"feature-history-deleted.stl")
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
        reference = Path(tmp)/"failed-feature-rust.3mf"
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
        key('Control_L+s')
        verify(window,'Saved')
        before_revolve=path.read_bytes()
        click(window,45,21)
        verify(window,'choose X or Y')
        click(window,550,280)
        click(window,550,390)
        verify(window,'crosses the revolve axis')
        key('Escape')
        verify(window,'Revolve cancelled')
        assert path.read_bytes()==before_revolve
        key('8')
        click(window,490,398)
        verify(window,'Click the axis endpoint')
        click(window,470,318)
        verify(window,'Revolve preview / New body / 360 deg')
        type_value('361')
        verify(window,'Enter a positive angle up to 360 degrees')
        key('Return')
        verify(window,'Enter a positive angle up to 360 degrees')
        assert path.read_bytes()==before_revolve
        key('Control_L+a')
        type_value('95.125')
        verify(window,'Revolve preview / New body / 95.125 deg')
        key('f')
        verify(window,'95.125 deg / Flipped')
        preview=output_dir/'revolve-preview.png'
        subprocess.run(['magick','import','-window',hex(window),preview],check=True)
        from PIL import Image
        with Image.open(preview) as image:
            area=image.convert('RGB').crop((0,76,image.width,image.height))
            painted=sum(n for n,(r,g,b) in area.getcolors(area.width*area.height) if b>g+12 and g>r+35)
            assert painted>1000
        key('Return')
        verify(window,'Revolved')
        verify(window,'angle 95.125 deg')
        key('Control_L+z')
        verify(window,'Sketch 1')
        key('Control_L+y')
        verify(window,'angle 95.125 deg')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        authored=json.loads(path.read_bytes())['items'][0]['data']
        feature=next(n for n in authored['cad']['nodes'] if n['type']=='revolve')
        assert feature['id'] in authored['cad']['rootNodeIds'] and authored['tree']['orderedNodeIds'][-1]==feature['id']
        assert feature['axis']=={'a':{'x':-2.5,'y':10},'b':{'x':2.5,'y':-10}}
        assert feature['angle']==95.125
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        ref=Path(tmp)/'reference-authored-revolve.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        compare_solid_packages(path.with_suffix('.3mf'),ref)
        key('Escape')
        app.wait(3)

        # A full-turn cut consumes the block. Author a valid partial cut directly.
        path=Path(tmp)/'Partial revolve cut.pcad'
        drawing=revolve_document()
        part=drawing['items'][0]['data']
        graph=part['cad']
        nodes={node['id']:node for node in graph['nodes']}
        nodes['native-sketch-2']['targetId']='plane-top'
        nodes['native-entity-1'].update(p0={'x':12,'y':2},p1={'x':17,'y':8})
        nodes['native-extrude-1']['depth']=8
        nodes['native-entity-1-2'].update(p0={'x':8,'y':0},p1={'x':24,'y':10})
        graph['nodes'].remove(nodes['native-revolve-1'])
        graph['rootNodeIds'].remove('native-revolve-1')
        part['tree']['orderedNodeIds'].remove('native-revolve-1')
        graph['nodes'].append(dict(id='revolve-axis',type='sketchLine',sketchId='native-sketch-2',construction=True,
                                   p0={'x':0,'y':0},p1={'x':0,'y':6}))
        path.write_text(json.dumps(drawing))
        original=path.read_bytes()
        app,window=open_app(path)
        key('F12')
        verify(window,'Editing Sketch 1')
        key('Tab')
        verify(window,'Sketch 2')
        key('8')
        key('y')
        verify(window,'Revolve preview / New body / 360 deg')
        key('Escape')
        verify(window,'Revolve cancelled')
        assert path.read_bytes()==original
        key('8')
        click(window,480,346)  # Pick the construction line as the axis.
        verify(window,'Revolve preview / New body / 360 deg')
        key('o')
        verify(window,'Revolve preview / Cut / 360 deg / Boolean result is empty')
        key('f')
        verify(window,'360 deg / Flipped / Boolean result is empty')
        key('a')
        type_value('15')
        verify(window,'Revolve preview / Cut / 15 deg / Flipped')
        assert path.read_bytes()==original
        click(window,840,55)
        verify(window,'Revolved')
        key('Control_L+s')
        verify(window,'Saved')
        key('Escape')
        app.wait(3)
        saved=json.loads(path.read_bytes())
        feature=next(node for node in saved['items'][0]['data']['cad']['nodes'] if node['type']=='revolve')
        assert (feature['sketchId'],feature['angle'],feature['operation'])==('native-sketch-2',15,'cut')
        assert feature['axis']=={'a':{'x':0,'y':6},'b':{'x':0,'y':0}}
        app,window=open_app(path)
        key('Control_L+Alt_L+e')
        verify(window,'Exported')
        ref=Path(tmp)/'reference-partial-revolve-cut.3mf'
        subprocess.run([root/'rust/target/debug/puppycad','--export',path,ref],check=True)
        compare_solid_packages(path.with_suffix('.3mf'),ref)
        assert math.isclose(package_geometry(ref)[1][0][1][1],123.4421,abs_tol=.001)
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
        verify(window, 'Extrusion preview')
        key('Return')
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
        verify(window, 'Choose a plane for the new sketch')
        subprocess.run(['magick','import','-window',hex(window),output_dir/'new-sketch-on-part.png'],check=True)
        click(window, 420, 55)
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
        # A legacy TypeScript bevel keeps the authored extrusion-edge reference.
        path=Path(tmp)/"Legacy bevel.pcad"
        document=json.loads((libs/"cad/tests/fixtures/typescript-chamfer.pcad").read_bytes())
        nodes=document["items"][0]["data"]["cad"]["nodes"]
        bevel=next(node for node in nodes if node["type"]=="chamfer")
        bevel["labelData"]={"note":"Preserve the original selector"}
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"legacy-chamfer-input.pcad")
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Unequal chamfer  d1 2 mm")
        key("F2")
        verify(window,"Set value")
        type_value("4")
        key("Return")
        verify(window,"Unequal chamfer  d1 4 mm")
        key("Tab")
        verify(window,"Unequal chamfer  d2 3 mm")
        key("F2")
        type_value("1")
        key("Return")
        verify(window,"Unequal chamfer  d2 1 mm")
        key("Control_L+z")
        verify(window,"Unequal chamfer  d2 3 mm")
        key("Control_L+y")
        verify(window,"Unequal chamfer  d2 1 mm")
        key("F2")
        type_value("-1")
        key("Return")
        verify(window,"Chamfer distances must be positive")
        key("Escape")
        verify(window,"Edit cancelled")
        key("F2")
        type_value("0")
        key("Return")
        verify(window,"Unequal chamfer  d2 4 mm")
        key("Control_L+s")
        verify(window,"Saved")
        inherited=next(node for node in json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"] if node["id"]==bevel["id"])
        assert "d2" not in inherited
        key("F2")
        type_value("0")
        key("Return")
        verify(window,"Value unchanged")
        key("Control_L+z")
        verify(window,"Unequal chamfer  d2 1 mm")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][0]["data"]["cad"]["nodes"]==[
            dict(node,d1=4,d2=1) if node is bevel else node for node in nodes
        ]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Unequal chamfer  d1 4 mm")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"legacy-chamfer.png"],check=True)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        bounds,volume=triangulation(stl)
        assert bounds==[(0.0,20.0),(0.0,10.0),(-20.0,0.0)] and math.isclose(volume,3960,abs_tol=.01)
        from struct import iter_unpack
        vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
        for x in (0,20):
            for expected in ((x,6,0),(x,10,-1)):
                assert any(math.dist(vertex,expected)<.0001 for vertex in vertices), expected
        shutil.copyfile(path,output_dir/"legacy-chamfer-edited.pcad")
        shutil.copyfile(stl,output_dir/"legacy-chamfer.stl")
        key("Escape")
        app.wait(3)

        # A rotated source keeps TypeScript's original edge numbering.
        path=Path(tmp)/"Rotated chamfer.pcad"
        document=json.loads((libs/"cad/tests/fixtures/typescript-convex-chamfers.pcad").read_bytes())
        document["items"]=[next(item for item in document["items"] if item["id"]=="rotated-rectangle")]
        nodes=document["items"][0]["data"]["cad"]["nodes"]
        bevel=next(node for node in nodes if node["type"]=="chamfer")
        bevel["notes"]={"keep":"original TS edge reference"}
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"rotated-chamfer-input.pcad")
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Unequal chamfer  d1 2 mm")
        key("F2")
        type_value("4")
        key("Return")
        verify(window,"Unequal chamfer  d1 4 mm")
        key("Tab")
        verify(window,"Unequal chamfer  d2 3 mm")
        key("F2")
        type_value("1")
        key("Return")
        verify(window,"Unequal chamfer  d2 1 mm")
        key("Control_L+z")
        verify(window,"Unequal chamfer  d2 3 mm")
        key("Control_L+y")
        verify(window,"Unequal chamfer  d2 1 mm")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        assert saved["items"][0]["data"]["cad"]["nodes"]==[
            dict(node,d1=4,d2=1) if node is bevel else node for node in nodes
        ]
        shutil.copyfile(path,output_dir/"rotated-chamfer-edited.pcad")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Unequal chamfer  d1 4 mm")
        click(window,735,451)
        key("4")
        verify(window,"Back view")
        key("Up")
        key("Up")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"rotated-chamfer.png"],check=True)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        _,volume=triangulation(stl)
        assert math.isclose(volume,3312,abs_tol=.001),volume
        from struct import iter_unpack
        vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
        # The unequal bevel is on the positive local-height edge of the rectangle.
        for expected in ((-7.892305,-1.937822,6),(12.892305,10.062178,6),
                         (-7.392305,-2.803848,10),(13.392305,9.196152,10)):
            assert any(math.dist(vertex,expected)<.00001 for vertex in vertices),expected
        shutil.copyfile(stl,output_dir/"rotated-chamfer.stl")
        key("Escape")
        app.wait(3)

        # A top-cap chamfer follows the attached source even on a reversed parent sketch.
        path=Path(tmp)/"Face chamfer.pcad"
        document=json.loads((libs/"cad/tests/fixtures/typescript-face-chamfers.pcad").read_bytes())
        document["items"]=[next(item for item in document["items"] if item["id"]=="top-cap")]
        nodes=document["items"][0]["data"]["cad"]["nodes"]
        bevel=next(node for node in nodes if node["type"]=="chamfer")
        bevel["notes"]={"keep":"original face and edge IDs"}
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"face-chamfer-input.pcad")
        app,window=open_app(path,design_tree=True)
        click(window,101,316)
        verify(window,"Attached bevel  d1 2 mm")
        key("F2")
        type_value("4")
        key("Return")
        verify(window,"Attached bevel  d1 4 mm")
        key("Tab")
        verify(window,"Attached bevel  d2 3 mm")
        key("F2")
        type_value("1")
        key("Return")
        verify(window,"Attached bevel  d2 1 mm")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        assert saved["items"][0]["data"]["cad"]["nodes"]==[
            dict(node,d1=4,d2=1) if node is bevel else node for node in nodes
        ]
        shutil.copyfile(path,output_dir/"face-chamfer-edited.pcad")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        click(window,101,316)
        verify(window,"Attached bevel  d1 4 mm")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"face-chamfer.png"],check=True)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        bounds,volume=triangulation(stl)
        assert bounds==[(0,20),(0,20),(0,14)] and math.isclose(volume,3240,abs_tol=.001),(bounds,volume)
        from struct import iter_unpack
        vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
        for expected in ((4,5,10),(16,5,10),(4,6,14),(16,6,14)):
            assert any(math.dist(vertex,expected)<.00001 for vertex in vertices),expected
        shutil.copyfile(stl,output_dir/"face-chamfer.stl")
        key("Escape")
        app.wait(3)

        # A closed line outline keeps the authored starting segment after winding normalization.
        path=Path(tmp)/"Line chamfer.pcad"
        document=json.loads((libs/"cad/tests/fixtures/typescript-line-chamfers.pcad").read_bytes())
        document["items"]=[next(item for item in document["items"] if item["id"]=="ordered-square")]
        nodes=document["items"][0]["data"]["cad"]["nodes"]
        bevel=next(node for node in nodes if node["type"]=="chamfer")
        bevel["notes"]={"keep":"original line and edge order"}
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"line-chamfer-input.pcad")
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Line bevel  d1 2 mm")
        key("F2")
        type_value("4")
        key("Return")
        verify(window,"Line bevel  d1 4 mm")
        key("Tab")
        verify(window,"Line bevel  d2 3 mm")
        key("F2")
        type_value("1")
        key("Return")
        verify(window,"Line bevel  d2 1 mm")
        key("Control_L+z")
        verify(window,"Line bevel  d2 3 mm")
        key("Control_L+y")
        verify(window,"Line bevel  d2 1 mm")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        assert saved["items"][0]["data"]["cad"]["nodes"]==[
            dict(node,d1=4,d2=1) if node is bevel else node for node in nodes
        ]
        shutil.copyfile(path,output_dir/"line-chamfer-edited.pcad")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Line bevel  d1 4 mm")
        key("1")
        verify(window,"Front view")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"line-chamfer.png"],check=True)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        bounds,volume=triangulation(stl)
        assert bounds==[(0,20),(0,20),(0,10)] and math.isclose(volume,3960,abs_tol=.001),(bounds,volume)
        from struct import iter_unpack
        vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
        for expected in ((20,0,6),(20,20,6),(19,0,10),(19,20,10)):
            assert any(math.dist(vertex,expected)<.00001 for vertex in vertices),expected
        shutil.copyfile(stl,output_dir/"line-chamfer.stl")
        key("Escape")
        app.wait(3)

        # A bevel on the outer rim of an L profile preserves both sides of the notch.
        path=Path(tmp)/"Concave chamfer.pcad"
        document=json.loads((libs/"cad/tests/fixtures/typescript-concave-chamfers.pcad").read_bytes())
        document["items"]=[next(item for item in document["items"] if item["id"]=="outer-top-rim")]
        nodes=document["items"][0]["data"]["cad"]["nodes"]
        bevel=next(node for node in nodes if node["type"]=="chamfer")
        bevel["notes"]={"keep":"saved L-profile selector"}
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"concave-chamfer-input.pcad")
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Line bevel  d1 2 mm")
        key("F2")
        type_value("4")
        key("Return")
        verify(window,"Line bevel  d1 4 mm")
        key("Tab")
        verify(window,"Line bevel  d2 3 mm")
        key("F2")
        type_value("1")
        key("Return")
        verify(window,"Line bevel  d2 1 mm")
        key("Control_L+z")
        verify(window,"Line bevel  d2 3 mm")
        key("Control_L+y")
        verify(window,"Line bevel  d2 1 mm")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        assert saved["items"][0]["data"]["cad"]["nodes"]==[
            dict(node,d1=4,d2=1) if node is bevel else node for node in nodes
        ]
        shutil.copyfile(path,output_dir/"concave-chamfer-edited.pcad")
        key("Escape")
        app.wait(3)
        app,window=open_app(path,design_tree=True)
        click(window,101,264)
        verify(window,"Line bevel  d1 4 mm")
        key("5")
        verify(window,"Left view")
        click(window,735,451)
        for stroke in ("Up","Up","Left","Left"):
            key(stroke)
        subprocess.run(["magick","import","-window",hex(window),output_dir/"concave-chamfer.png"],check=True)
        key("Control_L+e")
        verify(window,"Exported")
        stl=path.with_suffix(".stl")
        bounds,volume=triangulation(stl)
        assert bounds==[(0,20),(0,20),(0,10)] and math.isclose(volume,2750,abs_tol=.001),(bounds,volume)
        from struct import iter_unpack
        vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
        for expected in ((0,0,6),(0,20,6),(1,0,10),(1,20,10),(9,9,0),(9,9,10)):
            assert any(math.dist(vertex,expected)<.00001 for vertex in vertices),expected
        shutil.copyfile(stl,output_dir/"concave-chamfer.stl")
        key("Escape")
        app.wait(3)

        # The saved clockwise rim keeps its facet with one or several apertures.
        aperture_cases=(
            ("typescript-hole-chamfers.pcad","circular-aperture-rim","hole-chamfer",264,[(0,20),(0,20),(0,10)],3484.173166,
             ((5.847759,9.234633,9.25),(5.414214,8.585786,9.25),(14.363906,20,10),(0,20,1.856942))),
            ("typescript-multiple-apertures.pcad","mixed-direct-apertures","multiple-aperture-chamfer",264,[(0,20),(0,20),(0,10)],3437.651029,
             ((10.427106,14.400705,9.25),(6.745087,12.837780,9.25),(20,17.269161,10),(20,20,8.286079))),
            ("typescript-traced-apertures.pcad","mixed-traced-apertures","traced-aperture-chamfer",264,[(0,20),(0,20),(0,10)],2169.047976,
             ((6.5,14.1,9.25),(7.25,17.6,9.25),(6.639314,20,10),(20,20,1.092652))),
            ("typescript-pierced-support-chamfers.pcad","pierced-aperture-side","pierced-support-chamfer",316,[(0,30),(0,24),(0,6)],3889.030757,
             ((22,13.25,1),(22,12.5,2.1),(18,12.5,2.1),(18,13.25,1))),
        )
        for fixture,item_id,artifact,row_y,expected_bounds,expected_volume,expected_vertices in aperture_cases:
            path=Path(tmp)/f"{artifact}.pcad"
            document=json.loads((libs/"cad/tests/fixtures"/fixture).read_bytes())
            document["items"]=[next(item for item in document["items"] if item["id"]==item_id)]
            nodes=document["items"][0]["data"]["cad"]["nodes"]
            bevel=next(node for node in nodes if node["type"]=="chamfer")
            name=bevel["name"]
            bevel["notes"]={"keep":"saved clockwise aperture selector"}
            path.write_text(json.dumps(document))
            shutil.copyfile(path,output_dir/f"{artifact}-input.pcad")
            app,window=open_app(path,design_tree=True)
            click(window,101,row_y)
            for field,previous,value in (("d1","0.5","0.75"),("d2","1.25","1.1")):
                if field=="d2":
                    key("Tab")
                verify(window,f"{name}  {field} {previous} mm")
                key("F2")
                type_value(value)
                key("Return")
                verify(window,f"{name}  {field} {value} mm")
            key("Control_L+z")
            verify(window,f"{name}  d2 1.25 mm")
            key("Control_L+y")
            verify(window,f"{name}  d2 1.1 mm")
            key("Control_L+s")
            verify(window,"Saved")
            saved=json.loads(path.read_bytes())
            assert saved["items"][0]["data"]["cad"]["nodes"]==[
                dict(node,d1=0.75,d2=1.1) if node is bevel else node for node in nodes
            ]
            shutil.copyfile(path,output_dir/f"{artifact}-edited.pcad")
            key("Escape")
            app.wait(3)
            app,window=open_app(path,design_tree=True)
            click(window,101,row_y)
            verify(window,f"{name}  d1 0.75 mm")
            key("3")
            verify(window,"Top view")
            click(window,735,451)
            for stroke in ("Down","Down","Down","Left","Left"):
                key(stroke)
            subprocess.run(["magick","import","-window",hex(window),output_dir/f"{artifact}.png"],check=True)
            key("Control_L+e")
            verify(window,"Exported")
            stl=path.with_suffix(".stl")
            bounds,volume=triangulation(stl)
            assert bounds==expected_bounds and math.isclose(volume,expected_volume,abs_tol=.003),(bounds,volume)
            from struct import iter_unpack
            vertices={tuple(face[i:i+3]) for face in iter_unpack("<12fH",stl.read_bytes()[84:]) for i in (3,6,9)}
            for expected in expected_vertices:
                assert any(math.dist(vertex,expected)<.0001 for vertex in vertices),expected
            shutil.copyfile(stl,output_dir/f"{artifact}.stl")
            key("Escape")
            app.wait(3)

        # Replacing a part keeps the occurrence and its two mate endpoints.
        path=Path(tmp)/"Replace occurrence.pcad"
        document=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        slim=json.loads(json.dumps(document["items"][1]))
        slim.update(id="slim-arm",name="Slim arm")
        outline=next(node for node in slim["data"]["cad"]["nodes"] if node["type"]=="sketchCornerRectangle")
        outline["p0"]["x"]=-3
        outline["p1"]["x"]=3
        document["items"].insert(2,slim)
        document["selectedPath"]=[3]
        assembly=document["items"][3]["data"]
        assembly["instances"][1]["transform"]["memo"]="Keep the authored pose"
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"replace-occurrence-input.pcad")
        app,window=open_app(path,"Assembly opened",design_tree=True)
        click(window,105,317,button=3)
        verify(window,"Selected Upper arm")
        click(window,134,332)
        verify(window,"Choose a replacement for Upper arm")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"replace-occurrence-chooser.png"],check=True)
        key("Escape")
        verify(window,"Part replacement cancelled")
        assert json.loads(path.read_bytes())==document
        click(window,108,615)  # Open the chooser from the selected occurrence details.
        verify(window,"Choose a replacement for Upper arm")
        focus_search(window,76,design=True)
        type_value("slim")
        key("Return")
        click(window,84,160)
        verify(window,"Part replaced with Slim arm")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"]==document["items"]
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        assembly["instances"][1]["partId"]="slim-arm"
        assert saved["items"]==document["items"] and saved["selectedPath"]==[3]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened",design_tree=True)
        click(window,105,317)
        verify(window,"Selected Upper arm")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"replace-occurrence.png"],check=True)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"reference-replaced-occurrence.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        _,bodies=package_geometry(path.with_suffix(".3mf"))
        assert bodies[1][1][0]==[(-3.0,3.0),(-20.0,20.0),(10.0,15.0)]
        assert math.isclose(bodies[1][1][1],1200,rel_tol=1e-6)
        key("Control_L+e")
        verify(window,"Exported")
        shutil.copyfile(path,output_dir/"replace-occurrence-edited.pcad")
        for stl in path.parent.glob("Replace occurrence*.stl"):
            shutil.copyfile(stl,output_dir/stl.name)
        key("Escape")
        app.wait(3)
        # Editing a connector moves the attached arm and its hinged follower.
        path=Path(tmp)/"Assembly connector.pcad"
        document=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        document["selectedPath"]=[2]
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"assembly-connector-frame-input.pcad")
        original=document["items"][2]["data"]
        app,window=open_app(path,"Assembly opened",design_tree=True)
        click(window,76,395)  # Connectors group.
        click(window,88,421,button=3)
        verify(window,"Selected Base top")
        click(window,120,434)  # Edit connector opens the local X field.
        verify(window,"Set value")
        key("Escape")
        verify(window,"Edit cancelled")
        click(window,210,588)  # Local position Z.
        verify(window,"Set value")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"assembly-connector-value.png"],check=True)
        type_value("25")
        key("Return")
        verify(window,"Edited")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][2]["data"]==original
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        assembly=saved["items"][2]["data"]
        assert saved["items"][:2]==document["items"][:2]
        assert assembly["connectors"]==[dict(original["connectors"][0],position=dict(x=0,y=0,z=25)),*original["connectors"][1:]]
        assert assembly["mates"]==original["mates"] and assembly["actuators"]==original["actuators"]
        assert [item.get("transform",{}).get("translation",{}).get("z",0) for item in assembly["instances"]]==[0,25,30]
        click(window,210,624)  # Local rotation Z.
        verify(window,"Set value")
        type_value("90")
        key("Return")
        verify(window,"Edited")
        key("Control_L+s")
        verify(window,"Saved")
        rotated=json.loads(path.read_bytes())["items"][2]["data"]
        assert rotated["connectors"][0]["rotation"]==dict(x=0,y=0,z=90)
        assert rotated["instances"][1]["transform"]["rotation"]["z"]==90
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"]==saved["items"]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened",design_tree=True)
        click(window,76,395)
        click(window,88,421)
        verify(window,"Selected Base top")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"assembly-connector-frame.png"],check=True)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"reference-connector.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        key("Control_L+e")
        verify(window,"Exported")
        shutil.copyfile(path,output_dir/"assembly-connector-frame-edited.pcad")
        shutil.copyfile(path.with_suffix(".3mf"),output_dir/"assembly-connector-frame.3mf")
        for stl in path.parent.glob("Assembly connector*.stl"):
            shutil.copyfile(stl,output_dir/stl.name)
        key("Escape")
        app.wait(3)
        # Rename a v4 connector and reattach it to World without losing its mate.
        path=Path(tmp)/"Connector ownership.pcad"
        document=json.loads((root/"rust/fixtures/assembly.pcad").read_bytes())
        document["selectedPath"]=[2]
        data=document["items"][2]["data"]
        data["instances"][0]["transform"]=dict(translation=dict(x=15,y=0,z=0))
        path.write_text(json.dumps(document))
        shutil.copyfile(path,output_dir/"connector-ownership-input.pcad")
        app,window=open_app(path,"Assembly opened",design_tree=True)
        click(window,76,395)  # Connectors group.
        click(window,88,421,button=3)
        verify(window,"Selected Base top")
        click(window,120,460)  # Rename ID.
        verify(window,"Set value")
        type_value("mount-datum")
        key("Return")
        verify(window,"Connector ID changed to mount-datum")
        key("Control_L+s")
        verify(window,"Saved")
        renamed=json.loads(path.read_bytes())["items"][2]["data"]
        data["connectors"][0]["id"]="mount-datum"
        data["mates"][0]["a"]["connectorId"]="mount-datum"
        assert renamed==data
        click(window,88,421,button=3)
        click(window,120,486)  # Attach to.
        verify(window,"Attach Base top to an occurrence or World")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"connector-attach-chooser.png"],check=True)
        click(window,75,187)  # Upper arm already owns the other endpoint of this mate.
        verify(window,"Mate needs distinct instances")
        assert json.loads(path.read_bytes())["items"][2]["data"]==renamed
        click(window,75,161)  # World.
        verify(window,"Connector attached to World")
        key("Control_L+z")
        key("Control_L+s")
        verify(window,"Saved")
        assert json.loads(path.read_bytes())["items"][2]["data"]==renamed
        key("Control_L+y")
        key("Control_L+s")
        verify(window,"Saved")
        saved=json.loads(path.read_bytes())
        data["connectors"][0]["instanceId"]=None
        data["mates"][0]["a"]["instanceId"]=None
        assert saved["items"]==document["items"] and saved["selectedPath"]==[2]
        key("Escape")
        app.wait(3)
        app,window=open_app(path,"Assembly opened",design_tree=True)
        click(window,76,395)
        click(window,88,421)
        verify(window,"Selected Base top")
        subprocess.run(["magick","import","-window",hex(window),output_dir/"connector-attached-to-world.png"],check=True)
        key("Control_L+Alt_L+e")
        verify(window,"Exported")
        reference=Path(tmp)/"reference-connector-ownership.3mf"
        subprocess.run([root/"rust/target/debug/puppycad","--export",path,reference],check=True)
        compare_solid_packages(path.with_suffix(".3mf"),reference)
        _,bodies=package_geometry(path.with_suffix(".3mf"))
        assert bodies[0][1][0][0]==(-5.0,35.0) and bodies[1][1][0][0]==(-5.0,5.0)
        key("Control_L+e")
        verify(window,"Exported")
        shutil.copyfile(path,output_dir/"connector-ownership-edited.pcad")
        for stl in path.parent.glob("Connector ownership*.stl"):
            shutil.copyfile(stl,output_dir/stl.name)
        key("Escape")
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
