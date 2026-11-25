#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
case ${1:-build} in
    build|test) ;;
    *) echo "Usage: ./build.sh [build|test]" >&2; exit 2 ;;
esac
output="$PWD/workdir/flexscript"
mkdir -p "$output"
stage=$(mktemp -d "$output/.build-XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT

if [[ -z ${FLEXSCRIPT_LIB:-} || -z ${FLEX_COMPILER:-} ]]; then
    rev=$(cat flexscript-sdk.rev)
    [[ "$rev" =~ ^[0-9a-f]{40}$ ]] || { echo "Invalid SDK revision" >&2; exit 1; }
    sdk="$output/sdk/$rev"
    if [[ ! -d "$sdk" ]]; then
        checkout=${FLEXSCRIPT_DIR:-"$output/checkout"}
        if ! git -C "$checkout" rev-parse --verify --quiet "$rev^{commit}" > /dev/null 2>&1; then
            printf 'Flexscript SDK %s is unavailable in %s. See the SDK setup in docs/flexscript-native.md.\n' "$rev" "$checkout" >&2
            exit 1
        fi
        mkdir -p "$output/sdk" "$stage/sdk"
        git -C "$checkout" archive "$rev" compiler vm lib libs extensions platform scripts | tar -x -C "$stage/sdk"
        mv -T "$stage/sdk" "$sdk" 2>/dev/null || [[ -d "$sdk" ]]
    fi
    FLEXSCRIPT_LIB=${FLEXSCRIPT_LIB:-"$sdk/libs"}
    if [[ -z ${FLEX_COMPILER:-} ]]; then
        FLEX_COMPILER="$sdk/flexscript"
        if [[ ! -x "$FLEX_COMPILER" ]]; then
            if ! FLEXSCRIPT_LIB="$sdk/libs" flexscript "$sdk/compiler/main.flex" -o "$stage/compiler" 2>/dev/null; then
                python3 - "$sdk" "$stage/bridge-sdk" <<'PY'
import re, shutil, sys
from pathlib import Path

sdk, dst = Path(sys.argv[1]), Path(sys.argv[2])
dst.mkdir(parents=True, exist_ok=True)
KEYWORDS = {"requires","ensures","reads","modifies","releases","trusted","terminates","verify","invariant","decreases"}

def skip_block(src, i):
    n, depth, quoted = len(src), 0, False
    while i < n:
        c = src[i]; i += 1
        if quoted:
            if c == '\\': i += 1
            elif c == '"': quoted = False
        elif c == '"': quoted = True
        elif c == '/' and i < n and src[i] == '/':
            while i < n and src[i] != '\n': i += 1
        elif c == '{': depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0: return i
    raise RuntimeError("unterminated proof block")

def strip_annotations(src):
    out, n, i, dropping = [], len(src), 0, False
    while i < n:
        start = i
        nl = src.find('\n', i)
        line, i = (src[i:], n) if nl == -1 else (src[i:nl], nl + 1)
        kept = True
        if dropping:
            kept = False
            if line.endswith(';'): dropping = False
        else:
            s = line.lstrip(' \t')
            m = re.match(r'[A-Za-z0-9_]+', s)
            word = m.group(0) if m else ''
            rest = s[len(word):].lstrip(' \t') if word else ''
            if (word == 'proof' and rest.startswith('{')) or (word in ('lemma', 'spec') and (rest.startswith('fn ') or rest.startswith('fn\t'))):
                i = skip_block(src, start)
                while i < n and src[i] in ' \t\r': i += 1
                if i + 1 < n and src[i:i+2] == '//':
                    while i < n and src[i] != '\n': i += 1
                if i < n and src[i] == '\n': i += 1
                kept = False
            elif word in KEYWORDS:
                kept = False
                if not line.endswith(';'): dropping = True
        if kept: out.append(line + '\n')
    return ''.join(out)

for d in ("lib", "libs", "extensions", "platform", "scripts"):
    (dst / d).symlink_to(sdk / d)
for d in ("compiler", "vm"):
    shutil.copytree(sdk / d, dst / d)
    for path in (dst / d).rglob("*.flex"):
        path.write_text(strip_annotations(path.read_text()))
PY
                FLEXSCRIPT_LIB="$sdk/libs" flexscript "$stage/bridge-sdk/compiler/main.flex" -o "$stage/bridge"
                FLEXSCRIPT_LIB="$sdk/libs" "$stage/bridge" "$sdk/compiler/main.flex" -o "$stage/compiler"
            fi
            mv "$stage/compiler" "$FLEX_COMPILER"
        fi
    fi
fi
export FLEXSCRIPT_LIB FLEX_COMPILER
"$FLEX_COMPILER" --native-pipeline direct main.flex -o "$stage/puppycad"
mv "$stage/puppycad" "$output/puppycad"
if [[ ${1:-build} == test ]]; then
    cargo build --locked --manifest-path rust/Cargo.toml
    python3 tests/project.py
    python3 tests/window.py
fi
