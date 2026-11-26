import path from 'node:path';
import {fileURLToPath} from 'node:url';

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
  'daemon-level-and-kind.spec.mjs': 'level and kind as text',
  'daemon-live.spec.mjs': 'sets the gating sizes itself',
  'daemon-steps-model.spec.mjs': 'model function, no page',
  'daemon-routes.spec.mjs': 'route as text, request rate',
  'daemon-run-tree-security.spec.mjs': 'hostile run text stays text',
  'daemon-run-tree.spec.mjs': 'tree, products, address as text',
  'daemon-start.spec.mjs': 'start form against own daemons',
  'daemon-ui.spec.mjs': 'daemon serves, page lists runs',
  'decisions-reads-the-run.spec.mjs': 'pure reader refuses other runs',
  'decisions-yaml-parses-what-was-read.spec.mjs': 'in-process parse, no page',
  'event-log-lines.spec.mjs': 'server survives bad lines',
  'good-end.spec.mjs': 'end-of-run wording, a text',
  'fixture-hold.spec.mjs': 'player ends, no page',
  'graph-any-runbook.spec.mjs': 'graph labels per runbook',
  'holder-model.spec.mjs': 'model function, no page',
  'idle-picker-window.spec.mjs': 'list states and stream events',
  'idle-reads.spec.mjs': 'reads counted by the server',
  'idle-server.spec.mjs': 'server answers while idle',
  'idle-slow-page.spec.mjs': 'page that never reads',
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
  'runfiles-cache.spec.mjs': 'read cache, in process',
  'runfiles-fail-closed.spec.mjs': 'reader refuses, no page',
  'runfiles-inventory.spec.mjs': 'inventory function, no page',
  'runfiles-opens.spec.mjs': 'reader opens only files',
  'runner-live-guards.spec.mjs': 'guards of the live runner',
  'runner-live-model.spec.mjs': 'model function, no page',
  'runner-live.spec.mjs': 'live runner, own server',
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
  'security-start-answers.spec.mjs': 'answers that end the key',
  'security-start-close.spec.mjs': 'closed form keeps no token',
  'security-start-content.spec.mjs': 'start form shows only text',
  'security-start-floor-reread.spec.mjs': 'floor read again on Start',
  'security-start-gone-runbook.spec.mjs': 'runbook left the list',
  'security-start-idempotency.spec.mjs': 'one filled form, one run',
  'security-start-inspector.spec.mjs': 'inspector stays read-only',
  'security-start-key.spec.mjs': 'life of the key',
  'security-start-late-answers.spec.mjs': 'answer after form reopened',
  'security-start-ready-window.spec.mjs': 'floor changes under the form',
  'security-start-second-paint.spec.mjs': 'picks kept across a repaint',
  'security-start-token-places.spec.mjs': 'token found in no store',
  'security-start-token.spec.mjs': 'token field rules',
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
  'written-api.spec.mjs': 'route answers, no page',
  'written-bounds.spec.mjs': 'bounds of answers, no layout',
  'written-model.spec.mjs': 'model function, no page',
  'written-page.spec.mjs': 'sizes set by test blocks',
  'written-reader.spec.mjs': 'reader answers and page notes',
};
export const wideOnlyFiles = Object.keys(WIDE_ONLY);

// A wide-only file whose code names a size, a position or a visibility matcher (`viewport:`, `setViewportSize`, `viewportSize`, `boundingBox`,
// `getBoundingClientRect`, `scrollWidth`, `innerWidth`, `toHaveScreenshot`, `screenshot(`, `isVisible`, `toBeVisible`,
// `toBeHidden`, `isHidden`, `toBeInViewport`, `scrollIntoViewIfNeeded`, `matchMedia`) is listed here with the reason its
// claims do not depend on the viewport the project gives. budget.spec.mjs fails a wide-only file that names one and is
// not listed, and a listed file that names none.
export const SIZE_EXCEPTIONS = {
  'autopilot.spec.mjs': 'sets both gating sizes itself',
  'daemon-decide.spec.mjs': 'phone test sets size',
  'daemon-live.spec.mjs': 'sets the gating sizes itself',
  'daemon-run-tree-security.spec.mjs': 'presence, not layout',
  'daemon-run-tree.spec.mjs': 'presence; screenshot is a file',
  'daemon-start.spec.mjs': 'presence, not layout',
  'daemon-ui.spec.mjs': 'presence, not layout',
  'good-end.spec.mjs': 'presence, not layout',
  'graph-any-runbook.spec.mjs': 'presence, not layout',
  'loom-run-record.spec.mjs': 'presence, not layout',
  'markdown-hostile.spec.mjs': 'presence, not layout',
  'mobile.spec.mjs': 'sets the phone size itself',
  'read-only.spec.mjs': 'screenshot is a file',
  'run-picker-experiments.spec.mjs': 'presence, not layout',
  'runner-live.spec.mjs': 'sets the size itself',
  'run-picker.spec.mjs': 'sets 805x1270 and 390x844 itself',
  'security-products-page.spec.mjs': 'presence, not layout',
  'security-start-answers.spec.mjs': 'presence, not layout',
  'security-start-close.spec.mjs': 'presence, not layout',
  'security-start-idempotency.spec.mjs': 'presence, not layout',
  'security-start-key.spec.mjs': 'presence, not layout',
  'security-start-late-answers.spec.mjs': 'presence, not layout',
  'security-start-ready-window.spec.mjs': 'presence, not layout',
  'security-start-token-places.spec.mjs': 'presence, not layout',
  'security-start-token.spec.mjs': 'presence, not layout',
  'step-products-bounds.spec.mjs': 'presence, not layout',
  'step-products-page.spec.mjs': 'sets 390 and 1366 itself',
  'stopped.spec.mjs': 'sets both gating sizes itself',
  'stub-runs.spec.mjs': 'sets 805x1270 and 390x844 itself',
  'view.spec.mjs': 'presence, not layout',
  'written-page.spec.mjs': 'sets its sizes itself',
  'written-reader.spec.mjs': 'presence, not layout',
};

