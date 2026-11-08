#!/usr/bin/env bash
# Before the agent finishes: run the tests of the crates it changed.
# Exit 2 keeps the agent working until they pass.
export PATH="$HOME/.cargo/bin:$PATH"

input=$(cat)
# Already continuing because of this hook: let it stop rather than loop.
if printf '%s' "$input" | grep -qE '"stop_hook_active"[[:space:]]*:[[:space:]]*true'; then
  exit 0
fi

cwd=$(printf '%s' "$input" | tr -d '\r\n' \
  | sed -nE 's/.*"cwd"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p' | sed 's/\\\\/\//g')
cd "${cwd:-$CLAUDE_PROJECT_DIR}" 2>/dev/null || exit 0
root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root" || exit 0
[ -f Cargo.toml ] || exit 0

changed=$(git status --porcelain -- '*.rs' 'Cargo.toml' '**/Cargo.toml' | cut -c4-)
[ -n "$changed" ] || exit 0

# Only crates/<name>/ changes can be narrowed to one crate; anything else tests everything.
packages=()
while IFS= read -r path; do
  case "$path" in
    crates/*/*) dir=${path#crates/}; packages+=("-p" "${dir%%/*}") ;;
    *) packages=(); break ;;
  esac
done <<< "$changed"

if ! output=$(cargo test --quiet "${packages[@]}" 2>&1); then
  printf 'Tests failed; fix them before finishing:\n%s\n' "$output" | tail -40 >&2
  exit 2
fi
exit 0
