// Screenshots of the landing pitch slider: node tests/shot-pitch.mjs <url> <out dir>
// One per slide at 1440 px light, and the section at 390 px dark.
import {chromium} from '@playwright/test';
import path from 'node:path';

const [url, out] = process.argv.slice(2);
const browser = await chromium.launch();

async function context(width, theme) {
  const ctx = await browser.newContext({viewport: {width, height: 900}, colorScheme: theme, reducedMotion: width > 400 ? 'reduce' : 'no-preference'});
  await ctx.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
  const page = await ctx.newPage();
  await page.goto(url, {waitUntil: 'networkidle'});
  return page;
}

const wide = await context(1440, 'light');
for (let i = 0; i < 3; i++) {
  await wide.locator(`[data-pitch-dot="${i}"]`).click();
  await wide.mouse.move(0, 0);
  await wide.evaluate(() => document.activeElement?.blur());
  await wide.locator('[data-pitch]').screenshot({path: path.join(out, `pitch-slide-${i + 1}-1440-light.png`)});
}
const narrow = await context(390, 'dark');
for (let i = 0; i < 3; i++) {
  await narrow.locator(`[data-pitch-dot="${i}"]`).click();
  await narrow.waitForTimeout(600);
  await narrow.locator('[data-pitch]').screenshot({path: path.join(out, `pitch-slide-${i + 1}-390-dark.png`)});
}
await browser.close();
