// The lanes of the UI tests, and the viewports each spec runs at.
//   gate  the test step (`task test` in ui/): nothing in it reads runs/ (git-ignored), the driver or a canon binary or
//         a server somebody else started, so it runs the same on a fresh clone and on a machine with every
//         run recorded. 120 s for the run, 60 s for a test (playwright.config.mjs).
//   slow  the specs below (`task slow`, playwright.slow.config.mjs): each needs something the gate does not
//         have, named here. Run it by hand or in its own job; the gate never starts it.
// Every spec file under test/ is in exactly one of three lists (budget.spec.mjs holds that): SLOW, BOTH, WIDE_ONLY.
export const SLOW = {
  'now.spec.mjs': 'plays the recorded run runs/trial-todo-1 in full (the agent, two, changes scenarios)',
  'changes.spec.mjs': 'plays the recorded run runs/trial-todo-1 in full and serves its store',
  'screens-runs.spec.mjs': 'tours the recorded runs runs/full-cycle-1 and runs/trial-todo-1, billing cases included',
  'meaning-runs.spec.mjs': 'reads the specification files of runs/trial-todo-1',
  'live.spec.mjs': 'replays runs/full-cycle-1 into a store the caller names (SDC_UI_LIVE_DIR, SDC_UI_LIVE_SOURCE)',
};
export const slowFiles = Object.keys(SLOW);

// The gate's specs that run at both viewports, `wide` 1600x1000 and `laptop` 1280x800: their cases assert what is drawn
// at a size. A file that is not here and not in SLOW runs in `wide` only.
export const BOTH = {
  'screens.spec.mjs': 'each view at both sizes',
};
export const bothFiles = Object.keys(BOTH);

// The gate's specs that run in `wide` only: their cases assert a model function, a route's answer, a guard of the
// server, a refusal, a key or token rule or a text, none of which depends on the viewport. One line per file, the reason
// in five words. A new spec file must be added to one of the three lists.
export const WIDE_ONLY = {
  'autopilot.spec.mjs': 'who decides, said as text',
  'budget.spec.mjs': 'file checks, no page',
  'child.spec.mjs': 'process guard, no page',
  'daemon-decide-review.spec.mjs': 'daemon refusals and page text',
  'daemon-decide.spec.mjs': 'token rules and answer flow',
  'daemon-ui.spec.mjs': 'daemon serves, page lists runs',
  'decisions-reads-the-run.spec.mjs': 'pure reader refuses other runs',
  'decisions-yaml-parses-what-was-read.spec.mjs': 'in-process parse, no page',
  'event-log-lines.spec.mjs': 'server survives bad lines',
  'good-end.spec.mjs': 'end-of-run wording, a text',
  'graph-any-runbook.spec.mjs': 'graph labels per runbook',
  'holder-model.spec.mjs': 'model function, no page',
  'lenses-escape.spec.mjs': 'model check, no page',
  'loom-run-record.spec.mjs': 'record shown as text',
  'markdown-bounds.spec.mjs': 'renderer bounds, no layout',
  'markdown-hostile.spec.mjs': 'hostile text rendered inert',
  'meaning-model.spec.mjs': 'model function, no page',
  'mobile.spec.mjs': 'phone size set by file',
  'now-model.spec.mjs': 'model function, no page',
  'paths-served.spec.mjs': 'served text names no path',
  'paths.spec.mjs': 'committed files, no page',
  'read-only.spec.mjs': 'no control acts, a text',
  'reads-only-the-run.spec.mjs': 'server reads only the run',
  'run-picker-experiments.spec.mjs': 'keyboard and grouping, no layout',
  'run-picker.spec.mjs': 'sizes set by test blocks',
  'runfiles-at.spec.mjs': 'file reader function',
  'runfiles-fail-closed.spec.mjs': 'reader refuses, no page',
  'runfiles-inventory.spec.mjs': 'inventory function, no page',
  'runfiles-opens.spec.mjs': 'reader opens only files',
  'security-products-2-blob.spec.mjs': 'blob guard of the server',
  'security-products-2-markdown.spec.mjs': 'render cost bound, no layout',
  'security-products-2-race.spec.mjs': 'swap race, no page',
  'security-products-2-roots.spec.mjs': 'root guard of the server',
  'security-products-3-reads.spec.mjs': 'read guard of the server',
  'security-products-3-roots.spec.mjs': 'root guard of the server',
  'security-products-listing.spec.mjs': 'listing bound of the server',
  'security-products-markdown-cost.spec.mjs': 'render cost bound, no layout',
  'security-products-output.spec.mjs': 'output bound of the server',
  'security-products-page.spec.mjs': 'hostile text shown as text',
  'security-products-paths.spec.mjs': 'path guard of the server',
  'security-products-race.spec.mjs': 'swap race, no page',
  'security-products-routes.spec.mjs': 'route guard, Host and Origin',
  'security-products-runlog.spec.mjs': 'run-log guard of server',
  'slim-scrub.spec.mjs': 'committed fixture check',
  'stall-model.spec.mjs': 'model function, no page',
  'step-products-api.spec.mjs': 'route answers, no page',
  'step-products-bounds.spec.mjs': 'bounds of answers and page',
  'step-products-page.spec.mjs': 'sizes set by tests',
  'stopped.spec.mjs': 'both sizes set per test',
  'stub-runs.spec.mjs': 'sizes set by test blocks',
  'view-loom-store.spec.mjs': 'view file reads a store',
  'view-product.spec.mjs': 'server refuses a bad view',
  'view-race.spec.mjs': 'view swap race, no page',
  'view.spec.mjs': 'view file rules, a text',
};
export const wideOnlyFiles = Object.keys(WIDE_ONLY);

// A wide-only file whose code names a size or a position (a `viewport`, `setViewportSize`, `boundingBox`,
// `getBoundingClientRect`, `scrollWidth`, `toHaveScreenshot`, a screenshot, or a visibility check) is listed here
// with the reason its claims do not depend on the viewport the project gives. budget.spec.mjs fails a wide-only file
// that names one and is not listed.
export const SIZE_EXCEPTIONS = {
  'autopilot.spec.mjs': 'sets both gating sizes itself',
  'daemon-decide.spec.mjs': 'its phone test sets a size',
  'mobile.spec.mjs': 'sets the phone size itself',
  'read-only.spec.mjs': 'screenshot is a file, not a claim',
  'run-picker.spec.mjs': 'sets 805x1270 and 390x844 itself',
  'step-products-page.spec.mjs': 'sets 390 and 1366 sizes itself',
  'stopped.spec.mjs': 'sets both gating sizes itself',
  'stub-runs.spec.mjs': 'sets 805x1270 and 390x844 itself',
};
