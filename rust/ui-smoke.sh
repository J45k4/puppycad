#!/bin/sh
set -eu

cargo test --workspace

Xvfb :99 -screen 0 1280x800x24 -noreset >/tmp/puppycad-xvfb.log 2>&1 &
xvfb_pid=$!
app_pid=""


cleanup() {
    if [ -n "$app_pid" ]; then
        kill "$app_pid" 2>/dev/null || true
        wait "$app_pid" 2>/dev/null || true
    fi
    kill "$xvfb_pid" 2>/dev/null || true
    wait "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT

export DISPLAY=:99
tries=0
until xdpyinfo >/dev/null 2>&1; do
    tries=$((tries + 1))
    if [ "$tries" -ge 30 ]; then
        cat /tmp/puppycad-xvfb.log
        exit 1
    fi
    sleep 0.1
done

./target/debug/puppycad /tmp/native-smoke.pcad >/tmp/puppycad-ui.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then
        cat /tmp/puppycad-ui.log
        exit 1
    fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then
        cat /tmp/puppycad-ui.log
        exit 1
    fi
    sleep 0.1
done

window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1

# Draw a rectangle, circle, and line in the native sketch viewport.
xdotool mousemove 175 63 click 1
xdotool mousemove 490 250 click 1
xdotool mousemove 710 435 click 1
xdotool mousemove 310 63 click 1
xdotool mousemove 755 320 click 1
xdotool mousemove 820 380 click 1
xdotool mousemove 430 63 click 1
xdotool mousemove 455 510 click 1
xdotool mousemove 630 570 click 1

# Resize two shapes through the properties panel before saving.
xdotool mousemove 90 227 click 1
xdotool mousemove 1100 215 click 1
xdotool type --clearmodifiers '100'
xdotool key Return
xdotool mousemove 90 262 click 1
xdotool mousemove 1100 215 click 1
xdotool type --clearmodifiers '240'
xdotool key Return
# Add an ellipse and edit its width through the same Properties workflow.
xdotool mousemove 550 63 click 1
xdotool mousemove 550 365 click 1
xdotool mousemove 610 335 click 1
xdotool mousemove 530 410 click 1
xdotool mousemove 1100 215 click 1
xdotool type --clearmodifiers '150'
xdotool key Return
mkdir -p /out
import -window root /out/rust-native-ellipse.png
# The sketch header shows the solver DOF count (unconstrained shapes here).
import -window root -crop 1280x60+0+0 /out/rust-native-dof-header.png
xdotool key --clearmodifiers ctrl+d
import -window root /out/rust-native-dimension-copy.png
# Keep unsaved geometry in the first sketch while adding Front and Right sketches.
xdotool mousemove 795 63 click 1
xdotool mousemove 430 63 click 1
xdotool mousemove 690 490 click 1
xdotool mousemove 810 540 click 1
xdotool mousemove 915 63 click 1
xdotool mousemove 175 63 click 1
xdotool mousemove 730 270 click 1
xdotool mousemove 850 400 click 1
xdotool mousemove 1035 63 click 1
xdotool mousemove 690 610 click 1
xdotool mousemove 870 610 click 1
xdotool mousemove 780 530 click 1
xdotool mousemove 1150 63 click 1
import -window root /out/rust-native-tools-menu.png
xdotool mousemove 1150 110 click 1
xdotool mousemove 400 650 click 1
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 145 click 1
xdotool mousemove 450 400 click 1
xdotool mousemove 500 380 click 1
xdotool mousemove 1100 215 click 1
xdotool type --clearmodifiers '5'
xdotool key Return
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 180 click 1
xdotool mousemove 560 500 click 1
xdotool mousemove 640 500 click 1
xdotool mousemove 600 540 click 1
xdotool mousemove 1100 215 click 1
xdotool type --clearmodifiers '100'
xdotool key Return
xdotool mousemove 75 63 click 1
xdotool mousemove 600 500 mousedown 1
xdotool mousemove --sync 630 470
xdotool mouseup 1
xdotool key --window "$window_id" ctrl+z
xdotool key --window "$window_id" ctrl+y
xdotool mousemove 80 405 click 1
xdotool mousemove 1100 470 click 1
xdotool type --clearmodifiers --delay 50 -- '-20'
xdotool key Return
xdotool mousemove 1100 500 click 1
xdotool type --clearmodifiers --delay 50 -- '15'
xdotool key Return
xdotool mousemove 1100 535 click 1
xdotool mousemove 1100 575 click 1
import -window root /out/rust-native-copy.png
xdotool mousemove 1100 655 click 1
xdotool type --clearmodifiers --delay 50 -- '-50'
xdotool key Return
xdotool mousemove 1100 685 click 1
xdotool type --clearmodifiers '0.5'
xdotool key Return
xdotool mousemove 1100 745 click 1
import -window root /out/rust-native-scale.png
xdotool key --window "$window_id" ctrl+z
xdotool key --window "$window_id" ctrl+y
xdotool mousemove 80 264 click 1
xdotool mousemove 1190 626 click 1
xdotool mousemove 1100 655 click 1
xdotool type --clearmodifiers '100'
xdotool key Return
xdotool mousemove 1190 655 click 1
xdotool type --clearmodifiers '100'
xdotool key Return
xdotool mousemove 1100 685 click 1
xdotool type --clearmodifiers '30'
xdotool key Return
xdotool mousemove 1100 715 click 1
import -window root /out/rust-native-rotate.png
xdotool mousemove 100 153 click 1
xdotool key --window "$window_id" ctrl+s
sleep 1

if ! kill -0 "$app_pid" 2>/dev/null; then
    cat /tmp/puppycad-ui.log
    exit 1
fi
if [ ! -s /tmp/native-smoke.pcad ]; then
    cat /tmp/puppycad-ui.log
    import -window root /out/rust-native-save-failed.png || true
    echo "The simulated save did not create the project file" >&2
    exit 1
fi
mkdir -p /out
import -window root /out/rust-native-container.png
cp /tmp/native-smoke.pcad /out/rust-native-smoke.pcad
grep -q '"sketchCornerRectangle"' /tmp/native-smoke.pcad
grep -q '"sketchPrimitive"' /tmp/native-smoke.pcad
grep -q '"sketchLine"' /tmp/native-smoke.pcad
grep -q '"sketchArc"' /tmp/native-smoke.pcad
grep -q '"type": "point"' /tmp/native-smoke.pcad
grep -q '"type": "polygon"' /tmp/native-smoke.pcad
grep -q '"type": "capsule"' /tmp/native-smoke.pcad
grep -q '"rotation": 30.0' /tmp/native-smoke.pcad
grep -q '"id": "native-entity-6"' /tmp/native-smoke.pcad
grep -q '"id": "native-entity-7"' /tmp/native-smoke.pcad
grep -q '"type": "ellipse"' /tmp/native-smoke.pcad
grep -q '"type": "diameter"' /tmp/native-smoke.pcad
grep -q '"type": "length"' /tmp/native-smoke.pcad
grep -q '"type": "width"' /tmp/native-smoke.pcad
grep -q '"id": "native-dimension-native-entity-5-width"' /tmp/native-smoke.pcad
grep -q '"id": "native-sketch-2"' /tmp/native-smoke.pcad
grep -q '"sketchId": "native-sketch-2"' /tmp/native-smoke.pcad
grep -q '"targetId": "plane-front"' /tmp/native-smoke.pcad
grep -q '"id": "native-sketch-3"' /tmp/native-smoke.pcad
grep -q '"sketchId": "native-sketch-3"' /tmp/native-smoke.pcad
grep -q '"targetId": "plane-right"' /tmp/native-smoke.pcad
xdotool mousemove 100 222 click 1
sleep 0.2
import -window root /out/rust-native-container.png
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Open a web-compatible single-profile extrusion in the isolated display.
cp ./fixtures/extruded-rectangle.pcad /tmp/native-existing-solid.pcad
./target/debug/puppycad /tmp/native-existing-solid.pcad >/tmp/puppycad-solid.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then
        cat /tmp/puppycad-solid.log
        exit 1
    fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then
        cat /tmp/puppycad-solid.log
        exit 1
    fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
import -window root /out/rust-native-solid.png
xdotool mousemove 1100 215 click 1
xdotool type --clearmodifiers '80'
xdotool key Return
sleep 0.2
import -window root /out/rust-native-solid-depth.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
cp /tmp/native-existing-solid.pcad /out/rust-native-depth-edited.pcad
grep -q '"depth": 80.0' /tmp/native-existing-solid.pcad
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-existing-solid.stl
test "$(grep -c '^facet normal' /tmp/native-existing-solid.stl)" -eq 12
cp /tmp/native-existing-solid.stl /out/rust-native-export.stl
import -window root /out/rust-native-stl-export.png
xdotool mousemove 600 380 mousedown 1
xdotool mousemove --sync 790 425
xdotool mouseup 1
sleep 0.2
import -window root /out/rust-native-solid-orbit.png
xdotool key --clearmodifiers Tab
sleep 0.2
import -window root /out/rust-native-solid-sketch.png
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Reopen web primitives with nondefault tessellation and inspect their solid.
cp ./fixtures/segmented-primitives.pcad /tmp/native-segmented.pcad
./target/debug/puppycad /tmp/native-segmented.pcad >/tmp/puppycad-segmented.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-segmented.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-segmented.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
import -window root /out/rust-native-segmented-solid.png
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-segmented.stl
cp /tmp/native-segmented.stl /out/rust-native-segmented-solid.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Reopen a web slot with nondefault arc segmentation.
cp ./fixtures/segmented-slot.pcad /tmp/native-segmented-slot.pcad
./target/debug/puppycad /tmp/native-segmented-slot.pcad >/tmp/puppycad-segmented-slot.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-segmented-slot.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-segmented-slot.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
import -window root /out/rust-native-segmented-slot.png
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-segmented-slot.stl
cp /tmp/native-segmented-slot.stl /out/rust-native-segmented-slot.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Switch between two parts, preserving their unsaved sketches.
cp ./fixtures/two-parts.pcad /tmp/native-two-parts.pcad
./target/debug/puppycad /tmp/native-two-parts.pcad >/tmp/puppycad-parts.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-parts.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-parts.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 430 63 click 1
xdotool mousemove 440 520 click 1
xdotool mousemove 520 520 click 1
xdotool key --window "$window_id" k
xdotool mousemove 175 63 click 1
xdotool mousemove 500 300 click 1
xdotool mousemove 700 430 click 1
xdotool key --window "$window_id" ctrl+Page_Down
xdotool mousemove 310 63 click 1
xdotool mousemove 750 320 click 1
xdotool mousemove 810 380 click 1
xdotool key --window "$window_id" k
sleep 0.2
import -window root /out/rust-native-two-parts.png
import -window root /out/rust-native-construction.png
xdotool mousemove 75 63 click 1
xdotool mousemove 420 600 click 1
import -window root /out/rust-native-construction-dimmed.png
xdotool mousemove 100 150 click 1
sleep 0.2
import -window root /out/rust-native-part-list.png
xdotool mousemove 100 185 click 1
sleep 0.2
import -window root /out/rust-native-part-switch.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test "$(grep -c '"type": "sketchCornerRectangle"' /tmp/native-two-parts.pcad)" -eq 1
test "$(grep -c '"type": "sketchPrimitive"' /tmp/native-two-parts.pcad)" -eq 1
grep -q '"construction": true' /tmp/native-two-parts.pcad
grep -q '"selectedPath": \[' /tmp/native-two-parts.pcad
cp /tmp/native-two-parts.pcad /out/rust-native-two-parts.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Create a second top-level part without losing an unsaved first-part sketch.
./target/debug/puppycad /tmp/native-created-part.pcad >/tmp/puppycad-created-part.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-created-part.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-created-part.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 175 63 click 1
xdotool mousemove 500 300 click 1
xdotool mousemove 700 430 click 1
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 252 click 1
sleep 0.2
import -window root /out/rust-native-created-part.png
xdotool mousemove 310 63 click 1
xdotool mousemove 630 360 click 1
xdotool mousemove 660 360 click 1
xdotool key --window "$window_id" F2
xdotool type --clearmodifiers 'Bracket'
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" shift+F2
xdotool type --clearmodifiers 'Round sketch'
xdotool key --window "$window_id" Return
sleep 0.2
import -window root /out/rust-native-renamed-part.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-created-part.pcad
test "$(grep -c '"type": "part"' /tmp/native-created-part.pcad)" -eq 2
grep -q '"name": "Bracket"' /tmp/native-created-part.pcad
grep -q '"name": "Round sketch"' /tmp/native-created-part.pcad
cp /tmp/native-created-part.pcad /out/rust-native-created-part.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Author a native extrusion and persist its graph for web-runtime validation.
./target/debug/puppycad /tmp/native-extruded.pcad >/tmp/puppycad-author.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then
        cat /tmp/puppycad-author.log
        exit 1
    fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then
        cat /tmp/puppycad-author.log
        exit 1
    fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 430 63 click 1
xdotool mousemove 440 520 click 1
xdotool mousemove 520 520 click 1
xdotool key --window "$window_id" k
xdotool mousemove 175 63 click 1
xdotool mousemove 500 300 click 1
xdotool mousemove 700 430 click 1
xdotool mousemove 430 63 click 1
xdotool mousemove 760 510 click 1
xdotool mousemove 830 510 click 1
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 110 click 1
xdotool mousemove 800 590 click 1
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 215 click 1
sleep 0.3
import -window root /out/rust-native-authored-solid.png
xdotool key --clearmodifiers ctrl+s
sleep 0.3
test -s /tmp/native-extruded.pcad
cp /tmp/native-extruded.pcad /out/rust-native-authored-solid.pcad
grep -q '"type": "extrude"' /tmp/native-extruded.pcad
grep -q '"construction": true' /tmp/native-extruded.pcad
grep -q '"type": "point"' /tmp/native-extruded.pcad
test "$(grep -c '"type": "sketchLine"' /tmp/native-extruded.pcad)" -eq 2
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Extrude a closed loop made from four native line entities.
./target/debug/puppycad /tmp/native-line-loop.pcad >/tmp/puppycad-line-loop.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-line-loop.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-line-loop.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 430 63 click 1
xdotool mousemove 500 300 click 1
xdotool mousemove 700 300 click 1
xdotool mousemove 700 300 click 1
xdotool mousemove 700 430 click 1
xdotool mousemove 700 430 click 1
xdotool mousemove 500 430 click 1
xdotool mousemove 500 430 click 1
xdotool mousemove 500 300 click 1
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-line-loop-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-line-loop.pcad
test "$(grep -c '"type": "sketchLine"' /tmp/native-line-loop.pcad)" -eq 4
cp /tmp/native-line-loop.pcad /out/rust-native-line-loop-solid.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Draw and export a concave closed line profile in the virtual display.
./target/debug/puppycad /tmp/native-concave.pcad >/tmp/puppycad-concave.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-concave.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-concave.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 430 63 click 1
for point in '500 300' '700 300' '700 300' '700 350' '700 350' '570 350' '570 350' '570 430' '570 430' '500 430' '500 430' '500 300'; do
    set -- $point
    xdotool mousemove "$1" "$2" click 1
done
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-concave-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-concave.pcad
test -s /tmp/native-concave.stl
test "$(grep -c '"type": "sketchLine"' /tmp/native-concave.pcad)" -eq 6
cp /tmp/native-concave.pcad /out/rust-native-concave-solid.pcad
cp /tmp/native-concave.stl /out/rust-native-concave-solid.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Draw both an outer loop and a through-hole from native line entities.
./target/debug/puppycad /tmp/native-line-hole.pcad >/tmp/puppycad-line-hole.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-line-hole.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-line-hole.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 430 63 click 1
for point in '500 300' '700 300' '700 300' '700 430' '700 430' '500 430' '500 430' '500 300' '560 340' '620 340' '620 340' '620 390' '620 390' '560 390' '560 390' '560 340'; do
    set -- $point
    xdotool mousemove "$1" "$2" click 1
done
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-line-hole-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-line-hole.pcad
test -s /tmp/native-line-hole.stl
test "$(grep -c '"type": "sketchLine"' /tmp/native-line-hole.pcad)" -eq 8
cp /tmp/native-line-hole.pcad /out/rust-native-line-hole-solid.pcad
cp /tmp/native-line-hole.stl /out/rust-native-line-hole-solid.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Extrude a native slot (the web capsule primitive).
./target/debug/puppycad /tmp/native-slot-solid.pcad >/tmp/puppycad-slot-solid.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-slot-solid.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-slot-solid.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 180 click 1
xdotool mousemove 530 365 click 1
xdotool mousemove 630 365 click 1
xdotool mousemove 580 385 click 1
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-slot-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-slot-solid.pcad
test -s /tmp/native-slot-solid.stl
grep -q '"type": "capsule"' /tmp/native-slot-solid.pcad
cp /tmp/native-slot-solid.pcad /out/rust-native-slot-solid.pcad
cp /tmp/native-slot-solid.stl /out/rust-native-slot-solid.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Close a three-point arc with one line and extrude the semicircular profile.
./target/debug/puppycad /tmp/native-arc-line.pcad >/tmp/puppycad-arc-line.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-arc-line.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-arc-line.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 1035 63 click 1
xdotool mousemove 700 365 click 1
xdotool mousemove 500 365 click 1
xdotool mousemove 600 265 click 1
xdotool mousemove 430 63 click 1
xdotool mousemove 500 365 click 1
xdotool mousemove 700 365 click 1
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-arc-line-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-arc-line.pcad
test -s /tmp/native-arc-line.stl
grep -q '"type": "sketchArc"' /tmp/native-arc-line.pcad
grep -q '"type": "sketchLine"' /tmp/native-arc-line.pcad
cp /tmp/native-arc-line.pcad /out/rust-native-arc-line-solid.pcad
cp /tmp/native-arc-line.stl /out/rust-native-arc-line-solid.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Select the second of two disconnected closed profiles before extruding.
./target/debug/puppycad /tmp/native-two-profiles.pcad >/tmp/puppycad-two-profiles.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-two-profiles.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-two-profiles.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 175 63 click 1
xdotool mousemove 420 300 click 1
xdotool mousemove 520 430 click 1
xdotool mousemove 670 300 click 1
xdotool mousemove 770 430 click 1
xdotool key --window "$window_id" bracketright
sleep 0.2
import -window root /out/rust-native-profile-selection.png
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-second-profile-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-two-profiles.pcad
test -s /tmp/native-two-profiles.stl
grep -q '"profileId": "native-sketch-1-profile-2"' /tmp/native-two-profiles.pcad
xdotool key --window "$window_id" Tab
xdotool key --window "$window_id" bracketleft
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-two-profiles-two-bodies.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test "$(grep -c '"type": "extrude"' /tmp/native-two-profiles.pcad)" -eq 2
grep -q '"profileId": "native-sketch-1-profile-1"' /tmp/native-two-profiles.pcad
cp /tmp/native-two-profiles.pcad /out/rust-native-two-profiles.pcad
cp /tmp/native-two-profiles.stl /out/rust-native-two-profiles.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Check reference-plane extrusion and STL export on both vertical planes.
for plane in front right; do
    ./target/debug/puppycad "/tmp/native-$plane.pcad" >"/tmp/puppycad-$plane.log" 2>&1 &
    app_pid=$!
    tries=0
    until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
        if ! kill -0 "$app_pid" 2>/dev/null; then cat "/tmp/puppycad-$plane.log"; exit 1; fi
        tries=$((tries + 1))
        if [ "$tries" -ge 100 ]; then cat "/tmp/puppycad-$plane.log"; exit 1; fi
        sleep 0.1
    done
    window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
    xdotool windowmove "$window_id" 0 0
    xdotool windowsize "$window_id" 1280 800
    xdotool windowfocus --sync "$window_id"
    sleep 1
    if [ "$plane" = front ]; then xdotool key --window "$window_id" 2; else xdotool key --window "$window_id" 3; fi
    xdotool mousemove 175 63 click 1
    xdotool mousemove 500 300 click 1
    xdotool mousemove 700 430 click 1
    xdotool key --window "$window_id" x
    sleep 0.3
    import -window root "/out/rust-native-$plane-solid.png"
    xdotool key --window "$window_id" ctrl+s
    sleep 0.2
    xdotool key --window "$window_id" ctrl+e
    sleep 0.2
    test -s "/tmp/native-$plane.pcad"
    test -s "/tmp/native-$plane.stl"
    cp "/tmp/native-$plane.pcad" "/out/rust-native-$plane-solid.pcad"
    cp "/tmp/native-$plane.stl" "/out/rust-native-$plane-solid.stl"
    kill "$app_pid"
    wait "$app_pid" 2>/dev/null || true
    app_pid=""
done
# Draw an outer rectangle and two inner circles as one profile with holes.
./target/debug/puppycad /tmp/native-hole.pcad >/tmp/puppycad-hole.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-hole.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-hole.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 175 63 click 1
xdotool mousemove 500 300 click 1
xdotool mousemove 700 430 click 1
xdotool mousemove 310 63 click 1
xdotool mousemove 560 365 click 1
xdotool mousemove 585 365 click 1
xdotool mousemove 640 365 click 1
xdotool mousemove 665 365 click 1
xdotool key --window "$window_id" x
sleep 0.3
import -window root /out/rust-native-hole-solid.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-hole.pcad
test -s /tmp/native-hole.stl
cp /tmp/native-hole.pcad /out/rust-native-hole-solid.pcad
cp /tmp/native-hole.stl /out/rust-native-hole-solid.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""
echo "UI smoke passed; screenshot: /out/rust-native-container.png"

# Author two new-body extrusions on separate reference-plane sketches.
./target/debug/puppycad /tmp/native-two-bodies.pcad >/tmp/puppycad-two-bodies.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-two-bodies.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-two-bodies.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 175 63 click 1
xdotool mousemove 450 300 click 1
xdotool mousemove 610 410 click 1
xdotool key --window "$window_id" x
sleep 0.2
xdotool key --window "$window_id" 2
sleep 0.2
xdotool mousemove 175 63 click 1
xdotool mousemove 650 300 click 1
xdotool mousemove 790 430 click 1
xdotool key --window "$window_id" x
sleep 0.3
xdotool key --window "$window_id" bracketleft
xdotool key --window "$window_id" bracketright
xdotool mousemove 1120 208 click 1
xdotool type --window "$window_id" '75'
xdotool key --window "$window_id" Return
sleep 0.2
xdotool mousemove 100 230 click 1
xdotool mousemove 1120 208 click 1
xdotool type --window "$window_id" '65'
xdotool key --window "$window_id" Return
xdotool mousemove 100 265 click 1
sleep 0.2
import -window root /out/rust-native-two-bodies.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-two-bodies.pcad
test -s /tmp/native-two-bodies.stl
test -s /tmp/native-two-bodies-2.stl
test "$(grep -c '"type": "extrude"' /tmp/native-two-bodies.pcad)" -eq 2
grep -q '"depth": 75.0' /tmp/native-two-bodies.pcad
grep -q '"depth": 65.0' /tmp/native-two-bodies.pcad
cp /tmp/native-two-bodies.pcad /out/rust-native-two-bodies.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Apply an axis constraint to a line, then undo a vertical replacement.
./target/debug/puppycad /tmp/native-axis-line.pcad >/tmp/puppycad-axis-line.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-axis-line.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-axis-line.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 430 340 click 1
xdotool mousemove 630 420 click 1
xdotool key --window "$window_id" h
sleep 0.2
import -window root /out/rust-native-horizontal-line.png
xdotool key --window "$window_id" j
sleep 0.2
import -window root /out/rust-native-vertical-line.png
xdotool key --window "$window_id" ctrl+z
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-axis-line.pcad
grep -q '"type": "horizontal"' /tmp/native-axis-line.pcad
if grep -q '"type": "vertical"' /tmp/native-axis-line.pcad; then exit 1; fi
cp /tmp/native-axis-line.pcad /out/rust-native-axis-line.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Remove a reopened extrusion and edit its now-unlocked sketch.
cp ./fixtures/extruded-rectangle.pcad /tmp/native-delete-solid.pcad
./target/debug/puppycad /tmp/native-delete-solid.pcad >/tmp/puppycad-delete-solid.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-delete-solid.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-delete-solid.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" Delete
sleep 0.2
import -window root /out/rust-native-deleted-solid.png
xdotool key --window "$window_id" c
xdotool mousemove 730 300 click 1
xdotool mousemove 760 300 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
if grep -q '"type": "extrude"' /tmp/native-delete-solid.pcad; then exit 1; fi
grep -q '"type": "sketchPrimitive"' /tmp/native-delete-solid.pcad
cp /tmp/native-delete-solid.pcad /out/rust-native-deleted-solid.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Open web graph constraint nodes and edit the existing length node in place.
cp ./fixtures/graph-constrained-line.pcad /tmp/native-graph-constraint.pcad
./target/debug/puppycad /tmp/native-graph-constraint.pcad >/tmp/puppycad-graph-constraint.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-graph-constraint.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-graph-constraint.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 100 195 click 1
import -window root /out/rust-native-graph-constraint-before.png
xdotool mousemove 1120 208 click 1
xdotool type --window "$window_id" '140'
xdotool key --window "$window_id" Return
sleep 0.2
import -window root /out/rust-native-graph-constraint-after.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
grep -q '"id": "length-1"' /tmp/native-graph-constraint.pcad
grep -q '"value": 140.0' /tmp/native-graph-constraint.pcad
grep -q '"id": "horizontal-1"' /tmp/native-graph-constraint.pcad
cp /tmp/native-graph-constraint.pcad /out/rust-native-graph-constraint.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Construct a circle through three rim points and save it as a web circle primitive.
./target/debug/puppycad /tmp/native-three-point-circle.pcad >/tmp/puppycad-three-point-circle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-three-point-circle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-three-point-circle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+c
xdotool mousemove 623 310 click 1
xdotool mousemove 743 430 click 1
xdotool mousemove 623 550 click 1
sleep 0.2
import -window root /out/rust-native-three-point-circle.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-three-point-circle.pcad
grep -q '"type": "circle"' /tmp/native-three-point-circle.pcad
cp /tmp/native-three-point-circle.pcad /out/rust-native-three-point-circle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Construct a clockwise center arc and save its major sweep.
./target/debug/puppycad /tmp/native-center-arc.pcad >/tmp/puppycad-center-arc.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-center-arc.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-center-arc.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+a
xdotool key --window "$window_id" w
xdotool mousemove 623 430 click 1
xdotool mousemove 743 430 click 1
xdotool mousemove 623 310 click 1
sleep 0.2
import -window root /out/rust-native-center-arc.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-center-arc.pcad
grep -q '"type": "sketchArc"' /tmp/native-center-arc.pcad
cp /tmp/native-center-arc.pcad /out/rust-native-center-arc.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Draw a center rectangle, extrude its closed profile, and persist both features.
./target/debug/puppycad /tmp/native-center-rectangle.pcad >/tmp/puppycad-center-rectangle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-center-rectangle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-center-rectangle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+r
xdotool mousemove 623 430 click 1
xdotool mousemove 723 360 click 1
sleep 0.2
import -window root /out/rust-native-center-rectangle.png
xdotool key --window "$window_id" x
sleep 0.2
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-center-rectangle.pcad
grep -q '"type": "extrude"' /tmp/native-center-rectangle.pcad
cp /tmp/native-center-rectangle.pcad /out/rust-native-center-rectangle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Draw both rotated rectangle placement modes into one editable sketch.
./target/debug/puppycad /tmp/native-three-point-rectangles.pcad >/tmp/puppycad-three-point-rectangles.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-three-point-rectangles.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-three-point-rectangles.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" t
xdotool mousemove 400 400 click 1
xdotool mousemove 470 340 click 1
xdotool mousemove 530 410 click 1
xdotool key --window "$window_id" shift+t
xdotool mousemove 780 430 click 1
xdotool mousemove 830 390 click 1
xdotool mousemove 830 460 click 1
sleep 0.2
import -window root /out/rust-native-three-point-rectangles.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-three-point-rectangles.pcad
test "$(grep -c '"type": "rectangle"' /tmp/native-three-point-rectangles.pcad)" -eq 2
cp /tmp/native-three-point-rectangles.pcad /out/rust-native-three-point-rectangles.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Fit an off-center sketch to the available viewport without changing its geometry.
./target/debug/puppycad /tmp/native-fit-sketch.pcad >/tmp/puppycad-fit-sketch.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fit-sketch.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fit-sketch.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" r
xdotool mousemove 800 170 click 1
xdotool mousemove 950 300 click 1
import -window root /out/rust-native-fit-before.png
xdotool key --window "$window_id" f
sleep 0.2
import -window root /out/rust-native-fit-after.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-fit-sketch.pcad
cp /tmp/native-fit-sketch.pcad /out/rust-native-fit-sketch.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Cut a hole through a new body from a second Top sketch and export the evaluated mesh.
./target/debug/puppycad /tmp/native-cut-extrusion.pcad >/tmp/puppycad-cut-extrusion.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-cut-extrusion.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-cut-extrusion.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" r
xdotool mousemove 500 300 click 1
xdotool mousemove 750 550 click 1
xdotool key --window "$window_id" x
xdotool key --window "$window_id" 1
xdotool key --window "$window_id" r
xdotool mousemove 575 375 click 1
xdotool mousemove 675 475 click 1
xdotool key --window "$window_id" shift+x
sleep 0.2
import -window root /out/rust-native-cut-extrusion.png
xdotool key --window "$window_id" ctrl+s
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-cut-extrusion.pcad
grep -q '"operation": "cut"' /tmp/native-cut-extrusion.pcad
test -s /tmp/native-cut-extrusion.stl
cp /tmp/native-cut-extrusion.pcad /out/rust-native-cut-extrusion.pcad
cp /tmp/native-cut-extrusion.stl /out/rust-native-cut-extrusion.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Join an overlapping extrusion from a second Top sketch into the first body.
./target/debug/puppycad /tmp/native-join-extrusion.pcad >/tmp/puppycad-join-extrusion.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-join-extrusion.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-join-extrusion.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" r
xdotool mousemove 430 330 click 1
xdotool mousemove 630 530 click 1
xdotool key --window "$window_id" x
xdotool key --window "$window_id" 1
xdotool key --window "$window_id" r
xdotool mousemove 580 330 click 1
xdotool mousemove 780 530 click 1
xdotool key --window "$window_id" shift+j
sleep 0.2
import -window root /out/rust-native-join-extrusion.png
xdotool key --window "$window_id" ctrl+s
xdotool key --window "$window_id" ctrl+e
sleep 0.2
test -s /tmp/native-join-extrusion.pcad
grep -q '"operation": "join"' /tmp/native-join-extrusion.pcad
test -s /tmp/native-join-extrusion.stl
cp /tmp/native-join-extrusion.pcad /out/rust-native-join-extrusion.pcad
cp /tmp/native-join-extrusion.stl /out/rust-native-join-extrusion.stl
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Preview a nearby anchor snap and place a line from that existing sketch point.
./target/debug/puppycad /tmp/native-snap-anchor.pcad >/tmp/puppycad-snap-anchor.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-snap-anchor.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-snap-anchor.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" p
xdotool mousemove 500 300 click 1
xdotool key --window "$window_id" l
xdotool mousemove 505 304
sleep 0.2
import -window root /out/rust-native-snap-target.png
xdotool mousemove 505 304 click 1
xdotool mousemove 650 400 click 1
xdotool mousemove 100 195 click 1
xdotool mousemove 1100 535 click 1
sleep 0.2
import -window root /out/rust-native-snap-linked-move.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-snap-anchor.pcad
grep -q '"type": "sketchLine"' /tmp/native-snap-anchor.pcad
grep -q '"type": "coincident"' /tmp/native-snap-anchor.pcad
cp /tmp/native-snap-anchor.pcad /out/rust-native-snap-anchor.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Preview and save a point snapped to the crossing of two finite sketch lines.
./target/debug/puppycad /tmp/native-intersection-snap.pcad >/tmp/puppycad-intersection-snap.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-intersection-snap.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-intersection-snap.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" l
xdotool mousemove 450 300 click 1
xdotool mousemove 650 500 click 1
xdotool mousemove 450 500 click 1
xdotool mousemove 650 300 click 1
xdotool key --window "$window_id" p
xdotool mousemove 554 404
sleep 0.2
import -window root /out/rust-native-intersection-snap.png
xdotool mousemove 554 404 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-intersection-snap.pcad
cp /tmp/native-intersection-snap.pcad /out/rust-native-intersection-snap.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Two circles expose a finite crossing snap target in the sketch viewport.
./target/debug/puppycad /tmp/native-circle-intersection.pcad >/tmp/puppycad-circle-intersection.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-circle-intersection.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-circle-intersection.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" c
xdotool mousemove 500 400 click 1
xdotool mousemove 600 400 click 1
xdotool mousemove 620 400 click 1
xdotool mousemove 720 400 click 1
xdotool key --window "$window_id" p
xdotool mousemove 564 324
sleep 0.2
import -window root /out/rust-native-circle-intersection.png
xdotool mousemove 564 324 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-circle-intersection.pcad
cp /tmp/native-circle-intersection.pcad /out/rust-native-circle-intersection.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# A line crossing a rectangle edge exposes a boundary snap target.
./target/debug/puppycad /tmp/native-rectangle-intersection.pcad >/tmp/puppycad-rectangle-intersection.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-rectangle-intersection.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-rectangle-intersection.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" r
xdotool mousemove 500 300 click 1
xdotool mousemove 700 500 click 1
xdotool key --window "$window_id" l
xdotool mousemove 450 350 click 1
xdotool mousemove 750 350 click 1
xdotool key --window "$window_id" p
xdotool mousemove 504 354
sleep 0.2
import -window root /out/rust-native-rectangle-intersection.png
xdotool mousemove 504 354 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-rectangle-intersection.pcad
cp /tmp/native-rectangle-intersection.pcad /out/rust-native-rectangle-intersection.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Slot straight-side intersections appear at their analytic boundary.
./target/debug/puppycad /tmp/native-slot-intersection.pcad >/tmp/puppycad-slot-intersection.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-slot-intersection.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-slot-intersection.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" s
xdotool mousemove 500 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 450 click 1
xdotool key --window "$window_id" l
xdotool mousemove 600 300 click 1
xdotool mousemove 600 500 click 1
xdotool key --window "$window_id" p
xdotool mousemove 604 354
sleep 0.2
import -window root /out/rust-native-slot-intersection.png
xdotool mousemove 604 354 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-slot-intersection.pcad
cp /tmp/native-slot-intersection.pcad /out/rust-native-slot-intersection.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# A line crossing an ellipse snaps at the curved finite boundary.
./target/debug/puppycad /tmp/native-ellipse-intersection.pcad >/tmp/puppycad-ellipse-intersection.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-ellipse-intersection.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-ellipse-intersection.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" e
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 450 click 1
xdotool key --window "$window_id" l
xdotool mousemove 450 425 click 1
xdotool mousemove 750 425 click 1
xdotool key --window "$window_id" p
xdotool mousemove 517 429
sleep 0.2
import -window root /out/rust-native-ellipse-intersection.png
xdotool mousemove 517 429 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-ellipse-intersection.pcad
cp /tmp/native-ellipse-intersection.pcad /out/rust-native-ellipse-intersection.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# A circle and ellipse expose their isolated curved crossing as a snap target.
./target/debug/puppycad /tmp/native-circle-ellipse-intersection.pcad >/tmp/puppycad-circle-ellipse-intersection.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-circle-ellipse-intersection.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-circle-ellipse-intersection.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" e
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 450 click 1
xdotool key --window "$window_id" c
xdotool mousemove 650 400 click 1
xdotool mousemove 700 400 click 1
xdotool key --window "$window_id" p
xdotool mousemove 637 357
sleep 0.2
import -window root /out/rust-native-circle-ellipse-intersection.png
xdotool mousemove 637 357 click 1
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-circle-ellipse-intersection.pcad
cp /tmp/native-circle-ellipse-intersection.pcad /out/rust-native-circle-ellipse-intersection.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Split a selected line at the cursor and save both editable line pieces.
./target/debug/puppycad /tmp/native-split-line.pcad >/tmp/puppycad-split-line.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-split-line.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-split-line.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" l
xdotool mousemove 450 400 click 1
xdotool mousemove 750 400 click 1
xdotool mousemove 600 400
xdotool key --window "$window_id" i
sleep 0.2
import -window root /out/rust-native-split-line.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-split-line.pcad
test "$(grep -c '"type": "sketchLine"' /tmp/native-split-line.pcad)" -eq 2
cp /tmp/native-split-line.pcad /out/rust-native-split-line.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the selected horizontal section between two crossing lines.
./target/debug/puppycad /tmp/native-trim-line.pcad >/tmp/puppycad-trim-line.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-trim-line.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-trim-line.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" l
xdotool mousemove 450 400 click 1
xdotool mousemove 750 400 click 1
xdotool mousemove 550 300 click 1
xdotool mousemove 550 500 click 1
xdotool mousemove 650 300 click 1
xdotool mousemove 650 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 500 400 click 1
xdotool mousemove 600 400
xdotool key --window "$window_id" q
sleep 0.2
import -window root /out/rust-native-trim-line.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-trim-line.pcad
test "$(grep -c '"type": "sketchLine"' /tmp/native-trim-line.pcad)" -eq 4
cp /tmp/native-trim-line.pcad /out/rust-native-trim-line.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Extend a selected line to the next crossing boundary.
./target/debug/puppycad /tmp/native-extend-line.pcad >/tmp/puppycad-extend-line.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-extend-line.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-extend-line.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" l
xdotool mousemove 450 400 click 1
xdotool mousemove 600 400 click 1
xdotool mousemove 700 300 click 1
xdotool mousemove 700 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 500 400 click 1
xdotool mousemove 590 400
xdotool key --window "$window_id" u
sleep 0.2
import -window root /out/rust-native-extend-line.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-extend-line.pcad
test "$(grep -c '"type": "sketchLine"' /tmp/native-extend-line.pcad)" -eq 2
cp /tmp/native-extend-line.pcad /out/rust-native-extend-line.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Split a quarter-circle arc into two editable arc nodes.
./target/debug/puppycad /tmp/native-split-arc.pcad >/tmp/puppycad-split-arc.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-split-arc.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-split-arc.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" shift+a
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 300 click 1
xdotool mousemove 671 329
xdotool key --window "$window_id" i
sleep 0.2
import -window root /out/rust-native-split-arc.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-split-arc.pcad
test "$(grep -c '"type": "sketchArc"' /tmp/native-split-arc.pcad)" -eq 2
cp /tmp/native-split-arc.pcad /out/rust-native-split-arc.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the middle of a semicircular arc between two line crossings.
./target/debug/puppycad /tmp/native-trim-arc.pcad >/tmp/puppycad-trim-arc.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-trim-arc.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-trim-arc.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" shift+a
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 500 400 click 1
xdotool key --window "$window_id" l
xdotool mousemove 550 250 click 1
xdotool mousemove 550 400 click 1
xdotool mousemove 650 250 click 1
xdotool mousemove 650 400 click 1
xdotool key --window "$window_id" v
xdotool mousemove 600 300 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" q
sleep 0.2
import -window root /out/rust-native-trim-arc.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-trim-arc.pcad
test "$(grep -c '"type": "sketchArc"' /tmp/native-trim-arc.pcad)" -eq 2
cp /tmp/native-trim-arc.pcad /out/rust-native-trim-arc.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Extend a selected circular arc to the next finite line boundary.
./target/debug/puppycad /tmp/native-extend-arc.pcad >/tmp/puppycad-extend-arc.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-extend-arc.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-extend-arc.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" shift+a
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 300 click 1
xdotool key --window "$window_id" l
xdotool mousemove 550 250 click 1
xdotool mousemove 550 400 click 1
xdotool key --window "$window_id" v
xdotool mousemove 671 329 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" u
sleep 0.2
import -window root /out/rust-native-extend-arc.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-extend-arc.pcad
test "$(grep -c '"type": "sketchArc"' /tmp/native-extend-arc.pcad)" -eq 1
cp /tmp/native-extend-arc.pcad /out/rust-native-extend-arc.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the upper section of a circle between two crossing lines.
./target/debug/puppycad /tmp/native-trim-circle.pcad >/tmp/puppycad-trim-circle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-trim-circle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-trim-circle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" c
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool key --window "$window_id" l
xdotool mousemove 550 250 click 1
xdotool mousemove 550 550 click 1
xdotool mousemove 650 250 click 1
xdotool mousemove 650 550 click 1
xdotool key --window "$window_id" v
xdotool mousemove 600 300 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" q
sleep 0.2
import -window root /out/rust-native-trim-circle.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-trim-circle.pcad
test "$(grep -c '"type": "sketchArc"' /tmp/native-trim-circle.pcad)" -eq 1
cp /tmp/native-trim-circle.pcad /out/rust-native-trim-circle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Split a closed circle at two rim points into editable complementary arcs.
./target/debug/puppycad /tmp/native-split-circle.pcad >/tmp/puppycad-split-circle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-split-circle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-split-circle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" c
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 700 400
xdotool key --window "$window_id" i
xdotool mousemove 600 300
xdotool key --window "$window_id" i
sleep 0.2
import -window root /out/rust-native-split-circle.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-split-circle.pcad
test "$(grep -c '"type": "sketchArc"' /tmp/native-split-circle.pcad)" -eq 2
cp /tmp/native-split-circle.pcad /out/rust-native-split-circle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Split a closed ellipse into two editable elliptic arcs inside isolated Xvfb.
./target/debug/puppycad /tmp/native-split-ellipse.pcad >/tmp/puppycad-split-ellipse.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-split-ellipse.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-split-ellipse.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" e
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 350 click 1
xdotool mousemove 700 400
xdotool key --window "$window_id" i
xdotool mousemove 600 350
xdotool key --window "$window_id" i
sleep 0.2
import -window root /out/rust-native-split-ellipse.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-split-ellipse.pcad
test "$(grep -c '"type": "ellipticArc"' /tmp/native-split-ellipse.pcad)" -eq 2
grep -q '"type": "sameEllipse"' /tmp/native-split-ellipse.pcad
cp /tmp/native-split-ellipse.pcad /out/rust-native-split-ellipse.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the upper ellipse section between crossing lines.
./target/debug/puppycad /tmp/native-trim-ellipse.pcad >/tmp/puppycad-trim-ellipse.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-trim-ellipse.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-trim-ellipse.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" e
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 350 click 1
xdotool key --window "$window_id" l
xdotool mousemove 550 300 click 1
xdotool mousemove 550 500 click 1
xdotool mousemove 650 300 click 1
xdotool mousemove 650 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 600 350 click 1
xdotool mousemove 600 350
xdotool key --window "$window_id" q
sleep 0.2
import -window root /out/rust-native-trim-ellipse.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-trim-ellipse.pcad
test "$(grep -c '"type": "ellipticArc"' /tmp/native-trim-ellipse.pcad)" -eq 1
cp /tmp/native-trim-ellipse.pcad /out/rust-native-trim-ellipse.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Extend an elliptic arc to the next crossing with a sketch line.
./target/debug/puppycad /tmp/native-extend-ellipse.pcad >/tmp/puppycad-extend-ellipse.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-extend-ellipse.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-extend-ellipse.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool key --window "$window_id" e
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 600 350 click 1
xdotool mousemove 700 400
xdotool key --window "$window_id" i
xdotool mousemove 600 350
xdotool key --window "$window_id" i
xdotool key --window "$window_id" l
xdotool mousemove 550 300 click 1
xdotool mousemove 550 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 80 195 click 1
xdotool mousemove 600 350
xdotool key --window "$window_id" u
sleep 0.2
import -window root /out/rust-native-extend-ellipse.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-extend-ellipse.pcad
test "$(grep -c '"type": "ellipticArc"' /tmp/native-extend-ellipse.pcad)" -eq 2
cp /tmp/native-extend-ellipse.pcad /out/rust-native-extend-ellipse.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Open native fit and control spline graph nodes without a host display.
./target/debug/puppycad /app/fixtures/splines.pcad >/tmp/puppycad-splines.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-splines.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-splines.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 80 230 click 1
sleep 0.2
import -window root /out/rust-native-splines.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test "$(grep -c '"type": "sketchSpline"' /app/fixtures/splines.pcad)" -eq 2
cp /app/fixtures/splines.pcad /out/rust-native-splines.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Author one periodic fit spline and one open control spline from the More menu.
./target/debug/puppycad /tmp/native-author-splines.pcad >/tmp/puppycad-author-splines.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-author-splines.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-author-splines.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+g
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 508 click 1
xdotool mousemove 420 330 click 1
xdotool mousemove 520 330 click 1
xdotool mousemove 520 430 click 1
xdotool mousemove 420 430 click 1
xdotool key --window "$window_id" shift+Return
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 544 click 1
xdotool mousemove 690 380 click 1
xdotool mousemove 730 300 click 1
xdotool mousemove 790 460 click 1
xdotool mousemove 840 380 click 1
xdotool key --window "$window_id" Return
sleep 0.2
import -window root /out/rust-native-author-splines.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-author-splines.pcad
test "$(grep -c '"type": "sketchSpline"' /tmp/native-author-splines.pcad)" -eq 2
cp /tmp/native-author-splines.pcad /out/rust-native-author-splines.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Drag one visible control handle while keeping the other spline points fixed.
cp /app/fixtures/splines.pcad /tmp/native-spline-handle.pcad
./target/debug/puppycad /tmp/native-spline-handle.pcad >/tmp/puppycad-spline-handle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-spline-handle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-spline-handle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 80 230 click 1
xdotool mousemove 713 341 mousedown 1
xdotool mousemove --sync 733 321
xdotool mouseup 1
sleep 0.2
import -window root /out/rust-native-spline-handle.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-spline-handle.pcad
grep -q '"x": 110.0' /tmp/native-spline-handle.pcad
grep -q '"y": 110.0' /tmp/native-spline-handle.pcad
cp /tmp/native-spline-handle.pcad /out/rust-native-spline-handle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Edit a selected spline handle's X and Y numerically in Properties.
cp /app/fixtures/splines.pcad /tmp/native-spline-coordinates.pcad
./target/debug/puppycad /tmp/native-spline-coordinates.pcad >/tmp/puppycad-spline-coordinates.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-spline-coordinates.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-spline-coordinates.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 80 230 click 1
xdotool mousemove 713 341 click 1
xdotool mousemove 1080 299 click 1
xdotool type --clearmodifiers 110
xdotool key --window "$window_id" Return
xdotool mousemove 1080 329 click 1
xdotool type --clearmodifiers 110
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-spline-coordinates.png
grep -q '"x": 110.0' /tmp/native-spline-coordinates.pcad
grep -q '"y": 110.0' /tmp/native-spline-coordinates.pcad
cp /tmp/native-spline-coordinates.pcad /out/rust-native-spline-coordinates.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Fix a spline handle, reject a coordinate edit, and save its web fixed relation.
cp /app/fixtures/splines.pcad /tmp/native-fixed-spline.pcad
./target/debug/puppycad /tmp/native-fixed-spline.pcad >/tmp/puppycad-fixed-spline.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fixed-spline.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fixed-spline.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 80 230 click 1
xdotool mousemove 713 341 click 1
xdotool key --window "$window_id" shift+k
xdotool mousemove 1080 299 click 1
xdotool type --clearmodifiers 110
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-fixed-spline.png
grep -q '"type": "fixed"' /tmp/native-fixed-spline.pcad
grep -q '"point": "point2"' /tmp/native-fixed-spline.pcad
grep -q '"x": 90.0' /tmp/native-fixed-spline.pcad
cp /tmp/native-fixed-spline.pcad /out/rust-native-fixed-spline.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Edit a standalone sketch point's X and Y numerically in Properties.
./target/debug/puppycad /tmp/native-point-coordinates.pcad >/tmp/puppycad-point-coordinates.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-point-coordinates.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-point-coordinates.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" p
xdotool mousemove 650 400 click 1
xdotool key --window "$window_id" v
xdotool mousemove 650 400 click 1
xdotool mousemove 1080 214 click 1
xdotool type --clearmodifiers --delay 80 -- -15
xdotool key --window "$window_id" Return
xdotool mousemove 1080 244 click 1
xdotool type --clearmodifiers 25
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-point-coordinates.png
test -s /tmp/native-point-coordinates.pcad
grep -q '"x": -15.0' /tmp/native-point-coordinates.pcad
grep -q '"y": 25.0' /tmp/native-point-coordinates.pcad
cp /tmp/native-point-coordinates.pcad /out/rust-native-point-coordinates.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Split an open control spline at its curve midpoint and save both editable pieces.
cp /app/fixtures/splines.pcad /tmp/native-split-spline.pcad
./target/debug/puppycad /tmp/native-split-spline.pcad >/tmp/puppycad-split-spline.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-split-spline.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-split-spline.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 80 230 click 1
xdotool mousemove 698 431
xdotool key --window "$window_id" i
sleep 0.2
import -window root /out/rust-native-split-spline.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-split-spline.pcad
test "$(grep -c '"type": "sketchSpline"' /tmp/native-split-spline.pcad)" -eq 3
cp /tmp/native-split-spline.pcad /out/rust-native-split-spline.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the middle of a control spline between two crossing lines.
cp /app/fixtures/splines.pcad /tmp/native-trim-spline.pcad
./target/debug/puppycad /tmp/native-trim-spline.pcad >/tmp/puppycad-trim-spline.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-trim-spline.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-trim-spline.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 673 375 click 1
xdotool mousemove 673 495 click 1
xdotool mousemove 723 375 click 1
xdotool mousemove 723 495 click 1
xdotool key --window "$window_id" v
xdotool mousemove 80 230 click 1
xdotool mousemove 698 431
xdotool key --window "$window_id" q
sleep 0.2
import -window root /out/rust-native-trim-spline.png
xdotool key --window "$window_id" ctrl+s
sleep 0.2
test -s /tmp/native-trim-spline.pcad
test "$(grep -c '"type": "sketchSpline"' /tmp/native-trim-spline.pcad)" -eq 3
cp /tmp/native-trim-spline.pcad /out/rust-native-trim-spline.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the middle of a control spline using a circular boundary.
cp /app/fixtures/splines.pcad /tmp/native-circle-trim-spline.pcad
./target/debug/puppycad /tmp/native-circle-trim-spline.pcad >/tmp/puppycad-circle-trim-spline.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-circle-trim-spline.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-circle-trim-spline.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 698 431 click 1
xdotool mousemove 718 431 click 1
xdotool key --window "$window_id" v
xdotool mousemove 80 230 click 1
xdotool mousemove 698 431
xdotool key --window "$window_id" q
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-circle-trim-spline.png
test -s /tmp/native-circle-trim-spline.pcad
test "$(grep -c '"type": "sketchSpline"' /tmp/native-circle-trim-spline.pcad)" -eq 3
cp /tmp/native-circle-trim-spline.pcad /out/rust-native-circle-trim-spline.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim the middle of a control spline using an elliptical boundary.
cp /app/fixtures/splines.pcad /tmp/native-ellipse-trim-spline.pcad
./target/debug/puppycad /tmp/native-ellipse-trim-spline.pcad >/tmp/puppycad-ellipse-trim-spline.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-ellipse-trim-spline.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-ellipse-trim-spline.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" e
xdotool mousemove 698 431 click 1
xdotool mousemove 720 431 click 1
xdotool mousemove 698 446 click 1
xdotool key --window "$window_id" v
xdotool mousemove 80 230 click 1
xdotool mousemove 698 431
xdotool key --window "$window_id" q
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-ellipse-trim-spline.png
test -s /tmp/native-ellipse-trim-spline.pcad
test "$(grep -c '"type": "sketchSpline"' /tmp/native-ellipse-trim-spline.pcad)" -eq 3
cp /tmp/native-ellipse-trim-spline.pcad /out/rust-native-ellipse-trim-spline.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Trim one spline using two crossings with another curved control spline.
cp /app/fixtures/splines.pcad /tmp/native-spline-trim-spline.pcad
./target/debug/puppycad /tmp/native-spline-trim-spline.pcad >/tmp/puppycad-spline-trim-spline.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-spline-trim-spline.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-spline-trim-spline.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool mousemove 1150 63 click 1
xdotool mousemove 1150 544 click 1
xdotool mousemove 653 400 click 1
xdotool mousemove 683 500 click 1
xdotool mousemove 713 500 click 1
xdotool mousemove 743 400 click 1
xdotool key --window "$window_id" Return
xdotool mousemove 80 230 click 1
xdotool mousemove 698 431
xdotool key --window "$window_id" q
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-spline-trim-spline.png
test -s /tmp/native-spline-trim-spline.pcad
test "$(grep -c '"type": "sketchSpline"' /tmp/native-spline-trim-spline.pcad)" -eq 4
cp /tmp/native-spline-trim-spline.pcad /out/rust-native-spline-trim-spline.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Fix a selected line endpoint, save its web-compatible fixed relation, and capture the UI.
./target/debug/puppycad /tmp/native-fixed-line.pcad >/tmp/puppycad-fixed-line.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fixed-line.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fixed-line.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 500 320 click 1
xdotool mousemove 700 320 click 1
xdotool key --window "$window_id" v
xdotool mousemove 80 195 click 1
xdotool mousemove 700 320
xdotool key --window "$window_id" shift+k
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-fixed-line.png
test -s /tmp/native-fixed-line.pcad
grep -q '"type": "fixed"' /tmp/native-fixed-line.pcad
grep -q '"point": "p1"' /tmp/native-fixed-line.pcad
cp /tmp/native-fixed-line.pcad /out/rust-native-fixed-line.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""


# Fix a circle center while retaining an editable diameter.
./target/debug/puppycad /tmp/native-fixed-center.pcad >/tmp/puppycad-fixed-center.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fixed-center.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fixed-center.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 650 400 click 1
xdotool mousemove 700 400 click 1
xdotool key --window "$window_id" shift+k
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-fixed-center.png
test -s /tmp/native-fixed-center.pcad
grep -q '"type": "fixed"' /tmp/native-fixed-center.pcad
grep -q '"point": "center"' /tmp/native-fixed-center.pcad
cp /tmp/native-fixed-center.pcad /out/rust-native-fixed-center.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Fix a circular arc's end anchor and verify its web relation.
./target/debug/puppycad /tmp/native-fixed-arc-end.pcad >/tmp/puppycad-fixed-arc-end.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fixed-arc-end.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fixed-arc-end.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" a
xdotool mousemove 700 400 click 1
xdotool mousemove 650 350 click 1
xdotool mousemove 685 365 click 1
xdotool mousemove 650 350
xdotool key --window "$window_id" shift+k
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-fixed-arc-end.png
test -s /tmp/native-fixed-arc-end.pcad
grep -q '"type": "fixed"' /tmp/native-fixed-arc-end.pcad
grep -q '"point": "p1"' /tmp/native-fixed-arc-end.pcad
cp /tmp/native-fixed-arc-end.pcad /out/rust-native-fixed-arc-end.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Pin an ellipse axis anchor and verify its named fixed relation.
./target/debug/puppycad /tmp/native-fixed-ellipse-axis.pcad >/tmp/puppycad-fixed-ellipse-axis.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fixed-ellipse-axis.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fixed-ellipse-axis.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" e
xdotool mousemove 650 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 650 375 click 1
xdotool mousemove 700 400
xdotool key --window "$window_id" shift+k
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-fixed-ellipse-axis.png
test -s /tmp/native-fixed-ellipse-axis.pcad
grep -q '"type": "fixed"' /tmp/native-fixed-ellipse-axis.pcad
grep -q '"point": "p0"' /tmp/native-fixed-ellipse-axis.pcad
cp /tmp/native-fixed-ellipse-axis.pcad /out/rust-native-fixed-ellipse-axis.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Fix a corner rectangle's first corner and resize the other corner.
./target/debug/puppycad /tmp/native-fixed-corner-rectangle.pcad >/tmp/puppycad-fixed-corner-rectangle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fixed-corner-rectangle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fixed-corner-rectangle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" r
xdotool mousemove 500 320 click 1
xdotool mousemove 700 420 click 1
xdotool mousemove 500 320
xdotool key --window "$window_id" shift+k
xdotool mousemove 1100 214 click 1
xdotool type --clearmodifiers 250
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-fixed-corner-rectangle.png
test -s /tmp/native-fixed-corner-rectangle.pcad
grep -q '"type": "fixed"' /tmp/native-fixed-corner-rectangle.pcad
grep -q '"point": "p0"' /tmp/native-fixed-corner-rectangle.pcad
cp /tmp/native-fixed-corner-rectangle.pcad /out/rust-native-fixed-corner-rectangle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Match a target line's length to a hovered source line with Equal.
./target/debug/puppycad /tmp/native-equal-lines.pcad >/tmp/puppycad-equal-lines.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-equal-lines.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-equal-lines.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 500 300 click 1
xdotool mousemove 700 300 click 1
xdotool mousemove 500 400 click 1
xdotool mousemove 500 450 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" shift+e
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-equal-lines.png
test -s /tmp/native-equal-lines.pcad
grep -q '"type": "equal"' /tmp/native-equal-lines.pcad
cp /tmp/native-equal-lines.pcad /out/rust-native-equal-lines.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Match a target circle's radius to a hovered source circle.
./target/debug/puppycad /tmp/native-equal-radii.pcad >/tmp/puppycad-equal-radii.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-equal-radii.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-equal-radii.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 500 300 click 1
xdotool mousemove 600 300 click 1
xdotool mousemove 700 430 click 1
xdotool mousemove 750 430 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" shift+e
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-equal-radii.png
test -s /tmp/native-equal-radii.pcad
grep -q '"type": "equal"' /tmp/native-equal-radii.pcad
cp /tmp/native-equal-radii.pcad /out/rust-native-equal-radii.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Move an ellipse onto a circle's center with Concentric.
./target/debug/puppycad /tmp/native-concentric.pcad >/tmp/puppycad-concentric.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-concentric.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-concentric.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 500 300 click 1
xdotool mousemove 600 300 click 1
xdotool key --window "$window_id" e
xdotool mousemove 700 430 click 1
xdotool mousemove 750 430 click 1
xdotool mousemove 700 470 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" shift+o
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-concentric.png
test -s /tmp/native-concentric.pcad
grep -q '"type": "concentric"' /tmp/native-concentric.pcad
cp /tmp/native-concentric.pcad /out/rust-native-concentric.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Match a target line to a source as Parallel, Perpendicular, and Collinear.
for relation in parallel perpendicular collinear; do
    case "$relation" in
        parallel) shortcut=shift+p ;;
        perpendicular) shortcut=shift+n ;;
        collinear) shortcut=shift+l ;;
    esac
    ./target/debug/puppycad "/tmp/native-${relation}.pcad" >"/tmp/puppycad-${relation}.log" 2>&1 &
    app_pid=$!
    tries=0
    until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
        if ! kill -0 "$app_pid" 2>/dev/null; then cat "/tmp/puppycad-${relation}.log"; exit 1; fi
        tries=$((tries + 1))
        if [ "$tries" -ge 100 ]; then cat "/tmp/puppycad-${relation}.log"; exit 1; fi
        sleep 0.1
    done
    window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
    xdotool windowmove "$window_id" 0 0
    xdotool windowsize "$window_id" 1280 800
    xdotool windowfocus --sync "$window_id"
    sleep 1
    xdotool key --window "$window_id" l
    xdotool mousemove 500 300 click 1
    xdotool mousemove 700 300 click 1
    xdotool mousemove 500 400 click 1
    xdotool mousemove 550 450 click 1
    xdotool key --window "$window_id" u
    xdotool mousemove 70 225 click 1
    xdotool mousemove 600 300
    xdotool key --window "$window_id" "$shortcut"
    xdotool key --window "$window_id" ctrl+s
    sleep 0.5
    import -window root "/out/rust-native-${relation}.png"
    test -s "/tmp/native-${relation}.pcad"
    grep -q "\"type\": \"${relation}\"" "/tmp/native-${relation}.pcad"
    cp "/tmp/native-${relation}.pcad" "/out/rust-native-${relation}.pcad"
    kill "$app_pid"
    wait "$app_pid" 2>/dev/null || true
    app_pid=""
