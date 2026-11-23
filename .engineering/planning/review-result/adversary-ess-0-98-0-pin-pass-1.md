---
format: aep.planning-md/3
id: review-result:adversary-ess-0-98-0-pin-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 of the unit ess-0-98-0-pin at d85df2d7: green, 0 blockers, 3 warnings, 2 notes'
summary: Every archive and binary hash of the release equals the pin; the generated trees are byte-equal when regenerated; the guard refuses another binary by name. The changelog entry and two state files were completed in one commit after the pass; three older texts named a version two pins old.
relations:
- reviews: story:ess-0-98-0-pin
revision: 1
---
verdict: approve

unit: `story:ess-0-98-0-pin` at `d85df2d7`, tree `<trees>/b10x/runbook/ess-pin`, branch `impl/ess-0-98-0-pin`

cases: executed 0 to 0, red 0. No case added. The two committed guard cases, filter in `driver/`: `Summary [0.474s] 2 tests run: 2 passed, 663 skipped`, 313 s wall of which 192 s waiting for a slot, load 58.8 to 77.7.

origin: introduced 2 / pre-existing 3 / undecided 0, counted from the findings of the report.

What held: the reviewer downloaded the four archives and the checksum file of the release itself; every archive and every unpacked binary has the sha256 the pin file states, and the tag peels to the commit the pin names. The binary for this platform prints the pinned version. Regenerated with that binary, the JSON Schema (145 artifacts), the OpenAPI document, the authored suite and the synthesized suite are byte-equal to the committed trees, and the two digests equal the committed digest file; two syntheses gave identical bytes. The synthesized suite has 118 scenarios and 95 refusals against 112 and 101 with the earlier version: none removed, six added, and the six are the ones that were refusals before. No line of `driver/src/` or `driver/tests/` changed. A binary of the earlier version given through the environment is refused by name with both hashes; a pin file with one changed character of a hash is refused; a changed version gives the missing-binary refusal. Synthesis took 19.5 s to 52.6 s with the new version and 36.8 s to 48.7 s with the earlier one at a load of 58 to 89: no jump. The changelog entry stands under the unreleased heading and the released part is byte-equal to the base.

What did not hold: the changelog entry said none removed and six added and did not say that eleven existing scenarios carry new generated instance values. The two state files of the generated trees named the earlier version as their producer. Three texts named a version two pins old, the same on the base.

coordinator: one commit after the pass (`f0e6b876`) took the two introduced points and two of the three older ones: the entry names the eleven scenarios, both generated trees were removed and written again with the pinned binary and only their two state files changed, and the two task descriptions and the page of the daemon's interface name the pinned version without a measured time. The line of the readme is the note of an earlier release and stays as written. Not attacked by the pass: the tests of the `runbook` crate, the replay lane and the full driver lane; the implementor ran the driver lane (`665 passed`, 64.8 s).

```findings
[
 {"file":"CHANGELOG.md","line":13,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The entry must name changes to existing scenarios beside additions and removals: eleven existing scenarios of the synthesized suite carry new generated instance values. Taken in the commit after the pass."},
 {"file":"ess/generated/schema/.ess-output/state.json","line":1,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The producer recorded in the state files of the two generated trees must equal the pin. They named the earlier version while the check called the trees current. Taken in the commit after the pass."},
 {"file":"Taskfile.yml","line":284,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"A text that names the version of the pinned binary must follow the pin. Two task descriptions named a version two pins old with a time. Taken in the commit after the pass."},
 {"file":"docs/daemon-api.md","line":362,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"The page named a version two pins old with a time for the synthesized suite. Taken in the commit after the pass."},
 {"file":"README.md","line":149,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The line is the note of an earlier release and names the version that release ran on. It stays as written."}
]
```
