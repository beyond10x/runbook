import {defineConfig} from '@playwright/test';
import {slowFiles} from './test/lanes.mjs';

export const BUDGET = {testMs: 60000, runMs: 120000};

export default defineConfig({
  testDir: 'test',
  testIgnore: slowFiles.map((f) => `**/${f}`),
  outputDir: process.env.PW_OUT || 'node_modules/.pw-results',
  timeout: BUDGET.testMs,
  globalTimeout: BUDGET.runMs,
  reporter: [['line'], ['./test/slowest-reporter.mjs', {budgetMs: BUDGET.runMs}]],
  use: {baseURL: process.env.SDC_UI_URL, colorScheme: 'dark'},
  projects: [
    {name: 'wide', use: {viewport: {width: 1600, height: 1000}}},
    {name: 'laptop', use: {viewport: {width: 1280, height: 800}}},
  ],
});
