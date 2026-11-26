---
format: aep.planning-md/3
id: story:a-consumer-repository-template-and-a-setup-skill
kind: story
status: draft
title: A consumer repository template and a setup skill
summary: A setup skill writes a consumer skeleton (pin file, task run, task doctor, task check, .mcp.json, AGENTS.md); task doctor fails on a version that differs from the pin.
tags:
- consumer-asks-2026-11-26-after-ui-wave
revision: 1
---
# A consumer repository template and a setup skill

## Origin

Request of a consumer repository, relayed by the operator session on 2026-11-26. Ordered after the run-page wave.

## Problem

A second consumer of runbook needs the same skeleton the first one built by hand: a pin file for tool versions, a
generic `task run <runbook>`, `task doctor`, `task check`, an `.mcp.json` for the daemon, and an AGENTS.md skeleton.

## Contract

- A setup skill writes the skeleton into an empty directory.
- `task doctor` exits non-zero when an installed tool's version differs from the pin, and names the tool.
- `task check` runs the bindings, protocol and view checks.

## Acceptance

1. In a scratch directory, running the skill writes the skeleton; `task doctor` and `task check` exit 0.
2. Changing one pinned version makes `task doctor` exit 1 and name that tool.
