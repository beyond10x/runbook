# SDC phase model (the operator, 2026-11-20)

Principle: the engine derives the stage from facts, never from a stored "current phase". A fact is
a file in the repository or a recorded result of a run. At any moment the engine states where the
case is, what is missing, and what is next. A fully specified ESS is a milestone that brings the
case close to implementation; completeness is computed by validating, not declared.

## Flow

1. **Input.** A person gives 3 words up to a short paragraph.
2. **Dissection.** The input is split into typed items:
   - fact (stated as true)
   - wish (wanted outcome)
   - belief (stated, unverified)
   - unknown (named gap)
   - ambiguity (two or more readings)
   - dark spot (nothing said, but needed)
3. **Interview.** Each open item becomes a question with a default answer that can be accepted as is.
   Answers are normalised into a requirements list: a structural, product-level description of what
   should be built.
4. **Defaults for project context**, each changeable and each recorded as an assumption:
   - audience
   - source language and target language
   - target platform and operating systems
   - kind of thing: service, multi-tenant application, CLI, library
   - deployment and licence
5. **Vision.** One approved document that guides direction.
6. **Roadmap.** The outermost planning structure. The user approves it before any specification is written.
7. **Epics** under the roadmap.
8. **Specification (ESS)** per epic, from one of three sources:
   - authored: the system is described from the requirements (states, actions, events)
   - reused: an existing specification from another repository or a public place
   - reconstructed: derived from an RFC, an OpenAPI document or an observed system
9. **Spec complete** = the ESS gate passes at the current spec revision (validate, compile,
   conformance synthesize with 0 refusals, no open-question marker).
10. **Stories, implement, review, test, release, operate.** ESS conformance is the verification evidence.

## Mapping to tools (verified unless marked inferred)

- Planning store: `aep plan artifact new|move|relate|scope|waves|validate`. Artifact kinds in
  `canon/.engineering/planning/`: vision, epic, story, release-plan, approval-record, blocker,
  decision-blocker, review-result, verification-report. A roadmap kind: not verified.
- Reconstructed spec: `ess:retrofitting` skill (OpenAPI, Kubernetes, code). RFC as a source: not verified.
- Existing repository to plan: `aep plan reverse`.
- Human approvals (vision, roadmap, accepted defaults, scope-changing answers) are Canon authority
  decisions bound to the revision they approved.
