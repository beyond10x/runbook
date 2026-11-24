#!/bin/sh
# check-tree-links.sh [<root>]: exit 1, naming each, when the tree under <root> (default: the repository root) holds a
# symbolic link that a copy of the tree cannot follow: a link to nothing (also a chain that ends nowhere or loops) or a
# link to a directory that holds the link. Directories named .git or target are not read: a copy of the tree leaves
# the build output behind, and the specs of ui/ write their hostile run directories under ui/target/
# (ui/test/fixture-dir.mjs). The UI test step runs it last.
set -eu
root=$(cd "${1:-$(dirname "$0")/..}" && pwd -P)
found=$(
  find "$root" \( -name .git -o \( -type d -name target \) \) -prune -o -type l -print | while IFS= read -r link; do
    rel=${link#"$root"/}
    if [ ! -e "$link" ]; then
      echo "a link to nothing: $rel"
    elif [ -d "$link" ]; then
      to=$(cd "$link" && pwd -P)
      at=$(cd "$(dirname "$link")" && pwd -P)
      case "$at/" in "$to"/*) echo "a link to a directory that holds it: $rel" ;; esac
    fi
  done
)
if [ -n "$found" ]; then
  printf '%s\n' "$found"
  echo "$(printf '%s\n' "$found" | wc -l | tr -d ' ') links under $root that a copy of the tree cannot follow"
  exit 1
fi
echo "no link under $root that a copy of the tree cannot follow"
