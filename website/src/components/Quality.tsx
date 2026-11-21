import type {ReactNode} from 'react';
import {Marker} from './MetricChart';
import {stat, type Stat} from './evaluation';

/**
 * The blinded quality evaluation of delivered products (`experiments/<id>/scores.json`,
 * `sdc.quality-scores/1`, and its `rubric.md`). Every figure is read from those files; the
 * sentences on procedure and limits cite `docs/quality-0.1.0.md`.
 */

export interface QualityProduct {
  run: string;
  side: string;
  score: number | null;
  n: number | null;
  not_testable: number | null;
  failed: string[];
  cost_usd: number | null;
  wall_s: number | null;
  findings: string[];
}

export interface QualitySide {
  id: string;
  score: {median: number; min: number; max: number} | null;
  per_item_passes: Record<string, number>;
  runs: string[];
}

export interface Quality {
  experiment: string;
  n: number | null;
  rubric: string | null;
  items: Array<{id: string; name: string}>;
  sides: QualitySide[];
  products: QualityProduct[];
}

export const QUALITY_DOC = 'https://github.com/beyond10x/sdc/blob/main/docs/quality-0.1.0.md';

function DocCite({lines}: {lines: string}): ReactNode {
  const [a, b] = lines.split('-');
  return <a href={`${QUALITY_DOC}#L${a}${b ? `-L${b}` : ''}`}><code>docs/quality-0.1.0.md:{lines}</code></a>;
}

export interface SideStats {
  side: QualitySide;
  index: number;
  free: boolean;
  products: QualityProduct[];
  score: Stat | null;
  cost: Stat | null;
  wall: Stat | null;
}

/** The sides in falling median cost, the free side marked by the runs it shares with `freeRuns`. */
export function sideStats(q: Quality, freeRuns: string[]): SideStats[] {
  const free = new Set(freeRuns);
  return q.sides
    .map((side) => {
      const products = q.products.filter((p) => p.side === side.id);
      return {
        side,
        index: 0,
        free: side.runs.some((r) => free.has(r)),
        products,
        score: stat(products.map((p) => p.score)),
        cost: stat(products.map((p) => p.cost_usd)),
        wall: stat(products.map((p) => p.wall_s)),
      };
    })
    .sort((a, b) => (b.cost?.median ?? 0) - (a.cost?.median ?? 0))
    .map((s, index) => ({...s, index}));
}

/** The evaluation that scored these runs, if one is committed. */
export function qualityFor(all: Quality[] | undefined, runs: string[]): Quality | null {
  const want = new Set(runs);
  return (all ?? []).find((q) => q.products.some((p) => want.has(p.run))) ?? null;
}

const itemName = (q: Quality, id: string) => q.items.find((i) => i.id === id)?.name ?? id;

/** Items whose passes differ between sides; items that failed somewhere but equally on every side; items passed everywhere. */
export function itemSplit(q: Quality, sides: SideStats[]) {
  const ids = q.items.length ? q.items.map((i) => i.id) : Object.keys(sides[0]?.side.per_item_passes ?? {});
  const passes = (id: string) => sides.map((s) => s.side.per_item_passes[id] ?? 0);
  const differ = ids.filter((id) => new Set(passes(id)).size > 1);
  const allPass = ids.filter((id) => sides.every((s) => (s.side.per_item_passes[id] ?? 0) === s.products.length));
  const equalFail = ids.filter((id) => !differ.includes(id) && !allPass.includes(id));
  return {differ, allPass, equalFail, passes};
}

