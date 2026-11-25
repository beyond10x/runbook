---
format: aep.planning-md/3
id: story:ci-component-runs-a-runbook-in-a-pipeline-job
kind: story
status: draft
title: A CI/CD component runs a Runbook run in a pipeline job and links the job to the run
summary: A component template submits a run with an idempotency key, waits for its end, follows its outcome and writes a job annotation that links to the run.
tags:
- forge-gitlab
scope:
- confidence: cited
  path: docs/cli.md
- confidence: cited
  path: docs/daemon-api.md
- confidence: inferred
  path: docs/forge.md
- confidence: inferred
  path: runbook/src/client
- confidence: inferred
  path: scripts
revision: 6
---
## What is missing

A team on a GitLab instance wants a pipeline job to start a Runbook run, wait for its end, and fail or pass the job by the run's outcome, with a link from the job to the run. Runbook gives a job the parts and no job.

Read on `main` at `81079535`:

- `runbook run <runbook> --intake <file|-> … --detach` posts `submit-run` and prints the run id (`docs/cli.md:33`); `runbook runs show`, `runbook runs events` and `runbook runs decisions` read the run, its events and its open requests (`docs/cli.md:42-45`).
- A run's states are Submitted, Running, WaitingOnDecision, Ended, Cancelled and FailedToStart, and `Ended` carries an open-text `outcome` (`docs/daemon-api.md:19`).
- The daemon takes a bearer token of at least 32 bytes and never logs it (`docs/daemon-api.md:268-276`); the client has no TLS: `https://` is refused with exit 64, and a token to a daemon off loopback "travels in clear over plain HTTP" (`docs/cli.md:20-23`).
- `submit-run` takes an `idempotency_key` (at most 200 characters); a second submit with the same key answers the first run, `already-submitted`, status 202 (`docs/daemon-api.md:62-69`).

Nothing in the repository describes a pipeline job. The claims below about the forge are from reading its public documentation and are to be checked against the documentation of the instance's version before building; each was marked read or inferred there:

- a CI/CD component is a template file with `spec:inputs`, published from a catalog project by release tag and called with `include: component:` (read);
- a job's `artifacts:reports:annotations` takes an `external_link` with a label and a URL, and puts the link on the job page; the defaults are 20 annotations and 80 KB per job (read);
- components work only within one instance (read).

## Outcome

1. A component template, kept in this repository and documented, runs one Runbook run in a job. Its inputs are the runbook, the intake (a file or text), the autonomy level and decider kind, the confinement, the context and the daemon's address; the token comes from a masked variable of the project.
2. The job submits the run with an idempotency key built from the pipeline and job (so a retried job reaches the same run), waits for the run to leave the states Submitted, Running and WaitingOnDecision, and ends with a status that follows the run's `outcome`: success for an outcome the input lists as a pass, failure for any other end, a cancel or a failure to start.
3. A run that waits on a person action does not hold the job open past a bounded wait: the job ends with a status that says it is waiting and names the open requests.
4. The job writes an annotation file that links the job to the run (the run's page of the daemon's UI where `--ui` is on), and the run's id and outcome are in the job log.

## What a guard must hold

- The token is never printed in the log, never written to an artifact and never put in the annotation; the component warns when the daemon's address is not loopback, as the client does (`docs/cli.md:20-23`).
- A job's success follows the run's recorded outcome, never the exit of a helper; an unknown outcome is a failure.
- The wait is bounded and the bound is an input of the component.
- The component takes no autonomy or decider from the pipeline's own variables unless the project's input names it; a run's autonomy is the operator's choice.

## Acceptance

Doc claims about the forge first: a short note in `docs/` lists each claim above with the page it was read at, checked against the instance's version.

Then, against a daemon on the stub runtime, with the component's script run by a shell and no forge:

- a submit with the same idempotency key twice yields one run and the second answers `already-submitted`;
- a run that ends with a listed outcome ends the script with exit 0, an unlisted outcome ends it non-zero, and the run id and outcome are in its output;
- a run left waiting on a person action ends the script at the bound with a non-zero status that names the open requests;
- the annotation file is valid JSON with one `external_link` whose label and URL are the run's, and the token is in no output file.

## Scope

`docs/cli.md`, `docs/daemon-api.md`, `docs/forge.md` (new), `scripts/` (the component's script), `runbook/src/client`.

## Depends on

- None in the plan. `story:a-runs-end-is-a-commit-status-on-the-forge` builds on the same job.
