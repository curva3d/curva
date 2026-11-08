#!/usr/bin/env bash
# Refuses commands that can destroy history or work. Exit 2 blocks the command.

input=$(cat)
command=$(printf '%s' "$input" | tr -d '\r\n' \
  | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p')

refuse() {
  echo "Blocked by the project's safety rules: $1" >&2
  exit 2
}

# Keeps a match inside one command, so "git push; rm -f x" is not read as a forced push.
in_git() { printf '%s' "$command" | grep -qE "git[[:space:]]+([^;&|]*[[:space:]])?$1"; }

in_git 'push[[:space:]]+([^;&|]*[[:space:]])?(--force|--force-with-lease|-[a-zA-Z]*f)([[:space:]=]|$)' \
  && refuse 'force-pushing rewrites shared history.'
in_git 'push[[:space:]]+([^;&|]*[[:space:]])?\+[^[:space:]]' \
  && refuse 'a "+" in a push forces it, which rewrites shared history.'
in_git 'push[[:space:]]+([^;&|[:space:]]+[[:space:]]+)*(HEAD:)?(refs/heads/)?main([[:space:]]|$)' \
  && refuse 'changes reach main only through a pull request.'
in_git 'reset[[:space:]]+([^;&|]*[[:space:]])?--hard' \
  && refuse 'a hard reset throws away uncommitted work.'
in_git 'clean[[:space:]]+([^;&|]*[[:space:]])?-[a-zA-Z]*f' \
  && refuse 'git clean permanently deletes untracked files.'
in_git 'branch[[:space:]]+([^;&|]*[[:space:]])?-D([[:space:]]|$)' \
  && refuse 'forcing a branch deletion can lose commits.'
in_git '[^;&|]*--no-verify' \
  && refuse 'skipping the checks is not allowed.'

printf '%s' "$command" | grep -qE 'gh[[:space:]]+repo[[:space:]]+delete' \
  && refuse 'deleting the repository is not allowed.'
printf '%s' "$command" | grep -qE 'rm[[:space:]]+-[a-zA-Z]*[rR][a-zA-Z]*f?[[:space:]]+(/|~|\$HOME|[A-Za-z]:[/\\]*)([[:space:]]|$)' \
  && refuse 'that would delete an entire drive or home folder.'

exit 0
