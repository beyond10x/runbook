#!/bin/sh
# The first run writes what the questions already hold (no revision moves, no `evaluated` line
# follows); a later run adds a line, so the questions move and the check is opened again.
if grep -q '^amend$' order.log 2>/dev/null; then
  echo amended-again >> questions.txt
else
  echo amended > questions.txt
fi
echo amend >> order.log
