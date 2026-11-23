# What the SDC prototype needs from Canon

From: the `sdc` prototype (`sdc/`, beside `canon/` in the workspace), 2026-11-20. Evidence and reasoning:
`sdc/docs/boundary.md`, `sdc/docs/limitations/output.txt` (reproduce with `sdc/scripts/limitations.sh`),
`sdc/docs/canon-simulation-request.md`, `sdc/docs/driver.md`.

Each item says what, why (with the file that shows it), and whether a Canon story exists
(read from `canon/.engineering/planning/story/`; other stories were not read for status).

## Kernel changes

1. **Authority grants bound to a case revision and a principal.**
   `canon-authority/1` has only `capability` and `decision` (`documents.md:116-121`);
   `canon-decisions/1` already has `principal` and `case_revision` (`documents.md:140-148`).
   Today a grant given at spec r1 still holds at r2 (`sdc/docs/driver.md:146-148`).
   Related draft story: `capability-scope` (status: draft). Land both in one story (inferred).

2. **Usefulness in `canon-decision/1`.** Per admissible action: does some kind it may produce lack
   an applicable record at the current revision. The driver re-derives this (`sdc/driver/src/main.rs:162-176`)
   and its first run looped on an admissible, useless action (`driver.md:91-92`).
   Canon's design lists "useful missing evidence" (`canon-protocol-calculus-design.md:282-291`).
   No story found.

3. **Checker beyond 65,536 states.** 4 of 8 limitations are this bound (`output.txt:2,6,10,26`).
   In 2 of them the bound changed what the protocol means: intake lost its own abandon outcome,
   the story protocol dropped plan and design. Options (inferred): check independent dimensions
   separately; bounded or exhaustive walk (see item 4).

4. **`canon simulate`**: exhaustive walk over action sequences. Stuck states with the shortest
   action path, cycles, dead actions, no outcome reachable. Detail: `canon-simulation-request.md`.
   Exists on Canon branch `wave/2026-11-20` at `49ff2ce`, unreleased (the Canon session,
   2026-11-21); uptake in items 9-11.

5. **Child outcome bound to the parent's current revision.** Canon counts a child's outcome whatever
   its subject revision (`protocol.md:194-196`), so a spec revised after its stories passed still
   leaves `children.passed` true. Validate error `inert-invalidation` (`output.txt:30`).
   Needs a decision: a "current revision" option on the `children` form, or an explicit reopen.

6. **Parent-to-child evidence.** Child to parent exists (`case_outcome`); the reverse has no declared
   form. Check reports 8 `unproduced-evidence` findings for an imported kind (`output.txt:14-22`).
   Related draft story: `protocol-imports` (about protocol imports, not case evidence; relation inferred).

7. **Pure argument validator exported from the Canon library.** The dialect is Canon's;
   `protocol.md:92` leaves checking to every runner, so each reimplements a partial one
   (`main.rs:240-254`, `driver.md:259`).

8. **Optional, low value:** an explicit "absent" artifact revision (`output.txt:34`).

9. **Release of `canon simulate`** with waves 6-8 (`simulate-child-outcome-step`,
   `simulate-record-groups`, `simulate-revise-step`). 87 findings on SDC's protocols at `f364b6b`,
   most of them walk artifacts until those waves land. SDC story
   `story:simulate-as-a-protocol-gate`; blocker `upstream-blocker:canon-simulate-release`.
   Released: Canon `0.10.0` (2026-11-21, commit `2c7a1ea6`) carries `simulate`; SDC's simulate
   step is pinned to it since SDC 0.1.1, and its reports on the nine protocols are byte-equal to
   those of the earlier pin `a1a937ec` (71 findings on the eight cycle protocols, 0 on
   `sdc-bugfix`).

10. **`revises`, `author` and `decided_by` in a released `protocol/1`.** Accepted on the branch with
    no evaluation meaning yet; the revise step reads them. SDC story
    `story:protocols-declare-revises-and-authors`; same blocker.

11. **`records_together`** (Canon `story:simulate-record-groups`, wave 7) for `questions.check`.
    SDC story `story:questions-check-records-together`; blocker
    `upstream-blocker:canon-simulate-record-groups`.

## SDK (the operator's direction, not yet evidenced in the repository)

- Core stays Rust. Delivered as a library with bindings (JavaScript, Python, further languages
  after those), so workflows can be built programmatically.
- Target: condition- and evidence-driven workflows, not directed acyclic graphs.
- Design file says Canon "should remain usable as a pure library" (`canon-protocol-calculus-design.md:1235`)
  and the CLI "a thin shell over pure library semantics" (`:1358`).
- A search of `canon/` for `napi`, `pyo3`, `wasm-bindgen`, `uniffi`, `cdylib` found nothing:
  no binding exists today.
- Open question for the Canon session: a stable library API (parse, compile, evaluate, check)
  that the bindings wrap, and a builder that emits `protocol/1` so a program does not write YAML.

## Not Canon (for orientation)

- Child-case runtime (open child on `case_open`, write `case_outcome` back): Loom, not Canon.
  Blocks the SDC run at `story.open` (`driver.md:249-251`).
- Move each binding's `until` claim into the SDC protocols' preconditions: SDC edit.
