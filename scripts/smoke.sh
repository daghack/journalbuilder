#!/usr/bin/env bash
# Quick end-to-end smoke test for journalbuild.
#
#   1. Runs `cargo test`, which evaluates every rhai journal script.
#   2. Renders the small fixture in tests/smoke_journals at full scale, and
#      one page of it supersampled.
#   3. Renders every journal in journals/ at reduced scale: rhai journals
#      page 0 only, legacy journals whole, Darkly output only.
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
}

echo "== renders"
render fixture darkly_page_0_0.png darkly_page_1_0.png darkly_page_1_1.png darkly_page_2_0.png \
	page_0.png page_1.png page_2.png \
	-- smoke --journals-dir tests/smoke_journals
render fixture-supersampled darkly_page_0_0.png \
	-- smoke --journals-dir tests/smoke_journals --page 0 --no-plain --supersample 2

for journal in journals/*/; do
	name=$(basename "$journal")
	if [[ -f "$journal/journal_entry.rhai" ]]; then
		render "$name" darkly_page_0_0.png -- "$name" --page 0 --no-plain --scale 0.25
	elif [[ -f "$journal/journal_entry.json" ]]; then
		# Legacy page names vary; just require some Darkly output.
		render "$name" -- "$name" --legacy --no-plain --scale 0.1
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
