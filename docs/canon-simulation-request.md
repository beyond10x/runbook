# Canon request: exhaustive simulation of all cases

Source: the operator, 2026-11-20. Canon's main purpose is building reliable workflows, so it should
exhaust every case of a protocol deterministically and find issues, stuck places and logic errors,
the way ESS exhausts a state space.

## What `canon check` does today (read from canon/website/docs/reference/cli.md:19, 110-118)

- Enumerates every state: a set of evidence records, authority decisions and explicit decisions.
- Reports outcomes no state reaches.
- Reports actions whose precondition holds in no state.
- Reports claims, obligations, preconditions and outcome requirements that read evidence no action produces.
- Reports outcomes that rest on authority-requiring evidence yet hold without an authority decision.
- Checks `canon-properties/1`: an action's or outcome's status is independent of a claim.
- Refuses a state space above 65536 states.
- `canon generate` writes one minimal witness scenario per outcome and per blocked action.

## What it does not do (inferred from the same pages: none lists these)

- Treat an action as a transition from one state to the next (states are evidence sets, not paths).
- Report a stuck state: no admissible action, no legitimate outcome, case not terminated.
- Report a livelock: an action cycle that never reaches an outcome.
- Report an order of actions that reaches a state no declared outcome covers.
- Report an invalidation loop: a downstream action invalidates what an upstream one produced.
- Report an action that is admissible yet can never change any claim.
- Search beyond 65536 states (no partial order reduction, no bounded or random exploration).

## Requested

- `canon simulate`: breadth-first walk over action sequences from the empty case, each action adding
  the evidence it `may_produce` (each result variant), re-evaluating after each step.
- Output per protocol: reachable state count, terminal states by outcome, stuck states with the
  shortest action sequence that reaches each, cycles, dead actions.
- Same determinism rule as `evaluate`: byte-identical output for identical input.
- Where the space exceeds 65536 states: refuse, or run bounded with the bound in the output.
