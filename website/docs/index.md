---
title: What Runbook is
sidebar_label: What Runbook is
slug: /
description: "Runbook: agentic runbooks. A runbook is a fixed, checked way to handle a case. A program decides the next step from recorded facts; a model carries out single steps."
---

# What Runbook is

Runbook: agentic runbooks. A runbook is a fixed, checked way to handle a case. A program decides
the next step from recorded facts; a model carries out single steps. Runbook is a prototype that
runs on [Canon](https://github.com/beyond10x/canon) protocols.

The first runbook is the software delivery cycle (SDC): it takes an idea to a built, tested
product. Other runbooks are planned, not built: incident handling, investigations and ticket
triage.

In the SDC an idea goes through eight stages (idea, intake, direction, epic, spec, plan, story,
release), each a Canon `protocol/1` case with child cases. The stage is derived from facts (files
in the case's repository and recorded check results), never from a stored phase
([`README.md:3-6`](https://github.com/beyond10x/runbook/blob/0.1.1/README.md#L3-L6)). A ninth
protocol, `sdc.bugfix`, fixes one bug, red first
([`docs/bugfix.md`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/bugfix.md)).

## The name

Runbook is the name of the product: the driver, the protocol format, the run inspector, the
experiment tools and this site. SDC stands for software delivery cycle, the first runbook. The
binaries, environment names, format ids and the repository still carry `sdc`; they are renamed
later.

## The LLM is not the runtime

The driver `sdc-drive` runs one case tree on Canon. It reads Canon's decision for the case
(`canon-decision/1`), runs the tool bound to one admissible action, turns the tool's result into a
`canon-evidence/1` record and evaluates again. An LLM is only ever one action's executor: its
output becomes an artifact revision, never evidence and never the next state
([`docs/driver.md:3-6`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/driver.md#L3-L6)). Only
the bound programs record evidence about what the model wrote. The page
[The LLM is not the runtime](pathname:///runbook/llm-is-not-the-runtime) shows one run step by step.

## A run

A run drives one case tree from an idea to an end or a stop. `scripts/run-trial.sh <run> "<idea>"`
starts it; the driver takes one admissible action after another and stops with a named verdict when
nothing useful is left: `terminable` when a legitimate outcome exists, `awaiting_person`,
`writer_spent`, `stuck`, or a spent step, model-call or wall-time budget
([`docs/driver.md:164-172`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/driver.md#L164-L172)).
Decisions that belong to a person stop the run and print the command that takes them
([`README.md:11`](https://github.com/beyond10x/runbook/blob/0.1.1/README.md#L11)).

## Autopilot and the decider

Under autopilot (`--autopilot`, level A5) a decider model takes the place of a person and its
decision is final. It answers the scope-changing questions and the findings, approves or rejects,
and a stop that needs a person ends the run with a stop record by `rule:autopilot`
([`docs/autonomy-levels.md:449`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/autonomy-levels.md#L449)).
A rejected approval is not an end: the writer runs again with the reasons, and the third rejection
stops the run (`rework_spent`)
([`docs/releases/0.1.0.md:9`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/releases/0.1.0.md#L9)).

## A clean pass

A clean pass is a run that closed, delivered, with no reopen, no forced step and no unstick: the
records' pass rule `clean-delivery`
([`docs/workflow-metrics.md:177-178`](https://github.com/beyond10x/runbook/blob/0.1.1/docs/workflow-metrics.md#L177-L178)).
It is a Canon outcome closed by rule, not a product seen working: no run starts the delivered
product (`experiments/releases.yaml`, limits). The counts per release and model setting are on the
[evaluation](pathname:///runbook/evaluation) page.

## State

A prototype; no interface is stable. Releases are source releases at bare-version tags
([`README.md:15-17`](https://github.com/beyond10x/runbook/blob/0.1.1/README.md#L15-L17)).
