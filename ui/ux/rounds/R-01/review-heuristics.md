# R-01 heuristic review

2 pass, 7 partial, 6 fail (1 not observed)

One theme (dark). No recordings: every verdict cites screenshots in `shots/`. All 56 contact-sheet
frames reviewed; two extra shots taken from a read-only server on port 7530:
`shots/review-person-drawer-approve-1600x1000.png` (drawer scrolled to the Approve control) and
`shots/review-changes-spec-filter-1600x1000.png` (Specification filter after its fade).

| # | heuristic | verdict | evidence |
|---|---|---|---|
| 1 | One next thing | partial | `1600x1000-person-0.png`: one Decide cue in the Now band. `1600x1000-person-decided.png`: the drawer shows "Continue run", not Approve. `1600x1000-decider-0.png`: Decisions badge 1 while the decider is still deciding (UXF-001, UXF-006) |
| 2 | State in words | pass | `1600x1000-agent-0.png`, `1600x1000-two-end.png`: states in words (running, awaiting authority, ended passed, TRUE/FALSE/UNKNOWN, done/running under each phase) |
| 3 | Nothing required hidden | fail | Approve below the fold at every viewport: `1920x1080-person-decided.png`, `review-person-drawer-approve-1600x1000.png`. Model name cut: `1366x768-decider-0.png`, `1280x720-decider-0.png`. Graph and What changed gone: `1280x720-changes-end.png` (UXF-001, UXF-004, UXF-007) |
| 4 | Stable layout | fail | Pipeline moves 63 px: `1280x720-agent-0.png` against `1280x720-agent-end.png`. Legend wrap moves the graph 48 px: `1600x1000-two-0.png` against `1600x1000-two-end.png`. Opened row pushed down: `1600x1000-agent-end.png` (UXF-003) |
| 5 | Provenance of every value | partial | Good: diff header with action, case, Write #167, INFERRED tool-window, time (`1600x1000-agent-diff.png`); "state from evaluations/0006.json". Bad: two revisions and two clocks for one approval (`1600x1000-person-0.png`, `review-person-drawer-approve-1600x1000.png`, `1600x1000-decider-end.png`) (UXF-002) |
| 6 | Errors cheap | partial | One-click Change per context field (`1600x1000-person-decided.png`). The flagged unparsed change "broken.md … (not parsed)" has no control at all (`1600x1000-changes-0.png`) (UXF-005) |
| 7 | Words the user can say | fail | "Requireme…", "not reco", "implementation.requi…" (`1600x1000-agent-0.png`); "Dissecti…", "Spec re…", "Implem…" (`1280x720-closed-0.png`); tile codes R7 · L2, H1 · H2, F10; rotated rail labels (UXF-007) |
| 8 | The counterpart's time respected | partial | Wait timer and Decide are in the Now band (`1600x1000-person-0.png`), but the hidden Approve keeps the run waiting: person-decided and person-end are identical (`1600x1000-person-end.png`) (UXF-001) |
| 9 | Keyboard and pointer parity | fail | What changed rows are click-only divs; the last change is not a control at any viewport (`1600x1000-changes-0.png`, `1920x1080-changes-end.png`); #tree and .fd-list cannot be scrolled by keyboard (UXF-005) |
| 10 | Recognition over recall | partial | The Now card names the action, executor and model (`1600x1000-two-0.png`), but the user must compare hashes td1e3bfc8… and tdfd49c6… (`1600x1000-person-decided.png`) and remember what R7, H4 and F10 mean (`1600x1000-agent-0.png`) (UXF-002, UXF-007) |
| 11 | Calm by default | partial | Nine amber "not recorded" chips (`1600x1000-agent-0.png`); red "no driver attached" during a normal wait (`1366x768-decider-end.png`); graph mid-fade (`1366x768-two-0.png`); dialog mid-fade (`1280x720-agent-diff.png`) (UXF-009) |
| 12 | One state vocabulary | fail | "DECIDING" against "awaiting a person" (`1600x1000-decider-0.png`); "waiting on a person 29:17" with nobody waited on (`1600x1000-agent-0.png`, `1600x1000-two-0.png`); "phase Epics" while Implement runs (UXF-006) |
| 13 | A wrong cue recovered in one action | not observed | No scenario put a wrong cue on screen (no twist cards in R-01) |
| 14 | Unmistakable end state | pass | `1600x1000-closed-0.png`, `1920x1080-closed-end.png`: "CLOSED IDEA-1 ended operating", "Nothing running and nothing waiting", all phases done, 8/8, cost $0.10 |
| 15 | Readable in every theme; key data at 14 px | fail | Phase states 10.5 px and names 11.5 px in grey (`1920x1080-person-0.png`); 85 contrast nodes per page; dark theme only (UXF-008) |
| 16 | Focus follows the work | partial | Follow active case moves the graph to the running case (`1600x1000-two-end.png`), and focus returns to the opened row after the diff closes (`1600x1000-agent-end.png`). At 1280x720 the active graph has no height (`1280x720-changes-end.png`) (UXF-004) |

## Findings filed

| id | title | severity | class | sessions | cost |
|---|---|---|---|---|---|
| UXF-001 | The Approve control sits below the fold of the Decisions drawer | S1 | affordance | person, all 4 viewports | S |
| UXF-002 | One approval names two revisions and two waiting times | S2 | engine | person, decider, all viewports | M |
| UXF-003 | The page jumps as the Now band, the graph header and the change lists grow | S2 | layout | two, agent, changes (all); decider (1600, 1920) | M |
| UXF-004 | At 1366x768 and 1280x720 the page scrolls and the graph collapses | S2 | layout | all scenarios at 1366x768, 1280x720 | M |
| UXF-005 | What changed rows are not controls; the last change cannot be opened by keyboard | S2 | affordance | changes, all viewports; keyboard in all | S |
| UXF-006 | Regions name the same state differently and contradict each other | S2 | copy | decider, agent, two, changes, person | M |
| UXF-007 | Required labels are cut by ellipses, including the decider's model name | S2 | copy | all, worst at 1366, 1280 | M |
| UXF-008 | Key data is under 14 px and fails contrast | S2 | layout | all scenarios and viewports | M |
| UXF-009 | Alert colours for normal absence, and fades caught mid-transition | S3 | noise | agent, two, decider, person, changes | S |