done

# Match both axes of a target ellipse to a hovered source ellipse.
./target/debug/puppycad /tmp/native-equal-ellipses.pcad >/tmp/puppycad-equal-ellipses.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-equal-ellipses.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-equal-ellipses.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" e
xdotool mousemove 500 300 click 1
xdotool mousemove 600 300 click 1
xdotool mousemove 500 350 click 1
xdotool mousemove 750 430 click 1
xdotool mousemove 800 430 click 1
xdotool mousemove 750 470 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" shift+e
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-equal-ellipses.png
test -s /tmp/native-equal-ellipses.pcad
grep -q '"type": "equal"' /tmp/native-equal-ellipses.pcad
cp /tmp/native-equal-ellipses.pcad /out/rust-native-equal-ellipses.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Edit radius dimensions on an arc and a regular polygon.
./target/debug/puppycad /tmp/native-curve-radius.pcad >/tmp/puppycad-curve-radius.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-curve-radius.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-curve-radius.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+a
xdotool mousemove 500 350 click 1
xdotool mousemove 600 350 click 1
xdotool mousemove 500 450 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 195 click 1
xdotool mousemove 1100 214 click 1
xdotool key --window "$window_id" ctrl+a
xdotool type --clearmodifiers 150
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" g
xdotool mousemove 750 400 click 1
xdotool mousemove 800 400 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 1100 244 click 1
xdotool key --window "$window_id" ctrl+a
xdotool type --clearmodifiers 80
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-curve-radius.png
test -s /tmp/native-curve-radius.pcad
test "$(grep -c '"type": "radius"' /tmp/native-curve-radius.pcad)" -eq 2
cp /tmp/native-curve-radius.pcad /out/rust-native-curve-radius.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Edit a signed arc sweep and a regular polygon's rotation.
./target/debug/puppycad /tmp/native-angular-dimensions.pcad >/tmp/puppycad-angular-dimensions.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-angular-dimensions.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-angular-dimensions.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" shift+a
xdotool mousemove 500 350 click 1
xdotool mousemove 600 350 click 1
xdotool mousemove 500 450 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 195 click 1
xdotool mousemove 1100 244 click 1
xdotool key --window "$window_id" ctrl+a
xdotool type --clearmodifiers --delay 50 -- -90
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" g
xdotool mousemove 750 400 click 1
xdotool mousemove 800 400 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 1100 274 click 1
xdotool key --window "$window_id" ctrl+a
xdotool type --clearmodifiers --delay 50 -- -30
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-angular-dimensions.png
test -s /tmp/native-angular-dimensions.pcad
grep -q '"type": "arcSweep"' /tmp/native-angular-dimensions.pcad
grep -q '"type": "rotation"' /tmp/native-angular-dimensions.pcad
grep -q '"value": -90.0' /tmp/native-angular-dimensions.pcad
grep -q '"value": -30.0' /tmp/native-angular-dimensions.pcad
cp /tmp/native-angular-dimensions.pcad /out/rust-native-angular-dimensions.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Create and edit a signed line angle relation.
./target/debug/puppycad /tmp/native-line-angle.pcad >/tmp/puppycad-line-angle.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-line-angle.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-line-angle.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 500 300 click 1
xdotool mousemove 700 300 click 1
xdotool mousemove 500 400 click 1
xdotool mousemove 550 450 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 600 300
xdotool key --window "$window_id" shift+b
xdotool mousemove 1100 419 click 1
xdotool type --clearmodifiers --delay 50 -- -30
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-line-angle.png
test -s /tmp/native-line-angle.pcad
grep -q '"type": "angle"' /tmp/native-line-angle.pcad
grep -q '"value": -30.0' /tmp/native-line-angle.pcad
cp /tmp/native-line-angle.pcad /out/rust-native-line-angle.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Create and edit a signed radius difference between two circles.
./target/debug/puppycad /tmp/native-radius-difference.pcad >/tmp/puppycad-radius-difference.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-radius-difference.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-radius-difference.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 500 300 click 1
xdotool mousemove 550 300 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 730 400 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 550 300
xdotool key --window "$window_id" shift+d
xdotool mousemove 1100 419 click 1
xdotool type --clearmodifiers --delay 50 -- -20
xdotool key --window "$window_id" Return
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-radius-difference.png
test -s /tmp/native-radius-difference.pcad
grep -q '"type": "radiusDifference"' /tmp/native-radius-difference.pcad
grep -q '"value": -20.0' /tmp/native-radius-difference.pcad
cp /tmp/native-radius-difference.pcad /out/rust-native-radius-difference.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Keep a smaller circle internally tangent to a larger circle.
./target/debug/puppycad /tmp/native-internal-tangent.pcad >/tmp/puppycad-internal-tangent.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-internal-tangent.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-internal-tangent.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 600 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 650 400 click 1
xdotool mousemove 690 400 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 500 400
xdotool key --window "$window_id" shift+y
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-internal-tangent.png
test -s /tmp/native-internal-tangent.pcad
grep -q '"type": "internalTangent"' /tmp/native-internal-tangent.pcad
cp /tmp/native-internal-tangent.pcad /out/rust-native-internal-tangent.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Keep two circles externally tangent.
./target/debug/puppycad /tmp/native-external-tangent.pcad >/tmp/puppycad-external-tangent.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-external-tangent.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-external-tangent.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 500 400 click 1
xdotool mousemove 550 400 click 1
xdotool mousemove 700 400 click 1
xdotool mousemove 730 400 click 1
xdotool key --window "$window_id" v
xdotool mousemove 70 225 click 1
xdotool mousemove 450 400
xdotool key --window "$window_id" shift+q
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-external-tangent.png
test -s /tmp/native-external-tangent.pcad
grep -q '"type": "tangent"' /tmp/native-external-tangent.pcad
cp /tmp/native-external-tangent.pcad /out/rust-native-external-tangent.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Make a line tangent to a circle.
./target/debug/puppycad /tmp/native-line-circle-tangent.pcad >/tmp/puppycad-line-circle-tangent.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-line-circle-tangent.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-line-circle-tangent.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 450 400 click 1
xdotool mousemove 550 400 click 1
xdotool key --window "$window_id" c
xdotool mousemove 500 500 click 1
xdotool mousemove 530 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 70 225 click 1
xdotool mousemove 500 400
xdotool key --window "$window_id" shift+q
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-line-circle-tangent.png
test -s /tmp/native-line-circle-tangent.pcad
grep -q '"type": "tangent"' /tmp/native-line-circle-tangent.pcad
cp /tmp/native-line-circle-tangent.pcad /out/rust-native-line-circle-tangent.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Make a line tangent to an ellipse.
./target/debug/puppycad /tmp/native-line-ellipse-tangent.pcad >/tmp/puppycad-line-ellipse-tangent.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-line-ellipse-tangent.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-line-ellipse-tangent.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 450 400 click 1
xdotool mousemove 550 400 click 1
xdotool key --window "$window_id" e
xdotool mousemove 500 500 click 1
xdotool mousemove 550 500 click 1
xdotool mousemove 500 530 click 1
xdotool key --window "$window_id" u
xdotool mousemove 70 225 click 1
xdotool mousemove 500 400
xdotool key --window "$window_id" shift+q
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-line-ellipse-tangent.png
test -s /tmp/native-line-ellipse-tangent.pcad
grep -q '"type": "tangent"' /tmp/native-line-ellipse-tangent.pcad
cp /tmp/native-line-ellipse-tangent.pcad /out/rust-native-line-ellipse-tangent.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Link a sketch point to the midpoint of a line.
./target/debug/puppycad /tmp/native-midpoint.pcad >/tmp/puppycad-midpoint.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-midpoint.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-midpoint.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 500 400 click 1
xdotool mousemove 700 400 click 1
xdotool key --window "$window_id" p
xdotool mousemove 650 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 70 225 click 1
xdotool mousemove 550 400
xdotool key --window "$window_id" shift+m
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-midpoint.png
test -s /tmp/native-midpoint.pcad
grep -q '"type": "midpoint"' /tmp/native-midpoint.pcad
cp /tmp/native-midpoint.pcad /out/rust-native-midpoint.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Constrain a sketch point to a circle's finite curve.
./target/debug/puppycad /tmp/native-point-on-curve.pcad >/tmp/puppycad-point-on-curve.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-point-on-curve.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-point-on-curve.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 600 400 click 1
xdotool mousemove 680 400 click 1
xdotool key --window "$window_id" p
xdotool mousemove 750 500 click 1
xdotool key --window "$window_id" v
xdotool mousemove 70 225 click 1
xdotool mousemove 520 400
xdotool key --window "$window_id" shift+v
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-point-on-curve.png
test -s /tmp/native-point-on-curve.pcad
grep -q '"type": "pointOnCurve"' /tmp/native-point-on-curve.pcad
cp /tmp/native-point-on-curve.pcad /out/rust-native-point-on-curve.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Make a line endpoint normal to a circle.
./target/debug/puppycad /tmp/native-normal.pcad >/tmp/puppycad-normal.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-normal.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-normal.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" c
xdotool mousemove 600 400 click 1
xdotool mousemove 680 400 click 1
xdotool key --window "$window_id" l
xdotool mousemove 430 360 click 1
xdotool mousemove 500 380 click 1
xdotool key --window "$window_id" v
xdotool mousemove 70 225 click 1
xdotool mousemove 520 400
xdotool key --window "$window_id" shift+f
xdotool key --window "$window_id" ctrl+s
sleep 0.5
import -window root /out/rust-native-normal.png
test -s /tmp/native-normal.pcad
grep -q '"type": "normal"' /tmp/native-normal.pcad
cp /tmp/native-normal.pcad /out/rust-native-normal.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Drag a single line endpoint: the other end stays, the dragged anchor is
# drawn as a filled square while the button is held.
./target/debug/puppycad /tmp/native-anchor-drag.pcad >/tmp/puppycad-anchor-drag.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-anchor-drag.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-anchor-drag.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 430 360 click 1
xdotool mousemove 630 360 click 1
xdotool key --window "$window_id" v
xdotool mousemove 530 360 click 1
xdotool mousemove 630 360 mousedown 1
xdotool mousemove 610 330
xdotool mousemove 600 250
sleep 0.5
import -window root /out/rust-native-anchor-drag.png
xdotool mouseup 1
xdotool key --window "$window_id" ctrl+s
sleep 0.5
test -s /tmp/native-anchor-drag.pcad
cp /tmp/native-anchor-drag.pcad /out/rust-native-anchor-drag.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Mirror a circle across a line (Shift+I, click the line): a linked copy
# appears on the other side, both rows show [SYM], and the project saves a
# web "mirror" relation.
./target/debug/puppycad /tmp/native-mirror.pcad >/tmp/puppycad-mirror.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-mirror.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-mirror.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 530 230 click 1
xdotool mousemove 530 520 click 1
xdotool key --window "$window_id" c
xdotool mousemove 660 330 click 1
xdotool mousemove 710 330 click 1
xdotool key --window "$window_id" v
xdotool mousemove 710 330 click 1
xdotool key --window "$window_id" shift+i
xdotool mousemove 530 400 click 1
sleep 0.5
import -window root /out/rust-native-mirror.png
xdotool key --window "$window_id" ctrl+s
sleep 0.5
test -s /tmp/native-mirror.pcad
grep -q '"type": *"mirror"' /tmp/native-mirror.pcad
cp /tmp/native-mirror.pcad /out/rust-native-mirror.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Fillet one corner with Shift+W and chamfer another with Shift+H
# (distance-angle mode via Tab), then save the web relations.
./target/debug/puppycad /tmp/native-fillet.pcad >/tmp/puppycad-fillet.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-fillet.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-fillet.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
for segment in "330 250 480 250" "480 250 480 450" "560 250 760 250" "760 250 760 450"; do
    set -- $segment
    xdotool key --window "$window_id" l
    xdotool mousemove "$1" "$2" click 1
    xdotool mousemove "$3" "$4" click 1
