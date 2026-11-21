// Per-product locators, found from each product's own page and README. Every product goes through
// the same steps (rubric.ts); only how a control is reached differs. A missing control is `null`.
import type {Page, Locator, BrowserContext} from 'playwright';
import type {Running} from './products.ts';

export interface Adapter {
  listKind: 'implicit' | 'explicit';
  // the address the first person opens; for explicit lists, created in the page (F1)
  address(r: Running, page: Page): Promise<string>;
  // gates after each page load (a name, a sign-in); `who` is 'A', 'B' or 'C'
  gate(page: Page, who: string): Promise<void>;
  addInput: string;
  add(page: Page, title: string, who: string): Promise<void>;
  edit: ((page: Page, title: string, to: string) => Promise<void>) | null;
  tick: ((page: Page, title: string) => Promise<void>) | null;
  remove: ((page: Page, title: string) => Promise<void>) | null;
  isDone(page: Page, title: string): Promise<boolean>;
}

export const item = (page: Page, title: string): Locator => page.locator('li', {hasText: title});

async function typeAdd(page: Page, sel: string, title: string) {
  await page.locator(sel).fill(title);
  await page.locator(sel).press('Enter');
}
function button(label: string) {
  return async (page: Page, title: string) => {
    await item(page, title).first().getByRole('button', {name: label, exact: true}).click();
  };
}
function promptEdit(label: string) {
  return async (page: Page, title: string, to: string) => {
    page.once('dialog', d => d.accept(to));
    await item(page, title).first().getByRole('button', {name: label, exact: true}).click();
  };
}
async function checkbox(page: Page, title: string) {
  await item(page, title).first().locator('input[type=checkbox]').check();
}
const hasClass = (sel: string) => async (page: Page, title: string) =>
  (await item(page, title).first().locator(sel).count()) > 0 ||
  (await item(page, title).first().evaluate((el, s) => el.matches(s), sel));
const textHas = (marker: string) => async (page: Page, title: string) =>
  ((await item(page, title).first().textContent()) ?? '').includes(marker);
const root = async (r: Running) => r.base + '/';
const printedLink = async (r: Running) => {
  const m = r.out.match(/https?:\/\/\S+/);
  if (!m) throw new Error('no link printed at start-up');
  return m[0];
};
const noGate = async () => {};

export const ADAPTERS: Record<string, Adapter> = {
  p1: {
    listKind: 'implicit', address: root, addInput: '#text',
    gate: async (page, who) => {
      if (await page.locator('#name').isVisible()) {
        await page.locator('#name').fill(who);
        await page.locator('#name').press('Enter');
        await page.locator('#text').waitFor({state: 'visible', timeout: 5000});
      }
    },
    add: (p, t) => typeAdd(p, '#text', t),
    edit: promptEdit('Edit'), tick: button('Done'), remove: button('Delete'),
    isDone: hasClass('span.done'),
  },
  p2: {
    listKind: 'implicit', address: root, addInput: '#title',
    gate: async (page, who) => {
      await page.waitForTimeout(300);
      if (await page.locator('#account').isVisible()) {
        const id = who === 'B' ? '22222222-2222-4222-8222-222222222222' : '11111111-1111-4111-8111-111111111111';
        await page.locator('#account').fill(id);
        await page.locator('#account').press('Enter');
        await page.locator('#title').waitFor({state: 'visible', timeout: 5000});
      }
    },
    add: (p, t) => typeAdd(p, '#title', t),
    edit: promptEdit('Edit'), tick: button('Complete'), remove: null,
    isDone: hasClass('li.done'),
  },
  p3: {
    listKind: 'implicit', address: root, addInput: '#title', gate: noGate,
    add: (p, t) => typeAdd(p, '#title', t),
    edit: null, tick: checkbox, remove: null, isDone: hasClass('li.done'),
  },
  p4: {
    listKind: 'implicit', address: printedLink, addInput: '#title', gate: noGate,
    add: (p, t) => typeAdd(p, '#title', t),
    edit: promptEdit('Edit'), tick: button('Done'), remove: button('Remove'),
    isDone: textHas('[x]'),
  },
  p5: {
    listKind: 'implicit', address: root, addInput: '#title', gate: noGate,
    add: (p, t) => typeAdd(p, '#title', t),
    edit: null, tick: checkbox, remove: null, isDone: hasClass('li.done'),
  },
  p6: {
    listKind: 'implicit', address: printedLink, addInput: '#text', gate: noGate,
    add: (p, t) => typeAdd(p, '#text', t),
    edit: null, tick: button('Done'), remove: button('Delete'), isDone: textHas('(done)'),
  },
  p7: {
    listKind: 'implicit', address: root, addInput: 'input[name=title]', gate: noGate,
    add: async (p, t, who) => {
      await p.locator('#who').fill(who);
      await typeAdd(p, 'input[name=title]', t);
    },
    edit: null, tick: checkbox, remove: null, isDone: hasClass('li.Done'),
  },
  p8: {
    listKind: 'implicit', address: root, addInput: '#title', gate: noGate,
    add: (p, t) => typeAdd(p, '#title', t),
    edit: promptEdit('Edit'), tick: button('Done'), remove: button('Delete'),
    isDone: async (page, title) => (await page.locator('#done li', {hasText: title}).count()) > 0,
  },
  p9: {
    listKind: 'explicit',
    address: async (r, page) => {
      await page.goto(r.base + '/');
      await page.locator('#name').fill('Our list');
      await page.locator('#name').press('Enter');
      const a = page.locator('#out a');
      await a.waitFor({timeout: 5000});
      return new URL((await a.getAttribute('href'))!, r.base).href;
    },
    addInput: '#subject', gate: noGate,
    add: (p, t) => typeAdd(p, '#subject', t),
    edit: null, tick: button('Tick'), remove: button('Remove'), isDone: hasClass('span.done'),
  },
};
