#!/bin/sh
[ ! -f pause.txt ] || sleep "$(cat pause.txt)"
[ "$1" != write ] || echo note >> note.txt
w=.sdc/llm
mkdir -p "$w"
i=$(( $(find "$w" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))
d=$(printf '%03d-step.%s' "$i" "$1")
mkdir -p "$w/$d"
echo '{}' > "$w/$d/request.json"
printf '{"action":"step.%s","adapter":"stand-in","exit":0,"cost_usd":0.2,"record":"%s/%s"}\n' "$1" "$PWD/$w" "$d" >> "$w/calls.jsonl"