done
xdotool key --window "$window_id" v
xdotool mousemove 400 250 click 1
xdotool mousemove 480 380
xdotool key --window "$window_id" shift+w
xdotool type --clearmodifiers '40'
xdotool key Return
sleep 0.5
import -window root /out/rust-native-fillet.png
xdotool mousemove 650 250 click 1
xdotool mousemove 760 380
xdotool key --window "$window_id" shift+h
xdotool key --window "$window_id" Tab
xdotool type --clearmodifiers '50'
xdotool key Return
xdotool type --clearmodifiers '30'
xdotool key Return
sleep 0.5
import -window root /out/rust-native-chamfer.png
xdotool key --window "$window_id" ctrl+s
sleep 0.5
grep -q '"type": "endpointTangent"' /tmp/native-fillet.pcad
grep -q '"type": "chamfer"' /tmp/native-fillet.pcad
grep -q '"mode": "distance-angle"' /tmp/native-fillet.pcad
cp /tmp/native-fillet.pcad /out/rust-native-fillet.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Box-select two of three lines with a left-to-right window, then drag one
# of them: both selected lines move together, the third stays put.
./target/debug/puppycad /tmp/native-box-select.pcad >/tmp/puppycad-box-select.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-box-select.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-box-select.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
for y in 300 400; do
    xdotool key --window "$window_id" l
    xdotool mousemove 400 "$y" click 1
    xdotool mousemove 520 "$y" click 1
    xdotool key --window "$window_id" Escape
