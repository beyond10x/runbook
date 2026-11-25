---
format: aep.planning-md/3
id: story:a-runs-end-is-a-commit-status-on-the-forge
kind: story
status: draft
title: A run's end sets a commit status on the forge, pending while a person action is open
summary: A client of the daemon follows its events and posts a commit status per run, so a merge request can be gated on a run.
tags:
- forge-gitlab
relations:
- informed_by: story:ci-component-runs-a-runbook-in-a-pipeline-job
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: inferred
  path: docs/forge.md
- confidence: inferred
  path: runbook/src/client
- confidence: inferred
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 6
---
## What is missing

A team on a GitLab instance wants a Runbook run to gate a merge request: the merge request shows a pending state while the run works or waits on a person, and a passed or failed state when the run ends. Runbook tells the forge nothing about a run.

Read on `main` at `81079535`:

- A run is in one of Submitted, Running, WaitingOnDecision, Ended, Cancelled or FailedToStart; `Ended` carries an open-text `outcome` and `FailedToStart` a `failure` (`docs/daemon-api.md:19`).
- A run waiting on a person action has an Open decision request, listed by the view `open-decisions` (`docs/daemon-api.md:101-112`).
- The events of every run of the daemon are a stream a client can follow and resume by position (`docs/daemon-api.md:163-229`, view `daemon-events`).
- The daemon holds no credential of any forge and speaks plain http (`docs/daemon-api.md:268-276`, `docs/cli.md:20-23`).

The claims below about the forge are from its public documentation and are to be checked against the documentation of the instance's version before building:

- `POST /projects/:id/statuses/:sha` sets a commit status with `state`, `name`, `target_url` and `pipeline_id`, on every tier; it creates a job in an `external` stage (read);
- that a status keeps a merge request from merging, through the project's "pipelines must succeed" setting, is an inference and not read;
- the token scope and the role the call needs were not found;
- the instance's external status checks (a licensed feature) are a different mechanism and are not part of this story.

## Outcome

1. A run submitted with a forge context (a project and a commit) has a commit status on that commit, named by the runbook: `running` while the run is Submitted or Running, `pending` while it is WaitingOnDecision, `success` when it ends with an outcome the runbook lists as a pass, `failed` for any other end or a failure to start, `canceled` for a cancel. Its `target_url` is the run's page.
2. A small client of the daemon, not the daemon, follows the daemon's events and posts a status when a run changes state; it resumes by position after a restart and never posts a state older than the one it posted.
3. A person action that stays open keeps the status `pending` and names the open request; the answer moves it.
4. A post the forge refuses (unknown commit, stale commit, no right) is logged by name and retried within a bound; it never changes the run.

## What a guard must hold

- The daemon gets no forge credential: the token lives in the client, in a file of mode 600, and is never logged, echoed or put in a `target_url`.
- A run's recorded state decides the status, never a text a program of the run printed; the table from outcome to `success` or `failed` is the runbook's, and an outcome not in it is `failed`.
- A commit and project the run was not submitted with are never posted to.
- Posting the same state again is harmless; the client keeps the last state it posted per run and commit.

## Open questions, each with the option recommended

- Where a run learns its commit and project. Recommended: the submit's settings, set by the component of `story:ci-component-runs-a-runbook-in-a-pipeline-job` or by the webhook receiver; the daemon carries them as text and reads none of them.
- What "pending while a person action is open" maps to. Recommended: the forge's `pending`, since the run is not finished and nobody has failed it.

## Acceptance

Doc claims about the forge first: the note of `story:ci-component-runs-a-runbook-in-a-pipeline-job` lists these claims, checked against the instance's version, including the merge-blocking effect and the token's scope.

Then, with the stub runtime and a fake forge that records the calls:

- a run that ends with a listed outcome produces `running` then `success` on its commit; an unlisted outcome produces `failed`;
- a run that waits on a person action produces `pending`; after the answer the status follows the run;
- after a restart of the client, no state is posted twice and none older than the last is posted;
- a forge that answers 4xx for the post leaves the run's state unchanged and the client's log names the refusal; the token is in no log.

## Scope

`docs/daemon-api.md`, `docs/forge.md`, `runbook/src/client`, `runbook/src/main.rs`, `runbook/tests/daemon`.

## Depends on

- Informed by `story:ci-component-runs-a-runbook-in-a-pipeline-job`: the component is where a run gets its commit and project.