// The one form of a spec file that the lists above know: `<this directory>/<name>.spec.mjs`, directly under test/ (the pattern
// starts at this directory and Playwright matches it against the absolute path, so a spec below a subdirectory, also one
// below a directory that is itself named `test`, is not a test). The Playwright config matches test files by it, and
// budget.spec.mjs fails a file anywhere below test/ (but fixtures/ and recorded/) that Playwright's default match would take and this form does not, so a
// misnamed or misplaced spec is told, not left out.
const TEST_DIR = path.dirname(fileURLToPath(import.meta.url));
export const SPEC_FORM = new RegExp(`^${TEST_DIR.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/[^/]+\\.spec\\.mjs$`);

// The sleeps of the gate: the files that wait a fixed time, with how many sleeps each has (`calls`) and the sum of their
// literal milliseconds (`ms`). A sleep is a `waitForTimeout` (also with a space before the bracket, as a bracket member or
// as a reference), a `setTimeout` that a `new Promise` with an arrow function waits for, or a window `Date.now() + <number>`
// (the entry says `window`, with a reason of one to five words), in a `*.spec.mjs` of the gate and in every file they
// import by `from '<path>'` or `import('<path>')` with a relative path (`./x`, `../x`, in `ux/` too; an `import '<path>'` with no bindings is not followed; a name starting `../` here is relative to `test/`); it is counted per
// call site, once however often a loop runs it. A `waitForFunction` or `expect.poll` timeout or interval, a timer that ends a
// child or a request, a `function` callback of a promise, `timers/promises`, `Atomics.wait`, an alias of `setTimeout`,
// `setInterval` and a window whose time is no number are not found. A sleep whose time is the argument of a helper makes the
// entry say `variable`, with a reason of one to five words, the `helper` and the `largest` number its call sites in the file
// pass. `why` is a reason of one to five words that any entry may carry. budget.spec.mjs fails a count or a sum that differs
// from its entry, a call site over `largest` and one that passes no number, so a new or a longer sleep is a line here, in a
// change a reviewer reads.
export const FIXED_WAITS = {
  '../ux/core/contract.mjs': {calls: 1, ms: 750, why: 'upper bound, ends on event'},
  '../ux/core/phone.mjs': {calls: 1, ms: 60, why: 'lets the scroll settle'},
  'autopilot.spec.mjs': {calls: 1, ms: 100},
  'child.spec.mjs': {calls: 2, ms: 1025},
  'daemon-decide-review.spec.mjs': {calls: 2, ms: 5300},
  'daemon-decide.spec.mjs': {calls: 2, ms: 3000},
  'daemon-routes.spec.mjs': {calls: 1, ms: 5000},
  'daemon-start.spec.mjs': {calls: 1, ms: 300},
  'event-log-lines.spec.mjs': {calls: 1, ms: 2500},
  'fixture-hold.spec.mjs': {calls: 1, ms: 20000},
  'idle-picker-window.spec.mjs': {calls: 1, ms: 0, variable: 'pause of a polling loop', helper: 'sleep', largest: 300},
  'idle-reads.spec.mjs': {calls: 3, ms: 525},
  'idle-server.spec.mjs': {calls: 2, ms: 1, variable: 'pause of a polling loop', helper: 'sleep', largest: 3000},
  'idle-slow-page.spec.mjs': {calls: 1, ms: 0, variable: 'pause of a polling loop', helper: 'sleep', largest: 10000},
  'paths-served.spec.mjs': {calls: 1, ms: 700},
  'runner-live-guards.spec.mjs': {calls: 2, ms: 35000, window: 'bounded writer in a child'},
  'runner-live.spec.mjs': {calls: 2, ms: 50, why: 'polls a pipe, a process'},
  'security-products-3-reads.spec.mjs': {calls: 1, ms: 100},
  'security-start-content.spec.mjs': {calls: 2, ms: 800},
  'security-start-floor-reread.spec.mjs': {calls: 1, ms: 0, variable: 'retry pause of a loop', helper: 'later', largest: 250},
  'security-start-gone-runbook.spec.mjs': {calls: 1, ms: 300},
  'security-start-idempotency.spec.mjs': {calls: 1, ms: 1500},
  'security-start-key.spec.mjs': {calls: 5, ms: 3600},
  'security-start-late-answers.spec.mjs': {calls: 1, ms: 300},
  'security-start-ready-window.spec.mjs': {calls: 9, ms: 2400, variable: 'retry pause of a loop', helper: 'later', largest: 250},
  'security-start-second-paint.spec.mjs': {calls: 3, ms: 800},
  'security-start-token-places.spec.mjs': {calls: 2, ms: 3700},
  'security-start-token.spec.mjs': {calls: 2, ms: 800},
  'serve.mjs': {calls: 1, ms: 50},
};
