#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
"$repo_dir/build.sh"

if (( $# == 0 )); then
    set -- untitled.pcad
fi
exec "$repo_dir/workdir/flexscript/puppycad" "$@"
