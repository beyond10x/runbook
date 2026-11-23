#!/bin/sh
# Adversary fixture: a second writer that is not called while a finding waits.
echo "author2" >> order.log
if grep -q '^needs_person' findings.txt; then
  exit 79
fi
echo "spec v3" > spec.txt
