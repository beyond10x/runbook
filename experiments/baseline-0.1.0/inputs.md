# baseline-0.1.0: what each model node is given, what it fetches, and the smallest fact set

The operator, 2026-11-21: "reduce context to minimal fact sets / inputs - dont collect big
conversations". Budget and target: `costs.md`. Source: the six closed runs
`trial-0.1.0-{strong,mixed}-{1,2,3}` (not in git). Per model session, `llm-calls.jsonl` line i is
joined with `llm/<i>-<action>/`: its `cwd/in/` size, and from `events.jsonl` the usage of the first
request, the tool calls by tool, and the bytes each `Read`, `Glob` and `Grep` returned (inside
`cwd/in/` or elsewhere, mostly `impl/`). USD at the list prices of `costs.md`.

- **First request**: the cache write and read of the session's first request (the harness prefix,
  about 15,500 tokens read, plus the prompt).
- **Growth**: all input cost after the first request: each turn re-reads the conversation and
  writes what was added (tool results, the model's own turns).

## Per node (per run: mean of three; turns and KB: median per session)

Strong setting (Sonnet on every work node):

| node | sessions | turns | tool calls per run | in/ KB | read of in/ KB | read elsewhere KB | USD | first request | growth | output |
|---|---|---|---|---|---|---|---|---|---|---|
| `change.implement` | 5 | 25 | Read 55, Edit 37, Write 13, Glob 5, Bash 4, Grep 3 | 65 | 13 | 48 | 2.097 | 0.104 | 1.072 | 0.920 |
| `change.review` | 4.3 | one request | none (all inlined) | 119 | | | 0.799 | 0.691 (all input) | 0 | 0.109 |
| `spec.author` | 2.7 | 21 | Read 37, Write 8, Glob 3, Bash 2, Edit 1, Grep 1 | 42 | 35 | 10 | 0.442 | 0.050 | 0.253 | 0.140 |
| `questions.write` | 1.3 | 4 | Read 3, Write 1 | 1 | 1 | 0 | 0.068 | 0.025 | 0.019 | 0.024 |
| `vision.write` | 1.3 | 7 | Read 5, Write 3 | 4 | 4 | 0 | 0.064 | 0.023 | 0.023 | 0.018 |
| `requirements.normalise` | 1 | 7 | Read 3, Write 2, Glob 1 | 5 | 5 | 0 | 0.055 | 0.018 | 0.021 | 0.016 |
| `roadmap.write` | 1 | 8 | Read 5, Write 2 | 6 | 6 | 0 | 0.055 | 0.018 | 0.021 | 0.015 |
| `plan.decompose` | 1 | 7 | Read 4, Glob 1, Write 1 | 14 | 5 | 0 | 0.045 | 0.018 | 0.018 | 0.009 |
| `context.default` | 1 | 5 | Read 3, Write 1 | 2 | 2 | 0 | 0.041 | 0.018 | 0.013 | 0.010 |
| `questions.answer` | 1 | 5 | Read 3, Edit 1 | 4 | 1 | 3 | 0.036 | 0.017 | 0.014 | 0.005 |
| `idea.dissect` | 1 | 3 | Read 1, Write 1 | 0 | 0 | 0 | 0.031 | 0.017 | 0.007 | 0.006 |

Mixed setting (Sonnet on writer, implement, review; Haiku on the rest):

| node | sessions | turns | tool calls per run | in/ KB | read of in/ KB | read elsewhere KB | USD | first request | growth | output |
|---|---|---|---|---|---|---|---|---|---|---|
| `change.implement` | 4 | 23 | Read 45, Edit 22, Write 14, Glob 4, Bash 3, Grep 1 | 56 | 18 | 17 | 1.712 | 0.083 | 0.807 | 0.823 |
| `change.review` | 4 | one request | none | 108 | | | 0.633 | 0.547 (all input) | 0 | 0.086 |
| `spec.author` | 1.7 | 20 | Read 21, Write 7, Edit 3, Glob 2, Bash 1, Grep 1 | 38 | 37 | 8 | 0.338 | 0.027 | 0.183 | 0.129 |
| `plan.decompose` | 1 | 7 | Read 4, Glob 1, Write 1 | 16 | 10 | 0 | 0.052 | 0.018 | 0.024 | 0.010 |
| the seven Haiku intake and direction nodes | 9 | 3 to 8 | Read 1 to 11, Write 1 to 7 | 0 to 7 | 0 to 11 | 0 to 3 | 0.042 | 0.008 | 0.015 | 0.017 |

What the tables say:

- **Conversation growth is the largest input cost**: 1.46 USD per strong run (27 % of 5.50) and
  1.03 per mixed run (40 % of 2.55), against 0.31 and 0.14 for all first requests. Implement is
  most of it (1.07 and 0.81): about 11 `Read`s per session, each result re-read on every later
  turn.
- **Inputs the program already has are fetched by the model**: `spec.author` reads 35 to 37 KB of
  its own `in/` over 20 turns (`in/example-spec/`, 28.8 KB, is most of it); implement reads 13 to
  18 KB of `in/` and 17 to 48 KB of `impl/`.
- **The review is one request, but with everything**: `in/impl/` is the whole tree (24 KB at the
  first story of `trial-0.1.0-strong-1`, 84 KB at its fourth) plus `in/change.diff` (24 to 26 KB,
  for a new file the whole file again), `tests.txt`, `commands.json`: 33k to 53k tokens written
  per call. The fourth review's prompt (53k written, 68k read) is over 100K tokens, where Haiku
  costs five times as much (`models.md`).
- **The intake nodes pay for the harness, not for their facts**: their inputs are 0 to 7 KB; on
  Sonnet the first request alone (3,600 written, 16,000 read) is a third of each session.

## Smallest fact set per node, and whether it can be one request

One request means: the program assembles the facts into the prompt, the reply is a JSON object of a
response schema, no tools are offered, the program writes the files and runs the node's checks; a
refused check sends one follow-up request with the refusal lines (a cap of two follow-ups, then
the node's normal rerun). Facts are fields of records, not whole files.

| node | smallest fact set (assembled by the program) | one request? |
|---|---|---|
| `idea.dissect` | the idea text; build constraints | yes: `{items}` |
| `questions.write` | idea; dissection items | yes: `{questions}` |
| `questions.answer` | the open questions with options and defaults; build constraints | yes: `{answers}` |
| `requirements.normalise` | idea; dissection items; settled answers (id, text, chosen option) | yes: `{requirements}` |
| `context.default` | requirements (id, text); settled answers; build constraints | yes: `{context}` |
| `vision.write`, `roadmap.write` | requirements; context; for the roadmap the vision's items | yes: `{markdown}` per artifact |
| `plan.decompose` | roadmap items; requirements (id, text); the spec's command names | yes: `{stories}`, the program creates them through `aep` |
| `spec.author` | requirements; context; build constraints; findings addressed to the spec; the format as one minimal example (`system.yaml` and one domain file, not the 28.8 KB example tree); the last `spec.validate` refusal lines on a rerun | yes: `{files: [{path, content}]}`, then `spec.validate` |
| `change.review` | the story's acceptance lines, requirements and commands (`scope.json`, the matching entries of `commands.json`); the changed files of `impl/` with line numbers (the `at` of a finding must cite them); the changed line ranges, not the diff text; the test summary and the names of the story's tests | already one request; only the inputs change |
| `change.implement` | the story; its requirements and acceptance lines; its spec commands (compiled, only those); build constraints; findings and the last rejection addressed to it; the current text of the files its scope names, or of the files the previous story changed | one request per story, `{edits: [{path, old, new}], files: [{path, content}]}`, then `cargo build` and `tests.run`, compiler and test lines in the follow-up |

Implement, asked specifically:

- **Inputs inlined**: removes the 13 to 18 KB of `in/` reads and their re-reads; the 17 to 48 KB
  of `impl/` reads become the files its scope names.
- **Turn cap**: the median session takes 23 to 25 turns. A cap without inlining only cuts the
  tail: the reads that come first stay. With inlining, the cap is the follow-up count (two).
- **One request per file or per requirement is not cheaper**: the shared facts (story, spec
  commands, constraints, about 4k tokens) would be written once per request, and a session writes
  about 3 files and 7 edits; per story is the unit. Split only when a reply exceeds the output cap.
- **Output stays**: 0.82 to 0.92 USD per run. Edits keep it there, whole-file replies would raise
  it (inferred).

## Saving per run, ranked

From the tables above; the assembled input sizes are estimates (inferred), the old costs measured.

| rank | change | strong USD/run | mixed USD/run | how estimated |
|---|---|---|---|---|
| 1 | implement as one request per story with a two-follow-up cap | −0.70 | −0.53 | input per session about 15k tokens written (facts 13 KB, files about 40 KB), 1.5 requests: 0.09 against first request plus growth 0.24 (strong) and 0.22 (mixed) |
| 2 | review on the smallest fact set | −0.46 | −0.36 | input to a third: changed files instead of the tree, no diff text |
| 3 | intake, direction and `plan.decompose` as one request each | −0.17 | −0.04 | about 3k tokens per request against first request plus growth 0.29 on Sonnet; in mixed only `plan.decompose` is Sonnet, the Haiku nodes cost 0.04 in all |
| 4 | `spec.author` as one request with one follow-up | −0.13 | −0.10 | 11k tokens written, 1.5 requests: 0.07 per session against 0.11 to 0.13 |
| | together | −1.46 | −1.03 | strong 5.50 → about 4.0; mixed 2.55 → about 1.5 |

Build first, in this order:

1. **Review on the smallest fact set** (`story:review-minimal-facts`, record
   `review-minimal-facts`). It changes only what `llm-node.sh` inlines; the node, its schema and
   its checks stay. The review's reply reaches later steps only as its decision, so a recorded
   replay re-asks only the reviews (`--on-divergence live`) and compares the decisions.
2. **One structured request per node** (`story:structured-request-nodes`): one adapter mode (response
   schema, no tools, the program writes the files, follow-ups on a refused check), used first by
   `spec.author` and the eight intake, direction and planning nodes.
3. **Implement as one request per story** (`story:implement-one-request`): the same mode, the edit
   schema, `cargo build` and `tests.run` between follow-ups. The largest saving and the largest
   risk: a node that cannot look things up builds against what it was given.

## Commands

The runs live in `runs/` of the SDC checkout (not in git). Per session (one line per call):

```console
# llm-calls.jsonl line i is the call of llm/<i>-<action>/ (directories in name order)
jq -c '{action, class, model, num_turns, usage, cost_usd}' runs/<run>/llm-calls.jsonl
# first request: the first `usage` event per request_id, in seq order
jq -s '[.[] | select(.event=="usage")] | group_by(.request_id) | map(last) | sort_by(.seq) | .[0].usage' runs/<run>/llm/<i>-<action>/events.jsonl
# tool calls by tool, and the bytes each Read/Glob/Grep returned
jq -s '[.[] | select(.event=="tool.requested")] | group_by(.name) | map({(.[0].name): length}) | add' runs/<run>/llm/<i>-<action>/events.jsonl
jq -s '(map(select(.event=="tool.result")) | map({key: .call_id, value: (.content | tostring | length)}) | from_entries) as $s
  | map(select(.event=="tool.requested" and (.name=="Read" or .name=="Glob" or .name=="Grep")))
  | map({path: (.input.file_path // .input.path // .input.pattern), bytes: $s[.call_id]})' runs/<run>/llm/<i>-<action>/events.jsonl
du -sb runs/<run>/llm/<i>-<action>/cwd/in
```

Growth = (cache write × write price + cache read × read price + input × input price of the whole
session) − the same of the first request.
