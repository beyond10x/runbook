---
format: aep.planning-md/3
id: story:ci-runs-the-driver-tests
kind: story
status: proposed
title: The forge runs the driver tests the local gate runs
summary: pages.yml runs one binary's tests; test:driver with its 60 s kill and 120 s step is local only (review rb2 S3-8).
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: .github/workflows/pages.yml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:42:20Z", actor: "human:operator", revision: 2, executor: "agent:rb2-driver-fixes", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ci-runs-the-driver-tests

## Outcome

The forge runs the driver tests the local gate runs, with the same 60 s kill per test and the same
120 s step, so a green local gate and a green forge say the same.

## Context

Source: review rb2, driver, S3-8: `.github/workflows/pages.yml:92` runs
`cargo test ... --bin sdc-protocol-site` only; `task test:driver` (the kill and the step budget) is
local. Against the rule "the local gate equals the forge".

## Acceptance

A pull request that makes one driver test fail turns the required forge check red, and the check
runs `task test:driver`.
