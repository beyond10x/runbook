---
format: aep.planning-md/3
id: epic:bug-ticket-work-is-protocol-steps
kind: epic
status: draft
title: The work from a bug ticket to a verified fix is one protocol with bound steps
summary: The eleven steps done by hand around the bug-fix runbook become steps of a parent protocol that opens the bug-fix runbook once per repository; an organisation supplies bindings and data.
relations:
- informed_by: epic:ticket-to-merge-request-on-a-real-repository
- depends_on: epic:concurrent-child-cases
revision: 1
---
## The operator's words

2026-11-23, after a ticket was fixed through the bug-fix runbook of release 0.8.0 with eleven steps done by
hand around it: "this is about building a reliable process for runbook. So making specific changes to
agents.md, which basically have an impact on that workflow, would be wrong. I mean, I'm not saying your
previous changes are entirely wrong, but the idea is to put process descriptions and rules into runbook
protocols. That's the whole point."

## Outcome

The work from a bug ticket to a verified fix is one protocol with bound steps. A session that runs it reads
no rule from an instructions file: the protocol names each step, who or what performs it, and the outcomes
it can end with. The bug-fix runbook that exists (`sdc-bugfix`) is one step of it, opened once per
repository.

## The steps observed, outside any protocol today

1. `ticket.read`: the tracker issue, its comments and a linked chat thread. Outcomes: read; an open product
   question.
2. `cause.locate`: repositories and lines; one ticket can become several bug cases. Outcome: located in N
   repositories.
3. `duplicates.check`: open merge requests and newer branches that touch the same files. Outcomes: none;
   held by an open merge request.
4. `question.decide`: a person picks from lettered options with a default (a person action).
5. `fact.read`: a read-only lookup in a running system that settles the question. Outcomes: confirmed,
   refuted, unreadable.
6. `checks.prove`: write a `sdc.checks/1` file and its script, then prove it on probes: a passing test is
   pass; a failing assertion is red; an exception that is no assertion is not red; a test that does not
   compile is not red; the suite is green; all within the time bound and with a cleared environment.
   Outcomes: proven; not provable. The consumer names this the step most worth a protocol: a wrong red rule
   makes every later run report falsely.
7. `intake.write`: the ticket text, the expected behaviour, the constraints per repository, notes for the
   writer of the reproducing test.
8. `bugfix.run`: the bug-fix runbook per repository (exists).
9. `finding.report`: each defect seen in a tool goes to that tool's owner with the user's words and the
   evidence, as a record with an acknowledged hand-over.
10. `handback`: a branch, a commit and a merge request from the changed tree against its base; a person
    action first (`story:bugfix-hands-back-a-branch-and-a-merge-request`).
11. `verify`: the fix observed on the development environment after the merge; then the ticket moves.

## Open, to settle before decomposition

- Which steps are common enough for this repository. The operator's rule of the same day: a very common
  protocol can live here; what is specific to one organisation lives in that organisation's own repository.
  The consumer's reading, not the operator's word: steps 1 to 7 and 9 to 11 are common, and an organisation
  supplies only bindings and data (its tracker and forge, a checks file per repository, the route for the
  read-only lookup).
- How the parent opens one bug-fix case per repository: this needs child cases of the runtime
  (`epic:concurrent-child-cases`).
- Ticket text is real data: it never enters this repository, a log line or an event; fixtures are invented.

## Order

After the units already in build for the same consumer (the gateway for model programs, the pool time of a
check, the step in flight). `checks.prove` is the first story, because every later step trusts its result.

## Origin

Relayed by the operator's host session on 2026-11-23 with the list above from the consumer session.
