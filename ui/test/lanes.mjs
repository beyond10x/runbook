// The two lanes of the UI tests.
//   gate  the test step (`task test` in ui/): nothing in it reads runs/ (git-ignored), a built binary or
//         a server somebody else started, so it runs the same on a fresh clone and on a machine with every
//         run recorded. 120 s for the run, 60 s for a test (playwright.config.mjs).
//   slow  the specs below (`task slow`, playwright.slow.config.mjs): each needs something the gate does not
//         have, named here. Run it by hand or in its own job; the gate never starts it.
export const SLOW = {
  'now.spec.mjs': 'plays the recorded run runs/trial-todo-1 in full (the agent, two, changes scenarios)',
  'changes.spec.mjs': 'plays the recorded run runs/trial-todo-1 in full and serves its store',
  'screens-runs.spec.mjs': 'tours the recorded runs runs/full-cycle-1 and runs/trial-todo-1, billing cases included',
  'meaning-runs.spec.mjs': 'reads the specification files of runs/trial-todo-1',
  'live.spec.mjs': 'replays runs/full-cycle-1 into a store the caller names (SDC_UI_LIVE_DIR, SDC_UI_LIVE_SOURCE)',
  'decisions.spec.mjs': 'needs a write server the caller started over copies of a trial store (SDC_UI_WRITE_URL)',
  'scope.spec.mjs': 'makes stores with the built driver (driver/target/debug/sdc-drive) and a canon binary (SDC_UI_CANON)',
  'preflight.spec.mjs': 'makes a store with the built driver and a canon binary',
  'paths-served.spec.mjs': 'makes a store with the built driver and a canon binary',
};
export const slowFiles = Object.keys(SLOW);
