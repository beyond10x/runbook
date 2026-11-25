---
format: aep.planning-md/3
id: story:forge-job-tokens-are-verified-in-front-of-the-daemon
kind: story
status: draft
title: 'Design: verify the forge''s job ID tokens in front of the daemon and map a verified token to a principal'
summary: The daemon knows one bearer token; a verifier checks a job's signed identity token and gives the daemon a principal from the operator's table. Design first, to the operator before code.
tags:
- forge-gitlab
relations:
- informed_by: story:client-token-carries-its-principal
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: inferred
  path: docs/forge.md
- confidence: inferred
  path: runbook/src/api/auth.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 5
---
## What is missing

A pipeline job on a GitLab instance can carry a signed identity token of its own, so it need not hold a stored secret to reach a Runbook daemon. The daemon cannot read one: it knows a bearer token and nothing else.

Read on `main` at `81079535`:

- The token is one secret, at least 32 bytes, from `--token-file` or `RUNBOOK_TOKEN`; it decides only who may call (`docs/daemon-api.md:268-276`). The daemon acts as one principal fixed at its start (`docs/daemon-api.md:11`, `:253`); `story:client-token-carries-its-principal` is the story that would give a token a principal.
- The crate has no TLS client (`docs/cli.md:20-23`, `:182`), so any code that fetches keys from the forge over https is new to this repository.

The claims below about the forge are from its public documentation and are to be checked against the documentation of the instance's version before building:

- a job declares `id_tokens` with an audience and receives a JWT signed RS256, valid for the job's timeout or five minutes without one (read);
- the claims include `project_id`, `ref_protected`, `pipeline_source`, `sha`, `job_id`, `user_login` and `runner_environment` (read); the documentation says to trust `project_id` and not `user_login` (read);
- the public keys are served at `/oauth/discovery/keys` and `/.well-known/openid-configuration` on the instance (read on the cloud-services pages, not on the page of the tokens);
- in some 19.x patch releases a split hostname can break key retrieval (read).

## This story is design first

The change decides who may call a daemon that can spend money and answer decisions. The first part is a design (a `design` artifact related to this story with `designs`, and its text in `docs/daemon-api.md` once accepted), put to the operator before any code; the story stays at `draft` and no code starts until the operator has accepted the design, and the design of `story:client-token-carries-its-principal` is accepted too. The design answers, each with the option it recommends:

- where the verifier runs. Recommended: a separate process in front of the daemon on loopback, which hands the daemon its bearer token after a successful check, so no key-handling or https code enters the daemon; the other option is inside the daemon;
- how the verifier fetches and caches the keys, rotates them, and what it does when the instance is unreachable. Recommended: cache by key id with a bounded age, refuse when a key id is unknown after one refetch, and fail closed when nothing can be fetched;
- which claims it checks. Recommended: signature with RS256 only, `iss` equal to the configured instance, `aud` equal to the daemon's configured audience, `exp` and `nbf` with a small bound for clock skew, and `project_id` and `ref_protected` against the table below;
- the table from a verified token to a principal and a ceiling. Recommended: a table of the operator's, keyed by `project_id` and `ref_protected`, giving a principal of the form `agent:<id>` and the highest autonomy level the token may submit (for example, only protected refs may submit A3 and above);
- how a token that carries a user is treated. Recommended: the user is never the principal; `user_login` is not used;
- the https client the verifier needs, since the crate has none. Recommended: the verifier is its own program with its own dependency, and the crate stays without TLS.

## What a guard must hold

- The verifier fails closed: an unsigned or `none` token, any algorithm but the configured one, a wrong audience, an expired token, an unknown key, or a project or ref the table does not list is refused, and nothing is forwarded.
- A principal comes from the operator's table and nowhere a client controls; no claim is copied into a principal; the names `rule:`, `decider:` and `router:` are never produced.
- No token, key or claim beyond the project id and the principal is logged or echoed.
- A daemon started with its bearer token and no verifier behaves as today, byte for byte.

## Acceptance

Part one: the design artifact exists, names the recommended option of each question above and the doc claims checked, and carries an approval from the operator recorded against it; the story is not moved while it does not.

Part two, after that, with a verifier built to the design and a fake key server: a token signed by the listed key for a listed project and a protected ref reaches the daemon and its answers are recorded under the table's principal; a token with a wrong audience, an expired one, one signed by an unknown key, one for an unlisted project, one for an unprotected ref above its ceiling, and one with the algorithm `none` are each refused with a named cause and nothing reaches the daemon; with the key server down, a cached key still verifies within its age and an uncached one is refused.

## Scope

`docs/daemon-api.md`, `docs/forge.md`, `runbook/src/api/auth.rs`, `runbook/tests/daemon`.

## Depends on

- Informed by `story:client-token-carries-its-principal`: this story is the way a verified token becomes the principal that story lets a token carry. No code of either starts before both designs are accepted.
