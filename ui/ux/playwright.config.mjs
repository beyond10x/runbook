// The fast loop (L0 to L3) at the gating viewports; with UX_ROUND=R-nn also the measured-only ones,
// and screenshots into ux/rounds/R-nn/shots. Hermetic: fixture runs played into ux/out, no keys.
import {defineConfig} from '@playwright/test';
import {viewports} from './adapter.mjs';

const vps = [...viewports.gating, ...(process.env.UX_ROUND ? viewports.measured : [])];
export default defineConfig({
  testDir: 'specs',
  outputDir: process.env.PW_OUT ? `${process.env.PW_OUT}/ux` : '../node_modules/.pw-results/ux',
  reporter: 'line',
  timeout: 180000,
  globalSetup: './core/setup.mjs',
  fullyParallel: false,
  use: {colorScheme: 'dark', reducedMotion: 'no-preference'},
  projects: vps.map((v) => ({name: v.name, use: {viewport: {width: v.width, height: v.height}}})),
});
