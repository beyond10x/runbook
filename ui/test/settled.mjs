// What a fixed pause before a screenshot waited for, as events: the fonts are loaded, two frames have been drawn and
// every transition or animation that ends has ended (a spinner that never ends does not count). The pause cost
// the same on a quiet machine and a loaded one; this returns as soon as the page has settled.
export const settled = (page) => page.evaluate(async () => {
  await document.fonts.ready;
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  const running = () => document.getAnimations().filter((a) => a.playState === 'running' && a.effect?.getComputedTiming().iterations !== Infinity);
  await Promise.all(running().map((a) => a.finished.catch(() => {})));
  await new Promise((r) => requestAnimationFrame(r));
});
