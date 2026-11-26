---
format: aep.planning-md/3
id: story:daemon-evaluates-trigger-rules
kind: story
status: draft
title: The daemon evaluates trigger rules beside its schedule and starts runs within its gates
summary: 'Rules held by a consumer, a kernel in Canon, the evaluator in the daemon: sources, baseline, dry-run view, class gate, per-rule approval, kill switch, firing record.'
tags:
- consumer-asks-2026-11-26
relations:
- depends_on: story:daemon-submits-a-runbook-on-a-schedule
revision: 1
---
# Story: daemon-evaluates-trigger-rules

## Origin

The operator's decision Q8, 2026-11-26 about 12:05 CET (relayed by the host's operator session): "A - kernel in canon (for simulation purposes) and evaluator in runbook". The trigger-rule kernel (declaration form and `evaluate` semantics, usable in simulation) lives in Canon; the evaluator lives in runbook's daemon beside `story:daemon-submits-a-runbook-on-a-schedule`. The consumer (dev-operator) holds the rules and the readers of its entities. The request below is the consumer's text, unchanged.

## Depends on

- Canon: the trigger-rule kernel (requested through the host's operator session; no Canon session runs).
- `story:daemon-submits-a-runbook-on-a-schedule`, `story:a-run-waits-for-its-serial-key`.


Neutral wording: this request names no customer, company, host or path, and may be sent to a public repository as it is.
Status: draft request, not sent. Sender: the coordinator. Companion: `canon-trigger-kernel.md`, which defines the rule
declaration and the `evaluate` operation this request consumes.

## Problem

A consumer wants runs to start from the state of things outside the daemon: a merge request whose pipeline failed, a
ticket that reached a state, a question that went unanswered. The daemon knows runs, not those things. The consumer holds
the rules and the readers of those things; the daemon should hold the loop that evaluates the rules, starts the runs and
shows what it would start, once, for every consumer.

## What is asked

1. **Beside the schedule.** The loop that submits a run on a schedule (story `daemon-submits-a-runbook-on-a-schedule`)
   also evaluates a trigger rule set. One loop for time and for state; one place for the caps and the daemon's refusal.
2. **Sources.** A source is a program step, run on an interval, that prints rows: an id, the state fields, and the time
   the row entered its state. The daemon builds the snapshot of the kernel from them. A source that fails keeps the last
   good snapshot and marks it stale; a stale source starts nothing.
3. **Rules.** A rule set is handed to the daemon like its other configuration, in the kernel's declaration form. A rule's
   state (draft, active, disabled) is the daemon's; a change of state is a call that records who made it.
4. **Evaluation.** Each interval the daemon calls the kernel's `evaluate` with the snapshot, the protocols' declared
   classes, the history and `now`. For each `would-start` it submits the run with the firing id as the idempotency key
   (the submit's existing used-key behaviour answers the first run for a repeat) and records the firing.
5. **Baseline.** At activation the rows that already match are recorded as the baseline, listed, and never started.
6. **Caps.** The rule's cap per day, the run's own spend cap, and the daemon's global holds bind a rule's start as they
   bind a start by hand.
7. **Dry-run view.** A read-only view, and the same through the API, that returns the plan of `evaluate` now (every
   verdict), for the active rules and, on request, the drafts or one rule under test. It starts nothing and writes
   nothing.
8. **Class gate.** A rule starts a protocol of declared class read-only by itself. A write protocol starts by itself only
   for a rule version that has an approval record naming that exact version, made by a person the daemon's configuration
   names, and only while a daemon-level switch for automatic writes is on (off by default). A protocol with a guarded
   step never starts from a rule: publication refuses it, and the evaluator refuses it again at start.
9. **Per-rule human yes.** Every automatic write rule version needs its own approval; a new version of the rule starts
   nothing until approved. Offering a match (a way to start it by a person) needs no approval.
10. **Kill switch.** One setting stops all rule starts at once; a rule's disable stops that rule.
11. **Firing record.** Each start is recorded: rule key, row, entry time, firing id, run id.

The consumer supplies the rule files, the source programs, the approvals' names and the page that reads the view. The
daemon holds nothing of any consumer's systems.

## Acceptance statements

1. With a fake source of 7 rows in the matching state, activating an `auto` read-only rule starts no run and the dry-run
   view lists 7 `baseline`; a new row that then enters the state makes exactly one submit with the firing id as its
   idempotency key and one firing record.
2. A second evaluation of the same entry makes no submit; a row that leaves the state and re-enters makes a second.
3. The dry-run view over a fixture lists the expected rows and verdicts, performs zero submits, and leaves the daemon's
   state byte-equal.
4. A rule with the day's cap reached lists `held` and starts nothing; the next day it starts again.
5. A write protocol in an `auto` rule starts nothing while the automatic-write switch is off, and lists `held` with the
   reason; with the switch on, version 1 with an approval starts, and version 2 without one starts nothing.
6. A rule naming a protocol with a guarded step is refused at publication and, if forced into the set, at start.
7. A rule whose declared access flag differs from the protocol's declared class starts nothing and lists `refused`.
8. A failing or stale source starts nothing and the view says so; the kill switch stops all starts at once.
9. The daemon restarted between two evaluations still treats the same entry as fired (the history is persistent).

## Open questions

- Where the rule set and the history live: the daemon's configuration directory and its run store, or a store of their
  own.
- The source contract: one row format for every source, with a version, and whether a source may be a long-lived reader
  instead of an interval program.
- Who may make the approval and the daemon-level switch calls: the daemon's configuration names them; the request does
  not name a person.
- Whether the interval is global or per source.
- How a failed run is retried: by a person (the consumer's button), never by the evaluator, unless a rule says so.
