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
            FLEXSCRIPT_LIB="$sdk/libs" flexscript "$sdk/compiler/main.flex" -o "$stage/compiler"
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
