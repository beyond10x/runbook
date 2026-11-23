// Confirmation pass of the independent review, findings 3 and 4 of story inspector-shows-step-products, with inputs the
// first cases did not use. web/markdown.js documents "a document makes at most MAX_NODES (30000) elements; past it the
// rest is plain text". The list loop (list()) makes an li per item without asking over(), so a list of empty items does
// not stop at the bound. Each case runs in its own page, in Chromium, with the renderer the page loads. Invented text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun} from './products-fixture.mjs';
import {fixtureRoot} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-2-markdown');
let srv, base;
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  writeProductsRun(path.join(root, 'runs', 'r'));
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
});

async function render(page, build) {
  await page.goto(base);
  return page.evaluate(async ({src}) => {
    const {renderMarkdown} = await import('/markdown.js');
    // eslint-disable-next-line no-new-func
    const md = new Function(`return (${src})()`)();
    const host = document.createElement('div');
    const t0 = performance.now();
    let error = null;
    try { renderMarkdown(md, {dir: 'work', open: () => {}}, host); } catch (e) { error = String(e); }
    return {ms: Math.round(performance.now() - t0), error, nodes: host.querySelectorAll('*').length, chars: md.length};
  }, {src: build.toString()});
}

test('finding 3 again: a list of 65000 empty items makes at most the bound the renderer states (30000 elements, with the one note)', async ({page}) => {
  test.setTimeout(30000);
  const r = await render(page, () => '-\n'.repeat(65000));
  console.log(`empty items: ${r.chars} characters made ${r.nodes} nodes in ${r.ms} ms`);
  expect(r.error).toBeNull();
  expect(r.nodes, `${r.chars} characters made ${r.nodes} nodes`).toBeLessThanOrEqual(61000);
});

test('finding 3 again: a list of 32000 one-letter items makes at most 61000 nodes', async ({page}) => {
  test.setTimeout(30000);
  const r = await render(page, () => '- a\n'.repeat(32000));
  console.log(`one-letter items: ${r.chars} characters made ${r.nodes} nodes in ${r.ms} ms`);
  expect(r.nodes, `${r.chars} characters made ${r.nodes} nodes`).toBeLessThanOrEqual(61000);
});

const shapes = {
  nestedLists: () => Array.from({length: 4000}, (_, i) => `${'  '.repeat(i % 16)}- item ${i}`).join('\n'),
  nestedQuotes: () => Array.from({length: 4000}, (_, i) => `${'> '.repeat(1 + (i % 20))}line ${i}`).join('\n'),
  quoteOfList: () => '> - a\n'.repeat(20000),
  listOfQuotes: () => '- > a\n'.repeat(20000),
  tableThenList: () => `|${'a|'.repeat(50)}\n|${'-|'.repeat(50)}\n${`|${'x|'.repeat(50)}\n`.repeat(400)}\n${'- y\n'.repeat(8000)}`,
  pipesParagraph: () => 'a | b\n'.repeat(20000),
  separatorShape: () => `a|b\n${'-|'.repeat(65000)}`,
  headingClosers: () => `# a ${'# '.repeat(65000)}`,
  emptyHeadings: () => '#\n'.repeat(65000),
  hardBreaks: () => 'a  \n'.repeat(32000),
};
for (const [name, build] of Object.entries(shapes)) {
  test(`${name} is rendered in under 5 s and within twice the stated node bound`, async ({page}) => {
    test.setTimeout(30000);
    const r = await render(page, build);
    console.log(`${name}: ${r.chars} characters, ${r.nodes} nodes, ${r.ms} ms`);
    expect(r.error).toBeNull();
    expect(r.ms, `${r.ms} ms`).toBeLessThan(5000);
    expect(r.nodes, `${r.chars} characters made ${r.nodes} nodes`).toBeLessThanOrEqual(61000);
  });
}
