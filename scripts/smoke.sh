#!/usr/bin/env bash
# Quick end-to-end smoke test for journalbuild.
#
#   1. Runs `cargo test`, which evaluates every entry of every journal.
#   2. Runs every lock and chain file's examples with `journalbuild
#      --check-locks`: the library's, and each journal's own.
#   3. Renders the small fixture in tests/smoke_journals at full scale, one
#      page of it supersampled, and all of it on parchment and as a PDF at
#      reduced scale, and every page of webapp/journals/showcase at reduced scale.
#   4. Renders page 0 of every entry of every other journal, in journals/
#      and webapp/journals/, at reduced scale, Darkly output only.
#      Steps 3 and 4 need a GPU and are skipped without one (journalbuild
#      --check-gpu). Step 1 still checks what journals draw then: its
#      tests/svg_renders.rs compares SVG renders, made on the CPU, with
#      committed images.
#   5. Checks the web editor (webapp/): its engine still builds for the
#      browser and, once its npm packages are installed and its wasm built
#      (webapp/README.md), its unit tests pass and it builds.
#
# Renders go to a temporary directory, so the journals' directories are untouched.
# It is deleted afterwards unless something fails or --keep is passed.

set -euo pipefail
cd "$(dirname "$0")/.."

KEEP=false
for arg in "$@"; do
	case "$arg" in
		--keep) KEEP=true ;;
		*) echo "usage: $0 [--keep]" >&2; exit 2 ;;
	esac
done

OUT=$(mktemp -d "${TMPDIR:-/tmp}/journal-smoke.XXXXXX")
BIN=target/release/journalbuild
FAILURES=0

cleanup() {
	if $KEEP || [[ $FAILURES -gt 0 ]]; then
		echo "Renders and logs kept in $OUT"
	else
		rm -rf "$OUT"
	fi
}
trap cleanup EXIT

echo "== build"
cargo build --release --quiet --bin journalbuild
echo "== cargo test"
if test_output=$(cargo test --release --quiet 2>&1); then
	echo "ok   $(grep -c '^test result: ok' <<<"$test_output") test suites passed"
else
	echo "FAIL cargo test"
	grep -E "FAILED|panicked|failed:" <<<"$test_output" | sed 's/^/     /' || true
	FAILURES=$((FAILURES + 1))
fi

if embedded_output=$(cargo test --release --quiet --features embedded-assets --lib resources 2>&1); then
	echo "ok   the library compiled in is the library on disk"
else
	echo "FAIL the library compiled in"
	grep -E "FAILED|panicked|failed:|error" <<<"$embedded_output" | sed 's/^/     /' || true
	FAILURES=$((FAILURES + 1))
fi

echo "== lock checks"
if lock_output=$("$BIN" --check-locks 2>&1); then
	echo "ok   $(grep -c '^ok ' <<<"$lock_output") of the library's lock type(s) and chain(s) passed their examples"
else
	echo "FAIL lock checks"
	grep -vE '^ok ' <<<"$lock_output" | sed 's/^/     /' || true
	FAILURES=$((FAILURES + 1))
fi
# Journals' own lock and chain files.
for journal in journals/*/ webapp/journals/*/; do
	name=$(basename "$journal")
	[[ -d "$journal/locks" || -d "$journal/chains" ]] || continue
	if lock_output=$("$BIN" --check-locks "$name" 2>&1); then
		echo "ok   $name's lock and chain files passed their examples"
	else
		echo "FAIL $name's lock checks"
		grep -vE '^ok ' <<<"$lock_output" | sed 's/^/     /' || true
		FAILURES=$((FAILURES + 1))
	fi
done

# render NAME EXPECTED_FILES... -- JOURNALBUILD_ARGS...
render() {
	local name="$1"; shift
	local expected=()
	while [[ "$1" != "--" ]]; do expected+=("$1"); shift; done
	shift
	local dir="$OUT/$name"
	local start=$SECONDS
	local status=0
	"$BIN" "$@" --output-dir "$dir" >"$OUT/$name.log" 2>&1 || status=$?
	if [[ $status -ne 0 ]]; then
		echo "FAIL $name (exit $status; log: $OUT/$name.log)"
		tail -n 5 "$OUT/$name.log" | sed 's/^/     /'
		FAILURES=$((FAILURES + 1))
		return
	fi
	# ${arr[@]+...} guards the empty case under `set -u` on macOS's bash 3.2.
	for file in ${expected[@]+"${expected[@]}"}; do
		if [[ ! -s "$dir/$file" ]]; then
			echo "FAIL $name: missing or empty $file"
			FAILURES=$((FAILURES + 1))
			return
		fi
	done
	printf "ok   %-28s %3ss\n" "$name" "$((SECONDS - start))"
	# Surface warnings (e.g. characters drawn with the fallback glyph).
	grep "^warning:" "$OUT/$name.log" | sed 's/^/     /' || true
}

echo "== renders"
if ! gpu_output=$("$BIN" --check-gpu 2>&1); then
	echo "skip Darkly renders: no GPU ($(tail -n 1 <<<"$gpu_output" | cut -c1-120))"
	echo "     SVG renders were still compared with tests/svg_renders/ by cargo test"
