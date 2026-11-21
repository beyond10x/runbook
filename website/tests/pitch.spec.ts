import {test, expect, type Page} from '@playwright/test';

const INTERVAL_MS = 6000;
const WAIT_MS = INTERVAL_MS + 2500;

const words = (s: string) => s.trim().split(/\s+/).filter(Boolean).length;
const sentences = (s: string) => (s.trim().match(/[.!?](?=\s|$)/g) ?? []).length;

async function open(page: Page): Promise<void> {
  const res = await page.goto('', {waitUntil: 'networkidle'});
  expect(res?.status()).toBe(200);
  await expect(page.locator('[data-pitch]')).toBeVisible();
}

const current = (page: Page) => page.locator('[data-pitch]').getAttribute('data-current');

test.describe('pitch slider', () => {
  test.use({viewport: {width: 1440, height: 900}});

  test('has exactly three slides, each with a short claim, a short line and a link to a page of the build', async ({page, request, baseURL}) => {
    await open(page);
    const slides = page.locator('[data-pitch-slide]');
    await expect(slides).toHaveCount(3);
    await expect(page.locator('[data-pitch-dot]')).toHaveCount(3);
    const hrefs: URL[] = [];
    for (let i = 0; i < 3; i++) {
      const slide = slides.nth(i);
      const claim = (await slide.locator('[data-pitch-claim]').textContent()) ?? '';
      const line = (await slide.locator('[data-pitch-line]').textContent()) ?? '';
      await expect(slide.locator('p').first(), `label of slide ${i + 1}`).toHaveText(/^[A-Za-z][A-Za-z ]+$/);
      expect(words(claim), `claim of slide ${i + 1}: "${claim}"`).toBeGreaterThan(0);
      expect(words(claim), `claim of slide ${i + 1}: "${claim}"`).toBeLessThanOrEqual(8);
      expect(words(line), `line of slide ${i + 1}: "${line}"`).toBeGreaterThan(0);
      expect(words(line), `line of slide ${i + 1}: "${line}"`).toBeLessThanOrEqual(20);
      expect(sentences(line), `sentences in the line of slide ${i + 1}: "${line}"`).toBe(1);
      await expect(slide.locator('svg[role="img"]')).toHaveCount(1);
      const links = slide.locator('[data-pitch-link]');
      await expect(links).toHaveCount(1);
      const href = await links.evaluate((a) => (a as HTMLAnchorElement).href);
      const u = new URL(href);
      expect(u.origin).toBe(new URL(baseURL!).origin);
      const r = await request.get(u.origin + u.pathname);
      expect(r.status(), `status of ${u.pathname}`).toBe(200);
      hrefs.push(u);
    }
    for (const u of hrefs.filter((x) => x.hash)) {
      await page.goto(u.href, {waitUntil: 'networkidle'});
      await expect(page.locator(`[id="${decodeURIComponent(u.hash.slice(1))}"]`), `anchor ${u.hash} on ${u.pathname}`).toBeAttached();
    }
  });

  test('next, previous and the dots change the current slide', async ({page}) => {
    await open(page);
    expect(await current(page)).toBe('0');
    await page.locator('[data-pitch-next]').click();
    expect(await current(page)).toBe('1');
    await page.locator('[data-pitch-next]').click();
    expect(await current(page)).toBe('2');
    await page.locator('[data-pitch-next]').click();
    expect(await current(page)).toBe('0');
    await page.locator('[data-pitch-prev]').click();
    expect(await current(page)).toBe('2');
    await page.locator('[data-pitch-dot="1"]').click();
    expect(await current(page)).toBe('1');
    await expect(page.locator('[data-pitch-dot="1"]')).toHaveAttribute('aria-current', 'true');
    await expect(page.locator('[data-pitch-dot][aria-current="true"]')).toHaveCount(1);
    await expect(page.locator('[data-pitch-slide="1"] [data-pitch-claim]')).toBeInViewport();
  });

  test('the arrow keys change the current slide when the slider has focus', async ({page}) => {
    await open(page);
    await page.locator('[data-pitch]').focus();
    await page.keyboard.press('ArrowRight');
    expect(await current(page)).toBe('1');
    await page.keyboard.press('ArrowRight');
    expect(await current(page)).toBe('2');
    await page.keyboard.press('ArrowLeft');
    expect(await current(page)).toBe('1');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    expect(await current(page)).toBe('2');
  });

  test('only the current slide can be reached by keyboard', async ({page}) => {
    await open(page);
    for (let i = 0; i < 3; i++) {
      await page.locator(`[data-pitch-dot="${i}"]`).click();
      for (let j = 0; j < 3; j++) {
        const inert = await page.locator(`[data-pitch-slide="${j}"]`).evaluate((e) => (e as HTMLElement).inert);
        expect(inert, `slide ${j + 1} inert while slide ${i + 1} is current`).toBe(i !== j);
      }
    }
  });

  test('it advances by itself, and not while the pointer is over it', async ({page}) => {
    await page.clock.install();
    await page.mouse.move(5, 5);
    await open(page);
    await page.clock.runFor(WAIT_MS);
    await expect.poll(() => current(page), {message: 'advanced by itself'}).toBe('1');
    await page.locator('[data-pitch]').hover();
    const before = await current(page);
    await page.clock.runFor(WAIT_MS);
    expect(await current(page), 'no advance while hovered').toBe(before);
  });

  test.describe('reduced motion', () => {
    test.use({reducedMotion: 'reduce'});
    test('does not advance by itself', async ({page}) => {
      await page.clock.install();
      await page.mouse.move(5, 5);
      await open(page);
      await page.clock.runFor(WAIT_MS);
      expect(await current(page)).toBe('0');
      await expect(page.locator('[data-pitch-play]')).toHaveCount(0);
    });
  });
});

