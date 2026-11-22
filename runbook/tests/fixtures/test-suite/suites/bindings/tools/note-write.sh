#!/bin/sh
env > envdump.txt
[ ! -f pause.txt ] || sleep "$(cat pause.txt)"
if [ -f greeting.txt ]; then cat greeting.txt > note.txt; else echo hello > note.txt; fi
