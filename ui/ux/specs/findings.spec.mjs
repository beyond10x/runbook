// Every finding record is complete, and no S1 or S2 closes without a regression asset that exists.
import {test, expect} from '@playwright/test';
import path from 'node:path';
import {UI, viewports} from '../adapter.mjs';
import {problems} from '../core/findings.mjs';

test('finding records: complete, closed S1/S2 with a regression asset', async ({}, info) => {
  test.skip(info.project.name !== viewports.gating[0].name, 'runs once');
  expect(problems(path.join(UI, 'ux/findings'), UI)).toEqual([]);
});
