import {defineConfig} from '@playwright/test';

export default defineConfig({
  testDir: 'test',
  outputDir: process.env.PW_OUT || 'node_modules/.pw-results',
  reporter: 'line',
  use: {baseURL: process.env.SDC_UI_URL || 'http://127.0.0.1:7490', colorScheme: 'dark'},
  projects: [
    {name: 'wide', use: {viewport: {width: 1600, height: 1000}}},
    {name: 'laptop', use: {viewport: {width: 1280, height: 800}}},
  ],
});
