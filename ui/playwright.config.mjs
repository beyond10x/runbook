import {defineConfig} from '@playwright/test';
import {slowFiles, wideOnlyFiles} from './test/lanes.mjs';

export const BUDGET = {testMs: 60000, runMs: 120000};
export const VIEWPORTS = {wide: {width: 1600, height: 1000}, laptop: {width: 1280, height: 800}};

const glob = (files) => files.map((f) => `**/${f}`);

export default defineConfig({
  testDir: 'test',
  testIgnore: glob(slowFiles),
  outputDir: process.env.PW_OUT || 'node_modules/.pw-results',
  timeout: BUDGET.testMs,
  globalTimeout: BUDGET.runMs,
  reporter: [['line'], ['./test/slowest-reporter.mjs', {budgetMs: BUDGET.runMs}]],
  use: {baseURL: process.env.SDC_UI_URL, colorScheme: 'dark'},
  projects: [
    {name: 'laptop', use: {viewport: VIEWPORTS.laptop}, testIgnore: glob([...wideOnlyFiles, ...slowFiles])},
    {name: 'wide', use: {viewport: VIEWPORTS.wide}},
  ],
});
