#!/usr/bin/env bash
# The commits of a branch, before it goes into main: every subject follows
# Conventional Commits, "type(scope): summary" (the scope is optional).
# Merge commits are skipped.
# Usage: scripts/check-commits.sh [BASE]   (default origin/main)
# Prints the subjects that don't. Exit 0 = ok, 1 = some don't.
set -euo pipefail
base="${1:-origin/main}"
types='build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test'
pattern="^($types)(\([a-z0-9][a-z0-9-]*\))?!?: [^ ]"

bad=$(git log --no-merges --format='%h %s' "$base..HEAD" | while IFS=' ' read -r hash subject; do
    if ! [[ $subject =~ $pattern ]]; then
        echo "  $hash $subject"
    fi
done)
if [ -n "$bad" ]; then
    echo "Subjects not in the form \"type(scope): summary\", type one of ${types//|/, }:"
    echo "$bad"
    echo "Fix them with git rebase -i $base (reword), then git push --force-with-lease."
    exit 1
fi
echo "$(git rev-list --no-merges --count "$base..HEAD") commit(s) since $base, all Conventional Commits."
