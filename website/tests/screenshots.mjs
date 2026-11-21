// Screenshots of the review pages: node tests/screenshots.mjs <base URL> <output dir>
import {chromium} from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

const base = process.argv[2] ?? 'http://127.0.0.1:7493/runbook/';
const out = process.argv[3] ?? 'screenshots';
fs.mkdirSync(out, {recursive: true});
const pages = [
  ['landing', ''],
  ['flow', 'docs/flow'],
  ['layers', 'docs/built-on'],
  ['protocol-sdc', 'protocols/sdc'],
  ['protocol-sdc-story', 'protocols/sdc-story'],
  ['evaluation', 'evaluation'],
  ['llm', 'llm-is-not-the-runtime'],
];
const modes = [
  ['1440-light', {width: 1440, height: 900}, 'light'],
  ['390-dark', {width: 390, height: 844}, 'dark'],
];
const browser = await chromium.launch();
for (const [mode, viewport, theme] of modes) {
  const context = await browser.newContext({viewport, colorScheme: theme});
  await context.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
  const page = await context.newPage();
  for (const [name, route] of pages) {
    await page.goto(base + route, {waitUntil: 'networkidle'});
    await page.screenshot({path: path.join(out, `${name}-${mode}.png`), fullPage: true});
  }
  await context.close();
}
await browser.close();
console.log(`screenshots in ${out}`);
