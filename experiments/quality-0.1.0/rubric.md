# Quality rubric, idea "shared offline to-do list" (fixed before any product was opened)

## The idea (exact text, `idea.yaml` key `text`, identical in all nine runs)

> A to-do list web app for me and my partner that also works on the train without a connection

Readings taken from these words, and nothing else:

- "to-do list": tasks that can be created, changed, ticked done and removed, in a list.
- "web app": a page a browser can use (forms and buttons), not only an API.
- "for me and my partner": two people, two browsers, one shared list.
- "works on the train without a connection": the page keeps working with no network; what is done
  without a connection is kept and reaches the other person once the connection is back.

## Scoring rule

- Every item is scored `pass`, `fail` or `not testable`, with the step and what came back.
- One point per `pass`. Score = passes of n items (n = 23). `not testable` is reported separately and
  never counts as pass; it is used only when the harness cannot observe the item although the
  product offers it (harness limitation), never for a missing feature.
- A product that does not build or start inside the confinement fails every item that needs it
  running.
- A missing feature is `fail` (no UI: every UI item fails, HTTP is tested where it exists).
- Same harness, same timings, same inputs for every product. Timings: "live" = visible in the other
  context within 10 s without reload; "after reconnect" = up to 15 s for sync, then reload the other
  context.
- Where a step needs a product-specific selector or route, it is found from the product's own page
  and README, and recorded in the evidence.

## Items

Build and start

- B1 builds: from a clean copy, the README's (or Taskfile's) build command exits 0 inside the
  confinement. Evidence: command, exit code.
- B2 tests: the tree's own test command runs and every test passes. Evidence: passed/failed count.
- B3 starts: the README's start command starts a server answering HTTP on 127.0.0.1:<free port>
  within 15 s. Evidence: command, first response status.

Web UI and to-do functions (one browser context A, Chromium)

- U1 browser page: GET / (or the README's address) returns an HTML page with a visible control to
  add a task. Evidence: status, content type, control found.
- F1 list: the user can create a list, or the first page gives a usable list without setup (the
  idea names "a to-do list"; one shared list satisfies it). Evidence: which of the two, step.
- F2 add: typing a title and submitting shows the task in the page. Evidence: text shown.
- F3 edit: the title of an existing task can be changed through the page; the new title is shown
  after reload. Evidence: step, text after reload.
- F4 done: a task can be ticked done through the page; the done state is shown after reload.
- F5 remove: a task can be removed through the page; it is gone after reload.
- P1 restart: stop the server, start it again with the same README command and data location;
  the tasks are still there.

Sharing (contexts A and B, separate cookie and storage jars)

- S1 same list: B opens the address or share link A has; B sees A's tasks.
- S2 after reload: a task added in A appears in B after B reloads; a task added in B appears in A
  after A reloads.
- S3 live: a task added in A appears in B within 10 s without reloading B.

Offline (the train)

- O1 browser offline: A is set offline (`context.setOffline(true)`); the already loaded page still
  shows the list after 5 s (no error page, tasks visible).
- O2 server stopped: with the server process stopped, the already loaded page in A still shows the
  list after 5 s.
- O3 reopen offline: with A offline, reloading the page shows the app and the tasks (the page is
  available offline, e.g. by a service worker).
- O4 add offline kept: with A offline, a task added in the page is shown and is still shown after
  the context goes online again.
- O5 offline reaches partner: after A reconnects, the task added offline reaches the server and B
  sees it after reload (within 15 s of reconnect).
- O6 tick offline reaches partner: a task ticked done while A is offline is shown done in B after
  A reconnects and B reloads.
- O7 conflicting offline edits: A and B both offline edit the same task's title differently, then
  reconnect one after the other; both contexts, after reload, show the same state, the task exists
  once, and no other task is lost.

Input robustness

- R1 empty title: submitting an empty or whitespace title creates no blank task and the page keeps
  working.
- R2 long title: a 10 000-character title neither crashes the server nor breaks the page (server
  still answers, page still lists tasks).
- R3 markup: a title `<img src=x onerror="window.__xss=1">` is shown as text and does not execute
  (`window.__xss` stays undefined in both contexts).
