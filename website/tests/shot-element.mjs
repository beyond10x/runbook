// One element's screenshot: node tests/shot-element.mjs <url> <selector> <file> [width] [theme] [click selector]
import {chromium} from '@playwright/test';

const [url, selector, file, width = '1440', theme = 'light', click] = process.argv.slice(2);
const browser = await chromium.launch();
const context = await browser.newContext({viewport: {width: Number(width), height: 900}, colorScheme: theme});
await context.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
const page = await context.newPage();
await page.goto(url, {waitUntil: 'networkidle'});
if (click) await page.locator(click).first().click();
await page.locator(selector).first().screenshot({path: file});
await browser.close();
