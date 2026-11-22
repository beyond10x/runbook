#!/bin/sh
[ ! -f pause.txt ] || sleep "$(cat pause.txt)"
echo draft >> draft.txt
[ -z "$LOOM_RETRY_FEEDBACK" ] || cp "$LOOM_RETRY_FEEDBACK" feedback.json