done
xdotool key --window "$window_id" l
xdotool mousemove 760 300 click 1
xdotool mousemove 880 300 click 1
xdotool key --window "$window_id" Escape
xdotool key --window "$window_id" v
xdotool mousemove 370 260 mousedown 1
xdotool mousemove 450 340
xdotool mousemove 550 440
sleep 0.5
import -window root /out/rust-native-box-select-band.png
xdotool mouseup 1
sleep 0.5
import -window root /out/rust-native-box-select.png
xdotool mousemove 460 300 mousedown 1
xdotool mousemove 470 260
xdotool mousemove 480 200
xdotool mouseup 1
sleep 0.5
import -window root /out/rust-native-box-select-moved.png
xdotool key --window "$window_id" ctrl+s
sleep 0.5
test -s /tmp/native-box-select.pcad
cp /tmp/native-box-select.pcad /out/rust-native-box-select.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Offset an L-shaped line chain (Shift+U, 20 mm): both lines get mitered
# offset copies, the rows show [OFS], and the project saves a web
# "offsetChain" relation.
./target/debug/puppycad /tmp/native-offset.pcad >/tmp/puppycad-offset.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-offset.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-offset.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
for segment in "400 250 400 450" "400 450 700 450"; do
    set -- $segment
    xdotool key --window "$window_id" l
    xdotool mousemove "$1" "$2" click 1
    xdotool mousemove "$3" "$4" click 1
