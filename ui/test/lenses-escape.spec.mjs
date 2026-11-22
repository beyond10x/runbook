// A string the view supplies (a stage name, a phase id) reaches the page through renderPhases: it is
// escaped in every attribute and text it is written into, so a value that closes the attribute and opens
// a tag shows as text and starts nothing. No browser.
import {test, expect} from '@playwright/test';
import {renderPhases} from '../web/lenses.js';
import {setView} from '../web/view.js';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'a model check runs once'));
test.afterEach(() => setView(null));

const EVIL = '"><img src=x onerror=alert(1)>';

test('a stage name and a phase id from the view are escaped in the page markup', () => {
  const problems = setView({format: 'runbook.view/1', runbook: 'x',
    phases: {stages: [`S${EVIL}`], items: [{id: `p${EVIL}`, stage: `S${EVIL}`, label: 'Phase', protocol: 'x.y', claims: ['x.claim']}]}});
  expect(problems).toEqual([]);
  const html = renderPhases([]);
  expect(html).not.toContain('<img');
  expect(html).toContain('data-phase="p&quot;&gt;&lt;img src=x onerror=alert(1)&gt;"');
  expect(html).toContain('data-key="st:S&quot;&gt;&lt;img src=x onerror=alert(1)&gt;"');
});