else
# Each entry's renders go in a directory of its own: smoke/ for the
# fixture's one entry.
render fixture smoke/darkly_page_0_0.png smoke/darkly_page_1_0.png smoke/darkly_page_1_1.png smoke/darkly_lock_maze.png smoke/darkly_art_0.png \
	smoke/darkly_art_0_0_0.png smoke/darkly_art_1_1_0.png smoke/darkly_book_0_0.png smoke/darkly_book_1_0.png \
	smoke/page_0_0.png smoke/page_1_0.png smoke/page_1_1.png smoke/lock_maze.png smoke/art_0.png smoke/art_0_1_0.png \
	smoke/page_0_0.svg smoke/page_1_0.svg smoke/page_1_1.svg smoke/lock_maze.svg smoke/art_0.svg \
	-- smoke --journals-dir tests/smoke_journals --strict-glyphs --svg
render fixture-supersampled smoke/darkly_page_0_0.png \
	-- smoke --journals-dir tests/smoke_journals --page 0 --supersample 2
# Pages on parchment: one image per page, split and typeset pages included.
render fixture-paper smoke/darkly_page_0.png smoke/darkly_page_1.png smoke/darkly_art_0_0.png smoke/darkly_book_0.png \
	-- smoke --journals-dir tests/smoke_journals --parchment --scale 0.5
render fixture-pdf smoke/smoke.pdf \
	-- smoke --journals-dir tests/smoke_journals --pdf --scale 0.25
# Every page of the showcase, every character with a glyph.
render showcase showcase/darkly_01_title_0.png showcase/darkly_02_contents_0.png showcase/darkly_03_pens_brushes.png showcase/darkly_05_pens_0.png showcase/darkly_06_pens_0.png showcase/darkly_07_pens_0.png showcase/darkly_08_pens_0.png \
	showcase/darkly_09_glyphsets_0.png showcase/darkly_10_glyphsets_0.png showcase/darkly_12_glyphsets_0.png showcase/darkly_15_glyphs_0.png showcase/darkly_18_art_0.png showcase/darkly_22_art_0.png showcase/darkly_23_art_0.png showcase/darkly_24_art_0.png showcase/darkly_25_art_0.png showcase/darkly_26_art_0.png \
	showcase/darkly_27_generators_0.png showcase/darkly_28_generators_0.png showcase/darkly_29_generators_0.png showcase/darkly_30_text_0.png showcase/darkly_32_text_0.png showcase/darkly_33_text_0.png showcase/darkly_34_text_0.png \
	showcase/darkly_35_text_0.png showcase/darkly_36_text_0.png showcase/darkly_37_text_0.png showcase/darkly_38_locks_ciphers.png showcase/darkly_39_locks_0.png showcase/darkly_40_locks_0.png \
	showcase/darkly_41_locks_0.png showcase/darkly_42_locks_0.png showcase/darkly_43_spread_0.png showcase/darkly_44_spread_0.png showcase/darkly_45_layouts_0.png showcase/darkly_47_layouts_0.png showcase/darkly_51_files_0.png showcase/darkly_52_files_0.png \
	-- showcase --strict-glyphs --scale 0.25

for journal in journals/*/ webapp/journals/*/; do
	name=$(basename "$journal")
	# Rendered whole above.
	[[ "$name" == showcase ]] && continue
	if [[ -f "$journal/journal.json" ]]; then
		# Page 0 of every entry. Named pages name their files, so just
		# require some Darkly output from each entry.
		render "$name" -- "$name" --page 0 --scale 0.25
		for entry in "$journal"entries/*.rhai; do
			entry=$(basename "$entry" .rhai)
			if [[ -z "$(ls "$OUT/$name/$entry"/darkly_*.png 2>/dev/null)" ]]; then
				echo "FAIL $name/$entry: no Darkly output"
				FAILURES=$((FAILURES + 1))
			fi
		done
	fi
done
fi

echo "== web editor"
if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
	echo "skip the wasm32-unknown-unknown target isn't installed (rustup target add wasm32-unknown-unknown)"
elif wasm_output=$(cargo check --release --quiet -p scribe-wasm --target wasm32-unknown-unknown 2>&1); then
	echo "ok   the renderer builds for the browser"
else
	echo "FAIL the renderer doesn't build for the browser"
	grep -E "^error" -A5 <<<"$wasm_output" | sed 's/^/     /' || true
	FAILURES=$((FAILURES + 1))
fi
FRONTEND=webapp/frontend
if [[ ! -d $FRONTEND/node_modules || ! -f $FRONTEND/wasm-pkg/scribe_wasm.js ]]; then
	echo "skip the editor's frontend: run npm install and npm run wasm in $FRONTEND first"
elif web_output=$(cd $FRONTEND && { npm run --silent examples && npx ng test --watch=false && npx ng build; } 2>&1); then
	echo "ok   the editor's unit tests pass and it builds"
else
	echo "FAIL the editor's frontend"
	grep -iE "error|fail" <<<"$web_output" | head -n 10 | sed 's/^/     /' || true
	FAILURES=$((FAILURES + 1))
fi

if [[ $FAILURES -gt 0 ]]; then
	echo "== $FAILURES failure(s)"
	exit 1
fi
echo "== all smoke tests passed"