done
xdotool key --window "$window_id" v
xdotool mousemove 550 450 click 1
xdotool key --window "$window_id" shift+u
xdotool type --clearmodifiers '20'
xdotool key Return
sleep 0.5
import -window root /out/rust-native-offset.png
xdotool key --window "$window_id" ctrl+s
sleep 0.5
test -s /tmp/native-offset.pcad
grep -q '"type": "offsetChain"' /tmp/native-offset.pcad
cp /tmp/native-offset.pcad /out/rust-native-offset.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Tangent arc (,): click a line's end, then the arc end; the arc
# leaves the line tangentially. Then "." smooth-joins a second line to
# the first line's start (it swings into a straight continuation), and the
# project saves web "smoothJoin" relations.
./target/debug/puppycad /tmp/native-tangent-arc.pcad >/tmp/puppycad-tarc.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-tarc.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-tarc.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" l
xdotool mousemove 430 420 click 1
xdotool mousemove 630 420 click 1
xdotool key --window "$window_id" comma
xdotool mousemove 631 421 click 1
xdotool mousemove 760 290
sleep 0.3
import -window root /out/rust-native-tangent-arc-preview.png
xdotool click 1
xdotool key --window "$window_id" l
xdotool mousemove 420 440 click 1
xdotool mousemove 350 520 click 1
xdotool key --window "$window_id" v
xdotool mousemove 530 420 click 1
xdotool mousemove 385 480
xdotool key --window "$window_id" period
sleep 0.5
import -window root /out/rust-native-tangent-arc.png
xdotool key --window "$window_id" ctrl+s
sleep 0.5
test "$(grep -c '"type": "smoothJoin"' /tmp/native-tangent-arc.pcad)" -eq 2
cp /tmp/native-tangent-arc.pcad /out/rust-native-tangent-arc.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""

