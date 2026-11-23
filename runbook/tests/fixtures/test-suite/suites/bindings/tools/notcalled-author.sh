#!/bin/sh
# Fixture of the neighbours of the not-called fix. Behaviour by the first line of mode.txt: wait (79 while a finding waits, else a spec),
# always79, writes79 (writes spec.txt, then 79), unlisted (like wait).
echo "author" >> order.log
mode=$(head -n 1 mode.txt 2>/dev/null)
case "$mode" in
  always79) exit 79 ;;
  writes79) echo "spec v2" > spec.txt; exit 79 ;;
esac
if grep -q '^needs_person' findings.txt; then
  exit 79
fi
echo "spec v2" > spec.txt
