#!/bin/sh
echo "wait" >> order.log
grep -q '^needs_person' findings.txt
