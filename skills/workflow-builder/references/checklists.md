# Checklists

## 1. Route table (Phase 1.2, one row per no-point)

| Column | Content |
|---|---|
| protocol / step | the action, with `path:line` of its declaration |
| actor in unattended mode | program, LLM node, decider class, rule |
| results that say no | `rejected`, `failed`, `invalid`, `refused`, cap reached |
| route on 1st no | writer sent back (name it, and how the reasons reach it) or end by rule (name the outcome) |
| route on last no | the end after the bound: outcome, recorded by which principal |
| bound | number, and where it lives (protocol fact, declared rule data) |
| fed back | does the writer see the reasons, including its own apply error |
| cap shared | does this send-back share a cap with another cause (it must not) |
| cycle | the stub cycle that proves the route red then green |
| visible to the static walk | yes, or why not |

Also list every writer with a run cap: what sends it back, what happens after the cap, whether a run
that called no model counts (it must not). In the worked example one such audit found 12 routes and
13 dead ends (`docs/reject-routes.md` on `audit/reject-routes`).

## 2. Decider input audit (Phase 3.2, once per decision class, on a real input)

- [ ] Every citable id is listed in the input and resolves; the schema names the citation forms.
- [ ] Only the conditions of this decision are shown as open; later obligations are under "met after".
- [ ] Every file the decision may doubt is present (lock files, build output, test logs, README).
- [ ] Tool metadata that looks like a verdict is labelled (`status` front matter of a planning tool).
- [ ] The prompt states the run mode: who else can be asked (in unattended mode, nobody), that the
      answer is final, and what to write when it cannot tell.
- [ ] The reply is schema-constrained, program-checked, retried at most twice with the check's error.
- [ ] A refusal by a program check names the content the decision wrote; older faults go to their writer.
- [ ] Earlier rejections of the same approval are in the input, so the decider checks they were addressed.

## 3. Run start check (Phase 5)

- [ ] Static gate green on the run's tree; tree commit recorded.
- [ ] Stub dry run on the same tree closes.
- [ ] Setting: strongest models for every class and the decider; effort recorded.
- [ ] At least 2 samples of the main input and 1 of the small input, started together.
- [ ] Caps large enough not to be the first stop (calls, decider calls, wall seconds); each cap's
      stop names its cause.
- [ ] Keep-going on, with a limit per run; defects go to a defects file.
- [ ] The run is labelled real; the UI or list shows it as running only while a driver is attached.
- [ ] Someone watches for the end and records it; a run that waits on nobody is stopped with a reason.

## 4. Classifying a run end

| End | Counts as |
|---|---|
| closed in the success outcome, no stop, no reopen, nothing forced | clean pass |
| closed in a failure outcome by rule (`not_delivered`, `withheld`) | ran end to end; the failure is a defect or a product fault, read the reasons |
| stopped with a cause | defect |
| reopened or forced, then closed | defect evidence; the close shows the fix works for that run |
| closed with a contradicting requirement or a stub review | pass with a product fault listed; state the limit |

## 5. Defect record (one line per defect)

`at`, run, tree commit, case, step, cause, full reason, record line (`console.txt:N` or event seq),
the decision or reply that led to it, calls so far, forced or not, class (missing route, cap
accounting, decider input, check placement, prompt, product), fix commit, cycle name.
