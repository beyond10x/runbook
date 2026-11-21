#!/usr/bin/env node
// Screenshot one page of a running inspector, for looking at a change while building it.
//   node test/peek.mjs <url> <out.png> [width] [height] [click-selector...]
import {chromium} from 'playwright';

const [url, out, w = '1600', h = '1000', ...clicks] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({viewport: {width: Number(w), height: Number(h)}, colorScheme: 'dark'});
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
await page.goto(url);
await page.waitForSelector('#now .now-h', {timeout: 10000}).catch(() => {});
await page.waitForTimeout(1200);
for (const c of clicks) { await page.click(c); await page.waitForTimeout(500); }
await page.screenshot({path: out});
if (errors.length) console.log(errors.join('\n'));
await browser.close();
