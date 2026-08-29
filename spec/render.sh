#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
out_dir="${1:-"$repo_root/workdir/spec"}"

if [[ "$out_dir" != /* ]]; then
	out_dir="$PWD/$out_dir"
fi

mkdir -p "$out_dir"
cd "$script_dir"

if command -v kpsewhich >/dev/null 2>&1; then
	missing_styles=()
	for style in amsmath.sty amssymb.sty geometry.sty; do
		if ! kpsewhich "$style" >/dev/null 2>&1; then
			missing_styles+=("$style")
		fi
	done

	if ((${#missing_styles[@]} > 0)); then
		echo "error: missing LaTeX package files: ${missing_styles[*]}" >&2
		echo "On Debian/Ubuntu, install the spec dependencies with:" >&2
		echo "  sudo apt install latexmk texlive-latex-recommended" >&2
		exit 1
	fi
fi

if command -v latexmk >/dev/null 2>&1; then
	latexmk -pdf -interaction=nonstopmode -halt-on-error -outdir="$out_dir" puppycad.tex
elif command -v pdflatex >/dev/null 2>&1; then
	pdflatex -interaction=nonstopmode -halt-on-error -output-directory="$out_dir" puppycad.tex
	pdflatex -interaction=nonstopmode -halt-on-error -output-directory="$out_dir" puppycad.tex
else
	echo "error: install latexmk or pdflatex to render spec/puppycad.tex" >&2
	exit 127
fi

echo "Rendered $out_dir/puppycad.pdf"
