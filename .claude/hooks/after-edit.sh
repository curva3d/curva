#!/usr/bin/env bash
# After an edit to a Rust file: format it, then run the fast lint.
# Exit 2 sends the lint errors back to the agent.
export PATH="$HOME/.cargo/bin:$PATH"

input=$(cat)
# JSON escapes each backslash in a Windows path, so turn them into plain forward slashes.
file=$(printf '%s' "$input" | tr -d '\r\n' \
  | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p' | sed 's/\\\\/\//g')

case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0

rustfmt "$file" >/dev/null 2>&1 || true

cd "$(dirname "$file")" || exit 0
if ! output=$(cargo clippy --quiet --all-targets -- -D warnings 2>&1); then
  printf 'Clippy found problems after this edit:\n%s\n' "$output" | tail -40 >&2
  exit 2
fi
exit 0
