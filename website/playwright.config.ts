import {defineConfig} from '@playwright/test';

const site = process.env.SDC_SITE_URL ?? 'http://127.0.0.1:7495/sdc/';
const fixture = process.env.SDC_FIXTURE_URL ?? 'http://127.0.0.1:7496/sdc/';

export default defineConfig({
  testDir: 'tests',
  testMatch: '*.spec.ts',
  outputDir: 'test-results',
  timeout: 60_000,
  reporter: [['list']],
  projects: [
    {name: 'site', testMatch: 'smoke.spec.ts', use: {baseURL: site}},
    {name: 'fixture', testMatch: 'fixture.spec.ts', use: {baseURL: fixture}},
  ],
  webServer: process.env.SDC_SITE_URL
    ? undefined
    : [
        {command: 'node node_modules/@docusaurus/core/bin/docusaurus.mjs serve --dir build --port 7495 --host 127.0.0.1 --no-open', url: site, reuseExistingServer: true},
        {command: 'node node_modules/@docusaurus/core/bin/docusaurus.mjs serve --dir build-fixture --port 7496 --host 127.0.0.1 --no-open', url: fixture, reuseExistingServer: true},
      ],
});
