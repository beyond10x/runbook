---
format: aep.planning-md/3
id: story:context-roots-are-keys-of-runbook-toml
kind: story
status: draft
title: Context roots are a key of runbook.toml and are created when missing
summary: context_roots in runbook.toml beside --context-root; a missing root is created with mode 0700 instead of stopping the start
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:runbook-toml-carries-defaults
- informed_by: story:daemon-takes-a-delegation
revision: 1
---
## Outcome

`runbook.toml` names the daemon's context roots: `context_roots = ["<dir>", ...]`. The daemon reads them with
`--context-root`. A root that is missing is created (mode 0700) instead of stopping the start. A root that exists and
is not a directory, or that cannot be created, still stops the start by name.

## Context

Gap G5 of a consumer repository that dispatches agents through the daemon (2026-11-27, relayed by the host's
operator session).

- Context roots are an option only: `--context-root <DIR>`, repeatable (`runbook/src/serve.rs:28-30`). It goes into
  the daemon's `Config` (`runbook/src/serve.rs:242`; field `runbook/src/commands.rs:67`).
- At start each root is canonicalized. A missing root stops the daemon with `context root <path>: No such file or
  directory`, and a non-directory with `context root <path> is not a directory` (`runbook/src/commands.rs:1048-1053`).
  `serve` prints this as `state directory: ...` (`runbook/src/serve.rs:246`).
- `runbook.toml` has no key for them. The top-level keys are `OWN_KEYS`, `OPTION_KEYS` and `MEMBER_KEYS`
  (`runbook/src/config.rs:38-46`), and any other key is refused (`runbook/src/config.rs:662-666`).
- A submit's or a delegation's `workspace`/context path must lie under a root (`context_under_roots`,
  `runbook/src/commands.rs:2363-2370`). Otherwise it is refused as `context-outside-roots`
  (`runbook/src/commands.rs:275`).
- `docs/daemon-api.md:757` names the option as the only way.

## What this story does

1. `context_roots` is a key of `runbook.toml`: a list of paths. A relative path is taken against the file's directory,
   as `token_file` is (`runbook/src/routes.rs:174`). `~` is not expanded.
2. The daemon's roots are the union of the file's and the option's, each canonicalized once. `runbook config` prints
   each root with its source (file or option).
3. A root that does not exist is created with mode 0700 (its missing parents too, with the same mode), and the
   daemon's start log names it. A root that exists with group or other write bits is refused by name. A path that
   exists and is not a directory is refused as today. A root that cannot be created is refused with the OS error.
4. `docs/daemon-api.md` and the config document name the key.

## Acceptance

- A `runbook.toml` with `context_roots = ["ctx"]` and no `ctx`: the daemon starts, `ctx` exists with mode 0700, and a
  delegation whose `workspace` lies under it is taken.
- The same with `--context-root <other>`: paths under either root are taken, and a path under neither is refused
  `context-outside-roots`.
- A root that is a file stops the start, naming the key and the path.
- A root with mode 0777 stops the start, naming the mode.
- An unknown key beside `context_roots` is still refused.
- Each case runs under 5 s.

## Evidence

- The consumer's design, section 8, item G5: context roots are CLI only, and the daemon refuses to start when one is
  missing.

## Relations

- Informed by `story:runbook-toml-carries-defaults` (the file the daemon reads) and `story:daemon-takes-a-delegation`
  (a delegation's `workspace` lies under a root).
