# Probe: can takt run `sdc.story`?

**Verdict.** No, not as is, and not because of Canon. takt at `origin/main` (0.36.0, commit 25c62df) pins Canon 0.8.0, and that Canon validates, compiles and checks `protocol/sdc-story.protocol.yaml` without complaint. The `command_actions` route refuses or cannot express the story's bindings, so a bump of Canon in takt would change nothing.

Date: 2026-11-21. Read-only on every repository. Builds and runs in scratch (`$SCRATCH`), takt and Canon from `git clone --local` copies. No LLM call, no forge call, no commit in a real repository.

## 1. Static

- takt pin: `git -C ../takt show origin/main:Cargo.toml` gives `b10x-canon` and `b10x-canon-expr` tag 0.8.0; `Cargo.lock` resolves both to `cf17175`. engineering-protocols 0.15.2.
- `sdc.story` (revision 4) uses: `independence_dimensions` with `independent_on`, `invalidation` with `upstream`, `requires: capability`, an outcome `requires: decision`, a claim test `is: "false"`, `all`. It uses no `children`, `case_open`, `after`, `since` or `case_outcome`; those are in `sdc` and `sdc.epic` only.
- Canon 0.9.0 (`CHANGELOG.md`) adds `case_open`, `children` and `case_outcome` only. Nothing the story uses is newer than 0.8.0.

## 2. Parse test, Canon 0.8.0

`git clone --local ../canon $SCRATCH/canon-pin`, `git checkout 0.8.0`, `cargo build -p canon-cli --release --offline` (exit 0, 17 s, `canon 0.8.0`). On a copy of the story protocol:

| command | exit | output |
|---|---|---|
| `canon validate --path sdc-story.protocol.yaml` | 0 | `valid: protocol sdc.story revision 4` |
| `canon compile --path sdc-story.protocol.yaml` | 0 | `canon-ir/1`, 9,558 bytes |
| `canon check --path sdc-story.protocol.yaml` | 0 | `12288 states, 0 properties, 0 findings` |

(The first attempt with a positional path exited 64: the CLI takes `--path`.)

## 3. takt run

`git clone --local ../takt $SCRATCH/takt-main`, `git checkout 25c62df`, `cargo build -p b10x-takt-cli --release --offline` (exit 0, 3 min 7 s, `takt 0.36.0`). Scratch git repository with a stub script; run file binding the story's five command actions (`spec.gate`, `change.implement`, `tests.run`, `change.review`, `change.merge`) to the stub, which writes one file under `{artifacts}` and exits 0.

1. `takt run --repo tree --harness none --forge none --iterations 6 --test-cmd true --intent ... --protocol story.yaml --config run.yaml --state-dir state` exit 2: `run.yaml is refused at line 10: command_actions binds an action takt's own validators perform`. `tests.run` is a name takt reserves for its own test validator (`validator.rs:1914`; also `gate.run`, `pr.open`, `ci.read`, `repository.merge`, `change.read`).
2. Same without the `tests.run` binding, exit 5: takt opened the case on `sdc.story/4`, Canon evaluated it, takt admitted `spec.gate` (the only action `may`), then `stopped: error: the validator's subject has a revision`; 1 step, no evidence record written. Cause: a command action's evidence subject is the fixed string `implementation` (`command_action.rs:372`), and `sdc.story` has no such artifact (`spec`, `change`, `findings`, `requirements`).

Not reached, read from source: results are only `pass`, `unchanged`, `done`, `fail` (`command_action.rs:220`), while the story claims read `passed`, `approved`, `failed`, `rejected`, `finding`; a record has one fixed producer and no independence dimension, so `change.reviewed` (`independent_on: [different_principal]`) cannot hold; `effect: write` actions (`change.implement`, `change.merge`, `amend.upstream`) are refused for connector actions and are untested for command actions; `change.merge` needs a `capability` authority, and takt's own merge grant is bound to `repository.merge`.

## 4. What it would take

- Not a Canon bump. takt stays on 0.8.0 for the story; 0.9.0 matters only for `sdc` and `sdc.epic` (child cases), which takt has none of.
- A change to takt's `command_action.rs` and run-file reader: a per-binding `subject` (an artifact of the case's protocol), a `results` map from exit code to result string, a `producer` and independence dimensions, and a free action name where the protocol does not use takt's built-ins (or a rename of `tests.run` in `sdc.story`, a protocol change). The ESS-gate claim is not read from the parent in this protocol: `spec.gate` runs the gate itself, so no claim passes in from the parent case.
- Child cases (`case_open`, `case_outcome`) and the `sdc` / `sdc.epic` protocols: no support, and none in a Canon-0.8.0 takt.
- takt is stalled (last commit 2026-11-13); these are changes to a frozen codebase. The comparison's path A (freeze and harvest) is confirmed.
