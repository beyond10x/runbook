#!/bin/sh
[ ! -f pause.txt ] || sleep "$(cat pause.txt)"
exit 1
