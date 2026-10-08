#!/usr/bin/env bash
# Quick end-to-end smoke test for journalbuild.
#
#   1. Runs `cargo test`, which evaluates every rhai journal script.
#   2. Runs every lock file's examples with `journalbuild --check-locks`.
#   3. Renders the small fixture in tests/smoke_journals at full scale, one
#      page of it supersampled, and all of it on parchment and as a PDF at
#      reduced scale, and every page of journals/showcase at reduced scale.
#   4. Renders page 0 of every journal in journals/ at reduced scale, Darkly
#      output only.
#
# Renders go to a temporary directory, so journals/*/output is untouched.
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

echo "== lock checks"
if lock_output=$("$BIN" --check-locks 2>&1); then
	echo "ok   $(grep -c '^ok ' <<<"$lock_output") lock type(s) passed their examples"
else
	echo "FAIL lock checks"
	grep -vE '^ok ' <<<"$lock_output" | sed 's/^/     /' || true
	FAILURES=$((FAILURES + 1))
fi

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
render fixture darkly_page_0_0.png darkly_page_1_0.png darkly_page_1_1.png darkly_lock_maze.png darkly_art_0.png \
	darkly_art_0_0_0.png darkly_art_1_1_0.png darkly_book_0_0.png darkly_book_1_0.png \
	page_0_0.png page_1_0.png page_1_1.png lock_maze.png art_0.png art_0_1_0.png \
	page_0_0.svg page_1_0.svg page_1_1.svg lock_maze.svg art_0.svg \
	-- smoke --journals-dir tests/smoke_journals --strict-glyphs --svg
render fixture-supersampled darkly_page_0_0.png \
	-- smoke --journals-dir tests/smoke_journals --page 0 --supersample 2
# Pages on parchment: one image per page, split and typeset pages included.
render fixture-paper darkly_page_0.png darkly_page_1.png darkly_art_0_0.png darkly_book_0.png \
	-- smoke --journals-dir tests/smoke_journals --parchment --scale 0.5
render fixture-pdf smoke.pdf \
	-- smoke --journals-dir tests/smoke_journals --pdf --scale 0.25
# Every page of the showcase, every character with a glyph.
render showcase darkly_01_title_0.png darkly_02_contents_0.png darkly_03_pens_brushes.png darkly_05_fonts_0.png darkly_08_glyphs_0.png \
	darkly_10_art_0.png darkly_13_text_0.png darkly_15_text_0.png darkly_16_locks_ciphers.png darkly_17_spread_0.png darkly_18_spread_0.png \
	darkly_19_layouts_0.png \
	-- showcase --strict-glyphs --scale 0.25

for journal in journals/*/; do
	name=$(basename "$journal")
	# Rendered whole above.
	[[ "$name" == showcase ]] && continue
	if [[ -f "$journal/journal_entry.rhai" ]]; then
		# Named pages name their files, so just require some Darkly output.
		render "$name" -- "$name" --page 0 --scale 0.25
		if [[ -z "$(ls "$OUT/$name"/darkly_*.png 2>/dev/null)" ]]; then
			echo "FAIL $name: no Darkly output"
			FAILURES=$((FAILURES + 1))
		fi
	fi
done

if [[ $FAILURES -gt 0 ]]; then
	echo "== $FAILURES failure(s)"
	exit 1
fi
echo "== all smoke tests passed"