/** One sentence for the page's first paragraph: the cost factors, the point differences and the items that make them. */
export function qualitySentence(q: Quality, freeRuns: string[]): string | null {
  const sides = sideStats(q, freeRuns);
  const free = sides.find((s) => s.free);
  const sdc = sides.filter((s) => !s.free);
  if (!free?.cost || !free.score || !sdc.length) return null;
  const factors = sdc.map((s) => (s.cost ? Math.round(s.cost.median / free.cost!.median) : null)).filter((x): x is number => x !== null).sort((a, b) => a - b);
  const diffs = sdc.map((s) => (s.score ? s.score.median - free.score!.median : null)).filter((x): x is number => x !== null).sort((a, b) => a - b);
  const {differ} = itemSplit(q, sides);
  const names = differ.map((id) => itemName(q, id));
  const list = names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}` : names[0] ?? 'no item';
  const pts = diffs[0] === diffs[diffs.length - 1] ? `${diffs[0]}` : `${diffs[0]} to ${diffs[diffs.length - 1]}`;
  return `For about ${factors.join(' and ')} times the cost, the SDC products scored ${pts} points more of ${q.n} in a blinded browser evaluation, the difference being ${list}.`;
}

const LEFT = 46;
const RIGHT = 14;
const TOP = 14;
const BOTTOM = 40;
const W = 560;
const H = 250;

export function QualityCostChart({q, sides}: {q: Quality; sides: SideStats[]}): ReactNode {
  const all = sides.flatMap((s) => s.products.map((p) => ({p, s}))).filter(({p}) => typeof p.score === 'number' && typeof p.cost_usd === 'number' && p.cost_usd! > 0);
  const n = q.n ?? Math.max(...all.map(({p}) => p.score!));
  const yMin = Math.max(0, Math.min(...all.map(({p}) => p.score!)) - 3);
  const costs = all.map(({p}) => p.cost_usd!);
  const lo = Math.log10(Math.min(...costs) * 0.7);
  const hi = Math.log10(Math.max(...costs) * 1.3);
  const plotW = W - LEFT - RIGHT;
  const plotH = H - TOP - BOTTOM;
  const x = (c: number) => LEFT + ((Math.log10(c) - lo) / (hi - lo)) * plotW;
  const y = (v: number) => TOP + plotH - ((v - yMin) / (n - yMin)) * plotH;
  const xt = [0.2, 0.5, 1, 2, 5, 10, 20].filter((t) => Math.log10(t) >= lo && Math.log10(t) <= hi);
  const yt = Array.from({length: n - yMin + 1}, (_, i) => yMin + i);
  const desc = all.map(({p, s}) => `${p.run}, ${s.side.id}: ${p.score} of ${n}, $${p.cost_usd!.toFixed(2)}`).join('; ');
  return (
    <figure className="sdc-metric sdc-metric--wide" id="quality-cost">
      <figcaption>
        <strong>Quality points against cost, one mark per product</strong>
        <span className="sdc-metric__dir">Up is better (points of {n}), left is cheaper; cost on a log scale. Sides differ by shape.</span>
      </figcaption>
      <ul className="sdc-legend" aria-label="Sides">
        {sides.map((s) => (
          <li key={s.side.id}>
            <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><Marker i={s.index} x={8} y={8} r={4.5} /></svg>
            <span><strong>{s.side.id}</strong></span>
          </li>
        ))}
      </ul>
      <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label={`Quality points against cost per run. ${desc}.`} className="sdc-metric__svg">
        {yt.map((t) => (
          <g key={t}>
            <line x1={LEFT} x2={W - RIGHT} y1={y(t)} y2={y(t)} className="sdc-metric__grid" />
            <text x={LEFT - 6} y={y(t) + 4} textAnchor="end" className="sdc-metric__tick">{t}</text>
          </g>
        ))}
        {xt.map((t) => (
          <g key={t}>
            <line x1={x(t)} x2={x(t)} y1={TOP} y2={TOP + plotH} className="sdc-metric__grid" />
            <text x={x(t)} y={TOP + plotH + 14} textAnchor="middle" className="sdc-metric__tick">${t}</text>
          </g>
        ))}
        <text x={LEFT + plotW / 2} y={H - 6} textAnchor="middle" className="sdc-metric__tick">cost per run, USD (log scale)</text>
        <text x={12} y={TOP + plotH / 2} transform={`rotate(-90 12 ${TOP + plotH / 2})`} textAnchor="middle" className="sdc-metric__tick">points of {n}</text>
        {all.map(({p, s}) => (
          <g key={p.run} className="sdc-quality__mark">
            <title>{`${p.run}: ${p.score} of ${n}, $${p.cost_usd!.toFixed(2)}`}</title>
            <Marker i={s.index} x={x(p.cost_usd!)} y={y(p.score!)} />
          </g>
        ))}
      </svg>
    </figure>
  );
}

const usd = (x: number) => x.toFixed(3);
const secs = (x: number) => Math.round(x).toLocaleString('en-US');
const cell = (s: Stat | null, fmt: (x: number) => string) => (s ? `${fmt(s.median)} [${fmt(s.min)}–${fmt(s.max)}]` : 'no data');

export default function QualitySection({q, freeRuns}: {q: Quality; freeRuns: string[]}): ReactNode {
  const sides = sideStats(q, freeRuns);
  const {differ, allPass, equalFail, passes} = itemSplit(q, sides);
  const nProducts = q.products.length;
  const perSide = Array.from(new Set(sides.map((s) => s.products.length)));
  const withFindings = q.products.filter((p) => p.findings.length);
  const findings = withFindings.flatMap((p) => p.findings.map((f) => ({p, f})));
  const freeSide = new Set(sides.filter((s) => s.free).map((s) => s.side.id));
  const allSdc = withFindings.every((p) => !freeSide.has(p.side));
  return (
    <>
      <p>
        A blinded evaluation scored the {nProducts} products with one rubric of {q.n} items, written from the idea text
        before any product was opened (<code>{q.rubric ?? 'rubric.md'}</code>, <DocCite lines="22-24" />). Each product was
        built and started in a confinement and scored by a real browser on a copy renamed to <code>p1</code>…
        <code>p{nProducts}</code> in a random order (<DocCite lines="25-36" />). Blinding did not hide the side: READMEs citing
        the spec and the plan, or naming stories, findings and decisions, gave the origin away, as the report states (
        <DocCite lines="100-106" />).
      </p>
      <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Quality per side">
        <table className="sdc-table" id="quality-sides">
          <thead>
            <tr><th scope="col">Median [min–max]</th><th scope="col">Points of {q.n}</th><th scope="col">Cost per run, USD</th><th scope="col">Wall time per run, s</th><th scope="col">Products</th></tr>
          </thead>
          <tbody>
            {sides.map((s) => (
              <tr key={s.side.id}>
                <th scope="row">{s.side.id}</th>
                <td>{cell(s.score, String)}</td>
                <td>{cell(s.cost, usd)}</td>
                <td>{cell(s.wall, secs)}</td>
                <td>{s.products.length}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <QualityCostChart q={q} sides={sides} />
      <h4>Where the sides differ</h4>
      <ul id="quality-items">
        {differ.map((id) => (
          <li key={id}>
            <strong>{id} {itemName(q, id)}</strong>: passed in {sides.map((s, i) => `${passes(id)[i]} of ${s.products.length} (${s.side.id})`).join(', ')}
          </li>
        ))}
        {equalFail.map((id) => (
          <li key={id}>
            <strong>{id} {itemName(q, id)}</strong>: the same on every side, passed in {passes(id).join(' / ')} of{' '}
            {perSide.join(' / ')}
          </li>
        ))}
      </ul>
      <p>
        No free product has an edit or remove control (<DocCite lines="70-71" />). The other {allPass.length} items passed
        in all {nProducts} products.
      </p>
      <h4>Defects found by the exploratory probes</h4>
      <p>
        {findings.length} defects, {allSdc ? 'all in SDC products' : 'on both sides'}; the free products also offer fewer
        controls to go wrong (<DocCite lines="80-90" />).
      </p>
      <ul id="quality-defects">
        {findings.map(({p, f}) => <li key={`${p.run}-${f}`}><code>{p.run}</code>: {f}</li>)}
      </ul>
      <h4>Limits</h4>
      <ul>
        <li>{perSide.length === 1 ? `${perSide[0]} products per side` : `${perSide.join(', ')} products per side`}, one idea, one rubric that is one reading of the idea text.</li>
        <li>Offline was emulated in the browser (<code>setOffline</code>, a stopped server), not a phone losing its network; two browser contexts on one machine stand in for two people.</li>
        <li>The exploration was a fixed set of scripted probes, the same for every product, not free use.</li>
        <li>A browser test sees behaviour, not code quality or data integrity over weeks (<DocCite lines="116-125" />).</li>
      </ul>
    </>
  );
}
