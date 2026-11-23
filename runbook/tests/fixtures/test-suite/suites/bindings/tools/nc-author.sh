#!/bin/sh
# Ends with the not-called code, and writes nothing, while a finding waits for a person.
echo "author" >> order.log
if grep -q '^needs_person' findings.txt; then
  exit 79
fi
echo "spec v2" > spec.txt
