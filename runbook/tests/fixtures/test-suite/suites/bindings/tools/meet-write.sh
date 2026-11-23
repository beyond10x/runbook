#!/bin/sh
# Cycle n works in <scratch>/work/<n>. The two cycles of meet.suite.yaml each wait for the other's
# marker: they end only when they run beside each other.
n=$(basename "$PWD")
other=$((3 - n))
touch here
tries=0
while [ ! -f "../$other/here" ]; do
  tries=$((tries + 1))
  [ "$tries" -le 300 ] || exit 1
  sleep 0.1
done
echo met > note.txt
