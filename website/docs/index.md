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
repository is `beyond10x/runbook`; the binaries, environment names and format ids still carry
`sdc` and are renamed later.

## The LLM is not the runtime

`runbook run` runs one case tree through Loom, on Canon. Canon's decision for the case
(`canon-decision/1`) names the admissible actions; the tool bound to one of them runs, its result
becomes a `canon-evidence/1` record, and the case is evaluated again. An LLM is only ever one
action's executor: its output becomes an artifact revision, never evidence and never the next state
([`docs/loom-run.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run.md)). Only
the bound programs record evidence about what the model wrote. The page
[The LLM is not the runtime](pathname:///runbook/llm-is-not-the-runtime) shows one run step by step.

## A run

A run drives one case tree from an idea to an end or a stop. `runbook run <runbook> --intake <idea
file> …` starts it; the runner takes one step of the tree through Loom after another and ends the
run when the tree ends, when it is stuck, or when a cap is reached: spend, model calls or wall time
([`docs/loom-run.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run.md)). A decision
that belongs to a person stops the run: with no autonomy level set, every decision does
([`docs/loom-run-settings.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run-settings.md)).

## Autopilot and the decider

Under autopilot (autonomy level A5 in the run's settings) a decider model takes the place of a
person and its decision is final. It answers the scope-changing questions and the findings,
approves or rejects, and a decision it cannot take ends the run as `stopped: decider`
([`docs/loom-run.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run.md),
[`docs/loom-run-decisions.md`](https://github.com/beyond10x/runbook/blob/main/docs/loom-run-decisions.md)).
A rejected approval is not an end: the writer runs again with the reasons. The driver's cap on
rejections is not ported.

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
