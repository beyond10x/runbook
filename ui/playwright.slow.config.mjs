import {defineConfig} from '@playwright/test';
import gate, {VIEWPORTS} from './playwright.config.mjs';
import {slowFiles} from './test/lanes.mjs';

process.env.SDC_UI_LANE = 'slow';

export default defineConfig({
  ...gate,
  testIgnore: undefined,
  testMatch: slowFiles.map((f) => `**/${f}`),
  timeout: 180000,
  globalTimeout: 0,
  reporter: [['line'], ['./test/slowest-reporter.mjs']],
  projects: [
    {name: 'wide', use: {viewport: VIEWPORTS.wide}},
    {name: 'laptop', use: {viewport: VIEWPORTS.laptop}},
  ],
});
