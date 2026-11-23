// The checks of budget.spec.mjs, each a function of its inputs (a map, the text of files), so that the spec can call each
// on a good input and a bad one of its own and not only on the live lists. A check returns the list of what it found.

import path from 'node:path';

export const SIZE = /setViewportSize|viewport\s*:|viewportSize|boundingBox|getBoundingClientRect|scrollWidth|innerWidth|toHaveScreenshot|\.screenshot\(|\.isVisible\(|toBeVisible|toBeHidden|isHidden|toBeInViewport|scrollIntoViewIfNeeded|matchMedia/;

// The skips on the project name that bothSkipProblems knows.
export const SKIP_SPELLINGS = ["project.name !== 'wide'", "project.name != 'wide'", "project.name === 'laptop'", "project.name == 'laptop'"];
const PROJECT_SKIP = /project\.name\s*!==?\s*['"]wide['"]|project\.name\s*===?\s*['"]laptop['"]/;

// Playwright's default match for a test file, and the one form the lists of lanes.mjs know (`SPEC_FORM`).
const PLAYWRIGHT_DEFAULT = /\.(spec|test)\.[cm]?[jt]sx?$/;

export const stripComments = (src) => src.split('\n').filter((l) => !/^\s*\/\//.test(l)).join('\n').replace(/\s\/\/ .*$/gm, '');
export const code = (src) => stripComments(src.split('\n').filter((l) => !/test\.skip\(/.test(l)).join('\n'));

export const reasonProblems = (maps) => {
  const words = (why) => why.trim().split(/\s+/).filter(Boolean).length;
  const bad = [];
  for (const [list, map] of Object.entries(maps)) {
    for (const [name, why] of Object.entries(map)) {
      if (typeof why !== 'string') bad.push(`${list} ${name}: a reason that is no text`);
      else if (words(why) < 1 || words(why) > 5) bad.push(`${list} ${name}: "${why}" (${words(why)} words)`);
    }
  }
  return bad;
};

export const listProblems = (specs, lists) => {
  const bad = [];
  for (const f of specs) {
    const in_ = Object.entries(lists).filter(([, l]) => l.includes(f)).map(([k]) => k);
    if (in_.length !== 1) bad.push(`${f}: in ${in_.length ? in_.join(' and ') : 'no list'}`);
  }
  for (const [k, l] of Object.entries(lists)) for (const f of l) if (!specs.includes(f)) bad.push(`${f}: listed in ${k}, not a spec file under test/`);
  return bad;
};

export const bothSkipProblems = (files) => {
  const bad = [];
  for (const [f, src] of Object.entries(files)) {
    if (src.split('\n').some((l) => !/^\s*\/\//.test(l) && PROJECT_SKIP.test(l))) {
      bad.push(`${f}: listed for both viewports and skips on the project name (a skip it knows: ${SKIP_SPELLINGS.join(', ')}; any other spelling is a skip too and is not seen)`);
    }
  }
  return bad;
};

export const sizeProblems = (texts, wide, exceptions) => {
  const bad = [];
  for (const [f, src] of Object.entries(texts)) if (SIZE.test(code(src)) && !(f in exceptions)) bad.push(`${f}: names a size, a position or a visibility matcher and runs in wide only`);
  for (const f of Object.keys(exceptions)) {
    if (!wide.includes(f)) bad.push(`${f}: an exception for a file that is not wide only`);
    else if (!(f in texts)) bad.push(`${f}: an exception for a file the check does not read`);
    else if (!SIZE.test(code(texts[f]))) bad.push(`${f}: an exception for a file that names no size, position or visibility matcher`);
  }
  return bad;
};

export const sizePatternProblems = (re, named, none) => {
  const bad = [];
  for (const [what, lines] of Object.entries(named)) for (const l of lines) if (!re.test(l)) bad.push(`${what}: ${l} is not matched`);
  for (const l of none) if (re.test(l)) bad.push(`${l} is matched and names no size`);
  return bad;
};

// The text between the parenthesis that opens at `open` and its mate; the rest of the text when it has no mate.
const inside = (s, open) => {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    if (s[i] === '(') depth++;
    else if (s[i] === ')' && --depth === 0) return s.slice(open + 1, i);
  }
  return s.slice(open + 1);
};
const topLevel = (s) => {
  const parts = [];
  let depth = 0, from = 0;
  for (let i = 0; i < s.length; i++) {
    if ('([{'.includes(s[i])) depth++;
    else if (')]}'.includes(s[i])) depth--;
    else if (s[i] === ',' && depth === 0) { parts.push(s.slice(from, i)); from = i + 1; }
  }
  parts.push(s.slice(from));
  return parts;
};
const literal = (s) => (/^\s*\d[\d_]*\s*$/.test(s ?? '') ? Number(s.replace(/_/g, '')) : null);

// The sleeps of a file: every `waitForTimeout` (a call with or without a space before the bracket, a bracket member, a
// reference), every `setTimeout` that a `new Promise` with an arrow function waits for (the callback is the promise's
// resolver or calls it) and every window `Date.now() + <number>`.
// Each is {ms} (its literal time in milliseconds, or null when the time is no literal), and {ms, window: true} for a window.
// Counted per call site: how often a loop runs a call, a `waitForFunction` or `expect.poll` timeout or interval and a timer
// that ends a child or a request are not sleeps here. Not found: a `function` callback of the promise, `timers/promises`,
// `Atomics.wait`, an alias of `setTimeout`, a resolver passed on by `bind`, `setInterval` and a window whose time is no number.
export const sleeps = (src) => {
  const t = stripComments(src);
  const found = [];
  for (const m of t.matchAll(/waitForTimeout/g)) {
    const call = /^['"`]?\s*\]?\s*\(\s*(\d[\d_]*)\s*\)/.exec(t.slice(m.index + m[0].length));
    found.push({ms: call ? literal(call[1]) : null});
  }
  for (const m of t.matchAll(/Date\.now\(\)\s*\+\s*(\d[\d_]*)/g)) found.push({ms: literal(m[1]), window: true});
  const seen = new Set();
  for (const p of t.matchAll(/new\s+Promise\s*\(/g)) {
    const open = p.index + p[0].length - 1;
    const body = inside(t, open);
    const resolver = /^\s*(?:async\s*)?\(?\s*(\w+)/.exec(body)?.[1];
    if (!resolver) continue;
    for (const s of body.matchAll(/setTimeout\s*\(/g)) {
      const at = open + 1 + s.index;
      if (seen.has(at)) continue;
      const [callback, ms] = topLevel(inside(t, at + s[0].length - 1));
      if (callback.trim() === resolver || new RegExp(`\\b${resolver}\\s*\\(`).test(callback)) {
        seen.add(at);
        found.push({ms: literal(ms)});
      }
    }
  }
  return found;
};

// The arguments of the calls `name(...)` in a text, as text each (the definition `const name = (...)` is no call).
const callArguments = (src, name) => {
  const t = stripComments(src);
  return [...t.matchAll(new RegExp(`(?<![\\w$.])(?<!function\\s)${name}\\s*\\(`, 'g'))].map((m) => inside(t, m.index + m[0].length - 1));
};

// `entries`: {file: {calls, ms, variable?, helper?, largest?, window?}}; `files`: {file: text} of the gate's files. `variable`
// (a reason) says a sleep takes its time from the argument of a helper: `helper` is the helper's name and `largest` the
// largest number its call sites in the file pass. `window` (a reason) says the file has a `Date.now() + <number>` window.
export const sleepProblems = (files, entries) => {
  const bad = [];
  const hint = 'wait for an event (settled, expect, expect.poll) or name the file in FIXED_WAITS; counted: waitForTimeout in any spelling, a setTimeout inside new Promise and a Date.now() + number window';
  for (const [f, src] of Object.entries(files)) {
    const s = sleeps(src);
    const entry = entries[f];
    const calls = s.length;
    const ms = s.reduce((a, x) => a + (x.ms ?? 0), 0);
    const variable = s.filter((x) => x.ms === null).length;
    const windows = s.filter((x) => x.window).length;
    if (!entry) {
      if (calls) bad.push(`${f}: ${calls} sleeps, none allowed: ${hint}`);
      continue;
    }
    if (calls > entry.calls) bad.push(`${f}: ${calls} sleeps, ${entry.calls} allowed: ${hint}`);
    else if (calls < entry.calls) bad.push(`${f}: ${entry.calls} sleeps allowed, ${calls} made: lower the entry in FIXED_WAITS`);
    else if (ms !== entry.ms) bad.push(`${f}: the literal sleeps add up to ${ms} ms, the entry says ${entry.ms}: a longer sleep is a change a reviewer reads`);
    if (variable && !entry.variable) bad.push(`${f}: ${variable} sleeps whose time is no number: the entry needs "variable" with a reason of one to five words`);
    if (!variable && entry.variable) bad.push(`${f}: the entry says "variable", every sleep has a number: drop it`);
    if (windows && !entry.window) bad.push(`${f}: ${windows} Date.now() + number windows: the entry needs "window" with a reason of one to five words`);
    if (!windows && entry.window) bad.push(`${f}: the entry says "window", no Date.now() + number window is made: drop it`);
    if (variable && entry.variable) {
      if (!/^\w+$/.test(entry.helper ?? '') || !Number.isInteger(entry.largest)) bad.push(`${f}: "variable" needs "helper" (the name of the helper) and "largest" (the largest number its call sites pass)`);
      else {
        const args = callArguments(src, entry.helper).map((a) => ({a: a.trim(), n: literal(a)}));
        if (!args.length) bad.push(`${f}: the helper ${entry.helper} has no call site in the file: drop "variable"`);
        for (const x of args.filter((y) => y.n === null)) bad.push(`${f}: ${entry.helper}(${x.a}) passes no number`);
        const top = Math.max(...args.filter((y) => y.n !== null).map((y) => y.n), 0);
        if (top > entry.largest) bad.push(`${f}: ${entry.helper}(${top}) is over the largest the entry names, ${entry.largest}: a longer pause is a change a reviewer reads`);
        else if (args.length && top < entry.largest) bad.push(`${f}: the call sites of ${entry.helper} pass at most ${top}, the entry says ${entry.largest}: lower it`);
      }
    }
  }
  for (const f of Object.keys(entries)) if (!(f in files)) bad.push(`${f}: in FIXED_WAITS, not a file of the gate lane`);
  return bad;
};

// The files a set of roots reaches by relative imports (`from './x.mjs'`, `from '../x.mjs'`, `import('./x.mjs')`, in any
// directory); each name is relative to the directory of the roots, so a file one level up is `../x.mjs`. `read` gives a
// file's text, or undefined for a file that is not there. A file is read once, so a cycle ends; more than `limit` files read
// is an error.
export const closure = (roots, read, limit = 500) => {
  const out = new Set();
  const todo = [...roots];
  let reads = 0;
  while (todo.length) {
    const f = todo.pop();
    if (out.has(f)) continue;
    if (++reads > limit) throw new Error(`the closure reads more than ${limit} files (at ${f}): it does not end`);
    const src = read(f);
    if (src === undefined) continue;
    out.add(f);
    for (const m of stripComments(src).matchAll(/(?:from\s+|import\s*\(\s*)['"](\.{1,2}\/[^'"]+)['"]/g)) todo.push(path.posix.join(path.posix.dirname(f), m[1]));
  }
  return [...out].sort();
};

// A file that Playwright's default match takes for a test and the lists' form does not: it would run in no list, or not.
export const strayProblems = (files, formMatches) => files.filter((f) => PLAYWRIGHT_DEFAULT.test(f) && !formMatches(f)).map((f) => `${f}: matches Playwright's default test match and not the one form the lists know (test/<name>.spec.mjs)`);