# Dimension objects: a point-to-point distance with the D tool, an absolute
# X dimension driven by a sketch variable.
./target/debug/puppycad /tmp/native-dims.pcad >/tmp/puppycad-dims.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat /tmp/puppycad-dims.log; exit 1; fi
    tries=$((tries + 1))
    if [ "$tries" -ge 100 ]; then cat /tmp/puppycad-dims.log; exit 1; fi
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowmove "$window_id" 0 0
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
sleep 1
xdotool key --window "$window_id" p
xdotool mousemove 523 431 click 1
xdotool key --window "$window_id" l
xdotool mousemove 623 331 click 1
xdotool mousemove 783 331 click 1
# D: point, then line start -> distance; type its value in the label.
xdotool key --window "$window_id" d
xdotool mousemove 523 431 click 1
xdotool mousemove 623 331 click 1
xdotool type --clearmodifiers '150'
import -window root /out/rust-native-dimension-edit.png
xdotool key Return
# Add a sketch variable in the Properties Variables section.
xdotool key --window "$window_id" v
xdotool key --window "$window_id" Escape
xdotool mousemove 1100 281 click 1
xdotool type --clearmodifiers 'w = 40 mm'
xdotool key Return
# D: point, Shift+origin -> absolute X dimension from a variable.
xdotool key --window "$window_id" d
xdotool mousemove 523 431 click 1
xdotool keydown shift
xdotool mousemove 623 431 click 1
xdotool keyup shift
xdotool type --clearmodifiers '#w*-2'
xdotool key Return
xdotool mousemove 900 200
xdotool key --window "$window_id" v
xdotool key --window "$window_id" Escape
sleep 0.5
import -window root /out/rust-native-dimensions.png
xdotool key --window "$window_id" ctrl+s
sleep 0.5
test -s /tmp/native-dims.pcad
grep -q '"type": "distance"' /tmp/native-dims.pcad
grep -q '"axis": "x"' /tmp/native-dims.pcad
grep -q '"expression": "#w\*-2"' /tmp/native-dims.pcad
grep -q '"name": "w"' /tmp/native-dims.pcad
cp /tmp/native-dims.pcad /out/rust-native-dimensions.pcad
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""
