#!/bin/sh
# Run inside the UI-test image with /out mounted to the repository workdir.
set -eu
export DISPLAY=:99
Xvfb :99 -screen 0 1280x800x24 -noreset >/tmp/projection-xvfb.log 2>&1 &
xvfb_pid=$!
app_pid=""
cleanup() {
    [ -z "$app_pid" ] || kill "$app_pid" 2>/dev/null || true
    kill "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT
tries=0
until xdpyinfo >/dev/null 2>&1; do
    tries=$((tries + 1))
    [ "$tries" -lt 50 ] || exit 1
    sleep 0.1
done
./target/debug/puppycad fixtures/segmented-primitives.pcad >/tmp/projection-app.log 2>&1 &
app_pid=$!
tries=0
until xdotool search --name '^PuppyCAD$' >/dev/null 2>&1; do
    tries=$((tries + 1))
    [ "$tries" -lt 100 ] || { cat /tmp/projection-app.log; exit 1; }
    sleep 0.1
done
window_id=$(xdotool search --name '^PuppyCAD$' | head -n 1)
xdotool windowsize "$window_id" 1280 800
xdotool windowfocus --sync "$window_id"
xdotool mousemove 900 750
sleep 1
import -window "$window_id" /out/rust-projection-orthographic.png
xdotool mousemove 1080 22 click 1
xdotool mousemove 900 750
sleep 0.5
import -window "$window_id" /out/rust-projection-perspective.png
# Compare only the model viewport; the header itself must not satisfy this check.
convert /out/rust-projection-orthographic.png -crop 770x674+238+95 +repage /tmp/ortho.png
convert /out/rust-projection-perspective.png -crop 770x674+238+95 +repage /tmp/perspective.png
if compare -metric AE /tmp/ortho.png /tmp/perspective.png null: 2>/tmp/projection-difference; then
    echo 'Projection switch did not change the rendered model' >&2
    exit 1
fi
xdotool mousemove 1210 22 click 1
xdotool mousemove 900 750
sleep 0.5
import -window "$window_id" /out/rust-projection-restored.png
convert /out/rust-projection-restored.png -crop 770x674+238+95 +repage /tmp/restored.png
compare -metric AE /tmp/ortho.png /tmp/restored.png null:
echo 'Projection switched and orthographic view restored exactly.'
