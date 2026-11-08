#!/usr/bin/env bash
# Lists ways a change may weaken the tests, one Markdown bullet per finding.
# Prints nothing when it finds none. Usage: weakened-tests.sh BASE_COMMIT HEAD_COMMIT
set -euo pipefail

base=$1
head=$2

# Test names rather than counts, so a test that moves to another file is not reported as deleted.
test_names() {
  git ls-tree -r --name-only "$1" | { grep '\.rs$' || true; } | while read -r file; do
    git show "$1:$file" | tr -d '\r' | awk '
      /#\[([a-z_]+::)?test(\(|\])/ { in_test = 1; next }
      in_test && /fn / { sub(/.*fn /, ""); sub(/[(<].*/, ""); print; in_test = 0 }'
  done | LC_ALL=C sort
}

diff_lines() {
  git diff "$base...$head" -- '*.rs' | tr -d '\r'
}

# Prints "name value" for each tolerance set on a removed (-) or added (+) line.
tolerances() {
  diff_lines | { grep "^$1" || true; } | { grep -v "^$1$1$1" || true; } | sed -nE \
    's/^.*[^A-Za-z0-9_](([A-Za-z0-9_]*_)?(tol|tolerance|eps|epsilon)(_[A-Za-z0-9_]*)?)[[:space:]]*(:[^=]*)?=[[:space:]]*([0-9][0-9_.eE+-]*).*$/\1 \6/Ip' \
    | tr -d '_'
}

old_tests=$(mktemp); new_tests=$(mktemp); old_tol=$(mktemp); new_tol=$(mktemp)
trap 'rm -f "$old_tests" "$new_tests" "$old_tol" "$new_tol"' EXIT

test_names "$base" > "$old_tests"
test_names "$head" > "$new_tests"
LC_ALL=C comm -23 "$old_tests" "$new_tests" | sed 's/^\(.*\)$/- Test deleted or renamed: `\1`/'

diff_lines | { grep -E '^\+.*#\[(cfg_attr\(.*)?ignore' || true; } \
  | sed -E 's/^\+[[:space:]]*(.*)$/- Test marked as ignored: `\1`/'

tolerances - > "$old_tol"
tolerances + > "$new_tol"
awk '
  NR == FNR { old[$1, ++old_count[$1]] = $2; next }
  {
    seen = ++new_count[$1]
    if (($1, seen) in old && $2 + 0 > old[$1, seen] + 0)
      printf "- Tolerance loosened: `%s` from %s to %s\n", $1, old[$1, seen], $2
  }' "$old_tol" "$new_tol"