for (const width of [1440, 390]) {
  test(`no graphic text leaves its box or its graphic at ${width} px`, async ({page}) => {
    await page.setViewportSize({width, height: 900});
    await open(page);
    const clipped = await page.locator('[data-pitch-slide] svg[role="img"]').evaluateAll((svgs) => {
      const out: string[] = [];
      for (const svg of svgs as SVGSVGElement[]) {
        const vb = svg.viewBox.baseVal;
        for (const t of Array.from(svg.querySelectorAll('text'))) {
          const b = t.getBBox();
          if (b.x < vb.x - 0.5 || b.x + b.width > vb.x + vb.width + 0.5) out.push(`"${t.textContent}" leaves the graphic`);
          const rect = t.parentElement?.querySelector(':scope > rect') as SVGRectElement | null;
          if (rect) {
            const r = rect.getBBox();
            if (b.x + b.width > r.x + r.width - 2) out.push(`"${t.textContent}" leaves its box`);
          }
        }
      }
      return out;
    });
    expect(clipped).toEqual([]);
  });
}

test.describe('pitch slider at 390 px', () => {
  test.use({viewport: {width: 390, height: 844}, hasTouch: true, reducedMotion: 'reduce'});

  test('fits the viewport with tappable controls', async ({page}) => {
    await open(page);
    for (let i = 0; i < 3; i++) {
      await page.locator(`[data-pitch-dot="${i}"]`).click();
      const scroll = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(scroll, 'horizontal page scroll in px').toBeLessThanOrEqual(0);
      const box = await page.locator(`[data-pitch-slide="${i}"]`).boundingBox();
      expect(box!.x).toBeGreaterThanOrEqual(0);
      expect(box!.x + box!.width).toBeLessThanOrEqual(390);
      const fig = await page.locator(`[data-pitch-slide="${i}"] svg[role="img"]`).boundingBox();
      expect(fig!.x + fig!.width).toBeLessThanOrEqual(390);
    }
    const targets = page.locator('[data-pitch-prev], [data-pitch-next], [data-pitch-dot]');
    await expect(targets).toHaveCount(5);
    for (const b of await targets.all()) {
      const r = (await b.boundingBox())!;
      expect(r.width, 'target width').toBeGreaterThanOrEqual(40);
      expect(r.height, 'target height').toBeGreaterThanOrEqual(40);
    }
  });

  test('a swipe changes the slide', async ({page}) => {
    await open(page);
    const box = (await page.locator('[data-pitch-slide="0"]').boundingBox())!;
    const y = box.y + 60;
    const swipe = async (from: number, to: number) => {
      await page.locator('[data-pitch]').dispatchEvent('touchstart', {touches: [{identifier: 1, clientX: from, clientY: y}], changedTouches: [{identifier: 1, clientX: from, clientY: y}]});
      await page.locator('[data-pitch]').dispatchEvent('touchend', {touches: [], changedTouches: [{identifier: 1, clientX: to, clientY: y}]});
    };
    await swipe(300, 80);
    expect(await current(page)).toBe('1');
    await swipe(80, 300);
    expect(await current(page)).toBe('0');
  });
});
