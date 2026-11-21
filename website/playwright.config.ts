import {defineConfig} from '@playwright/test';

const sitePort = Number(process.env.SDC_DOCS_SITE_PORT ?? 7591);
const fixturePort = Number(process.env.SDC_DOCS_FIXTURE_PORT ?? 7592);
const site = process.env.SDC_SITE_URL ?? `http://127.0.0.1:${sitePort}/sdc/`;
const fixture = process.env.SDC_FIXTURE_URL ?? `http://127.0.0.1:${fixturePort}/sdc/`;
const serve = (dir: string, port: number) =>
  `node node_modules/@docusaurus/core/bin/docusaurus.mjs serve --dir ${dir} --port ${port} --host 127.0.0.1 --no-open`;

export default defineConfig({
  testDir: 'tests',
  testMatch: '*.spec.ts',
  outputDir: 'test-results',
  timeout: 60_000,
  reporter: [['list']],
  projects: [
    {name: 'site', testMatch: ['smoke.spec.ts', 'pitch.spec.ts'], use: {baseURL: site}},
    {name: 'context', testMatch: 'context.spec.ts', use: {baseURL: site}},
    {name: 'fixture', testMatch: 'fixture.spec.ts', use: {baseURL: fixture}},
  ],
  webServer: process.env.SDC_SITE_URL
    ? undefined
    : [
        {command: serve('build', sitePort), url: site, reuseExistingServer: false, gracefulShutdown: {signal: 'SIGTERM', timeout: 5_000}},
        {command: serve('build-fixture', fixturePort), url: fixture, reuseExistingServer: false, gracefulShutdown: {signal: 'SIGTERM', timeout: 5_000}},
      ],
});
