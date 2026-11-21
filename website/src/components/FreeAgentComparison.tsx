import type {ReactNode} from 'react';
import useBrokenLinks from '@docusaurus/useBrokenLinks';
import {Lanes, dur, usd, type Timeline} from './RunTimeline';
import {stat, median, type Stat} from './evaluation';
import QualitySection, {qualityFor, qualitySentence, type Quality} from './Quality';

/**
 * The comparison of the SDC with one free-running agent, from the committed experiment record
 * (cost, tokens, hypothesis, verdict, confounds), its run summaries (wall time, API requests,
 * reviews, rework), its timelines, `sizes.json` and the conformance summaries. Every figure is
 * read from those files; a value no file holds is shown as "no data", never as 0.
 */

export interface RunRow {
  experiment: string;
  run: string;
  format: string;
  wall_s: number | null;
  api_requests: number | null;
  decider_calls: number | null;
  rework_takes: number | null;
  rework_cost_usd: number | null;
  reviews: number | null;
  review_rejections: number | null;
  merge_rejections: number | null;
}

export interface ConfRow {
  experiment: string;
  run: string;
  measured: boolean;
  n: number | null;
  k: number | null;
  f: number | null;
  p: number | null;
  u: number | null;
  tables_counted: number | null;
  tables_written: number | null;
  summary: string | null;
}

export interface SizeRow {
  experiment: string;
  run: string;
  product_lines: number | null;
  asset_lines: number | null;
  tests: number | null;
  requirements: number | null;
  scenarios: number | null;
  files: number | null;
}

export type Tl = Timeline & {__experiment: string; __file: string};

interface Side {
  id: string;
  label: string;
  free: boolean;
  runs: string[];
  cost: Stat | null;
  tokens: Stat | null;
  wall: Stat | null;
  api: Stat | null;
  apiFromTimelines: boolean;
}

export interface Model {
  record: any;
  sides: Side[];
  free: Side;
  base: Side[];
  runs: Map<string, RunRow>;
  conf: Map<string, ConfRow>;
  sizes: Map<string, SizeRow>;
  timelines: Map<string, Tl>;
}

const SDC_WRITEUP = 'https://github.com/beyond10x/sdc/blob/main/docs/llm-is-not-the-runtime.md';

function fromPrimary(p: any): Stat | null {
  return p && typeof p.median === 'number' ? {median: p.median, min: p.min, max: p.max, n: p.n} : null;
}

/** The comparison model, or null when no committed record compares a free-running agent with SDC settings. */
export function comparisonModel(all: Tl[], experiments: any[], runs: RunRow[], conformance: ConfRow[], sizes: SizeRow[]): Model | null {
  const freeIds = new Set(all.filter((t) => t.kind === 'free-agent').map((t) => t.__experiment));
  const record = experiments.find((e) => freeIds.has(e.id) && e.treatment && e.baseline?.length && e.variant_primaries?.length);
  if (!record) return null;
  const runMap = new Map(runs.filter((r) => r.experiment === record.id).map((r) => [r.run, r]));
  const tlMap = new Map(all.map((t) => [t.run, t]));
  const side = (id: string): Side => {
    const v = record.variants.find((x: any) => x.id === id);
    const names: string[] = v?.runs ?? [];
    const prim = record.variant_primaries.find((x: any) => x.id === id)?.primaries ?? {};
    const rows = names.map((n) => runMap.get(n)).filter((r): r is RunRow => Boolean(r));
    let api = stat(rows.map((r) => r.api_requests));
    let apiFromTimelines = false;
    if (!api) {
      api = stat(names.map((n) => tlMap.get(n)?.totals.api_requests ?? null));
      apiFromTimelines = api !== null;
    }
    const free = id === record.treatment;
    return {
      id,
      label: free ? `Free agent (${id})` : `SDC ${id}`,
      free,
      runs: names,
      cost: fromPrimary(prim['run.cost_usd']),
      tokens: fromPrimary(prim['run.tokens_total']),
      wall: stat(rows.map((r) => r.wall_s)),
      api,
      apiFromTimelines,
    };
  };
  const base = record.baseline.map(side);
  const free = side(record.treatment);
  const runNames = new Set([...base, free].flatMap((s) => s.runs));
  return {
    record,
    sides: [...base, free],
    free,
    base,
    runs: runMap,
    conf: new Map(conformance.filter((c) => runNames.has(c.run)).map((c) => [c.run, c])),
    sizes: new Map(sizes.filter((s) => s.experiment === record.id).map((s) => [s.run, s])),
    timelines: tlMap,
  };
}

const factor = (a: Stat | null, b: Stat | null) => (a && b && b.median > 0 ? a.median / b.median : null);
const about = (x: number | null) => (x === null ? 'an unknown number of' : String(Math.round(x)));
const int = (x: number) => (Number.isInteger(x) ? x.toLocaleString('en-US') : x.toLocaleString('en-US', {maximumFractionDigits: 1}));

function sizeMedian(m: Model, sides: Side[], key: keyof SizeRow): number | null {
  const v = sides.flatMap((s) => s.runs).map((r) => m.sizes.get(r)?.[key]).filter((x): x is number => typeof x === 'number').sort((a, b) => a - b);
  return v.length ? median(v) : null;
}

/** The result in one paragraph, every figure computed from the record, the run summaries and the sizes. */
export function ResultLede({m, quality}: {m: Model; quality?: Quality[]}): ReactNode {
  const q = qualityFor(quality, m.free.runs);
  const qs = q ? qualitySentence(q, m.free.runs) : null;
  const idea = m.record.inputs?.[0]?.name ?? 'the idea';
  const cheaper = m.base.map((b) => about(factor(b.cost, m.free.cost))).join(' and ');
  const faster = m.base.map((b) => about(factor(b.wall, m.free.wall))).join(' and ');
  const lines = [sizeMedian(m, m.base, 'product_lines'), sizeMedian(m, [m.free], 'product_lines')];
  const tests = [sizeMedian(m, m.base, 'tests'), sizeMedian(m, [m.free], 'tests')];
  const reqs = [sizeMedian(m, m.base, 'requirements'), sizeMedian(m, [m.free], 'requirements')];
  const smaller = lines.every((x) => x !== null) && lines[1]! < lines[0]!;
  return (
    <p className="sdc-lede" id="result">
      On the idea <code>{idea}</code>, the free-running agent was about {cheaper} times cheaper than the SDC settings{' '}
      {m.base.map((b) => b.id).join(' and ')}, and about {faster} times faster
      {smaller ? (
        <>
          , and it delivered a smaller product: median {int(lines[1]!)} Rust product lines against {int(lines[0]!)}, {int(tests[1]!)} tests against{' '}
          {int(tests[0]!)}, {int(reqs[1]!)} requirements against {int(reqs[0]!)}
        </>
      ) : null}
      . The registered hypothesis, that the free agent costs more, is <strong>{m.record.verdict ?? 'not computed'}</strong> (
      <a href="#free">the comparison below</a>).
      {qs ? <> {qs.replace(/\.$/, '')} (<a href="#quality">quality</a>).</> : null}
    </p>
  );
}

function CostBar({label, cost, max}: {label: string; cost: number | null | undefined; max: number}): ReactNode {
  const c = cost ?? 0;
  return (
    <div className="sdc-costbar" aria-label={`${label}: cost ${usd(c)} on a scale to ${usd(max)}`} role="img">
      <span className="sdc-costbar__fill" style={{width: `${max ? (c / max) * 100 : 0}%`}} />
      <span className="sdc-costbar__label">{usd(c)}</span>
    </div>
  );
}

const cell = (s: Stat | null, fmt: (x: number) => string) => (s ? `${fmt(s.median)} [${fmt(s.min)}, ${fmt(s.max)}] (n=${s.n})` : 'no data');

function MediansTable({m}: {m: Model}): ReactNode {
  const rows: Array<[ReactNode, (s: Side) => string]> = [
    [<><code>run.cost_usd</code>, USD</>, (s) => cell(s.cost, (x) => x.toFixed(3))],
    [<><code>run.tokens_total</code></>, (s) => cell(s.tokens, (x) => Math.round(x).toLocaleString('en-US'))],
    [<><code>run.wall_s</code>, s</>, (s) => cell(s.wall, (x) => int(Math.round(x * 10) / 10))],
    [<>API requests</>, (s) => `${cell(s.api, (x) => String(Math.round(x)))}${s.apiFromTimelines ? ' †' : ''}`],
  ];
  const anyTl = m.sides.some((s) => s.apiFromTimelines);
  return (
    <>
      <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Medians per setting">
        <table className="sdc-table" id="free-medians">
          <thead>
            <tr><th scope="col">Median [min, max] (n)</th>{m.sides.map((s) => <th key={s.id} scope="col">{s.label}</th>)}</tr>
          </thead>
          <tbody>
            {rows.map(([k, f], i) => (
              <tr key={i}><th scope="row">{k}</th>{m.sides.map((s) => <td key={s.id}>{f(s)}</td>)}</tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="sdc-muted">
        Cost and <code>run.tokens_total</code> from the record's result, wall time and API requests from the committed run
        summaries.
        {anyTl ? ' † The SDC run summaries do not count API requests; this figure is from the committed timelines of that setting, so n is the number of timelines.' : ''}{' '}
        On the SDC side <code>run.tokens_total</code> counts the model-node sessions and leaves the decider out; its cost is in{' '}
        <code>run.cost_usd</code>.
      </p>
    </>
  );
}

function range(v: any, fmt: (x: number) => string): string {
  return v ? `${fmt(v.min)} to ${fmt(v.max)}` : 'no data';
}

function Verdict({m}: {m: Model}): ReactNode {
  const r = m.record;
  const fmt = (metric: string) => (metric === 'run.cost_usd' ? (x: number) => `${x.toFixed(3)} USD` : (x: number) => Math.round(x).toLocaleString('en-US'));
  return (
    <>
      <dl className="sdc-kv">
        <dt>Record</dt><dd><code>experiments/{r.id}.yaml</code></dd>
        <dt>Hypothesis</dt><dd>{r.hypothesis}</dd>
        <dt>Decision rule</dt><dd>{r.decision_rule?.text ?? r.decision_rule?.rule ?? 'not recorded'}</dd>
        <dt>Verdict</dt><dd><strong>{r.verdict ?? 'not computed'}</strong></dd>
      </dl>
      <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Verdict per pair">
        <table className="sdc-table" id="free-verdict">
          <thead>
            <tr><th scope="col">Metric</th><th scope="col">Against</th><th scope="col">Verdict</th><th scope="col">Free runs</th><th scope="col">Baseline runs</th><th scope="col">p, one-sided</th></tr>
          </thead>
          <tbody>
            {r.comparisons.map((c: any) => (
              <tr key={`${c.metric}-${c.baseline}`}>
                <th scope="row"><code>{c.metric}</code></th>
                <td>{c.baseline}</td>
                <td><strong>{c.verdict}</strong></td>
                <td>{range(c.treatment_values, fmt(c.metric))}</td>
                <td>{range(c.baseline_values, fmt(c.metric))}</td>
                <td>{c.p_one_sided ?? '–'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {r.expectation ? <p className="sdc-muted">{r.expectation}</p> : null}
    </>
  );
}

const SIZE_COLS: Array<[keyof SizeRow, string]> = [
  ['product_lines', 'Rust product lines'],
  ['asset_lines', 'web asset lines'],
  ['tests', 'tests'],
  ['requirements', 'requirements'],
  ['scenarios', 'scenarios'],
];

function Sizes({m}: {m: Model}): ReactNode {
  const groups: Array<[string, Side[]]> = [['median, SDC', m.base], [`median, ${m.free.label.toLowerCase()}`, [m.free]]];
  const num = (x: unknown) => (typeof x === 'number' ? int(x) : 'no data');
  return (
    <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Product sizes">
      <table className="sdc-table" id="free-sizes">
        <thead>
          <tr><th scope="col">Run</th>{SIZE_COLS.map(([k, l]) => <th key={k} scope="col">{l}</th>)}</tr>
        </thead>
        <tbody>
          {m.sides.flatMap((s) => s.runs).map((r) => (
            <tr key={r}><th scope="row"><code>{r}</code></th>{SIZE_COLS.map(([k]) => <td key={k}>{num(m.sizes.get(r)?.[k])}</td>)}</tr>
          ))}
          {groups.map(([label, sides]) => (
            <tr key={label}><th scope="row">{label}</th>{SIZE_COLS.map(([k]) => <td key={k}>{num(sizeMedian(m, sides, k))}</td>)}</tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Conformance({m}: {m: Model}): ReactNode {
  return (
    <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Conformance of both sides">
      <table className="sdc-table" id="free-conformance">
        <thead>
          <tr><th scope="col">Run</th><th scope="col">Conformance to its own specification</th><th scope="col">Tables counted</th></tr>
        </thead>
        <tbody>
          {m.sides.flatMap((s) => s.runs).map((r) => {
            const c = m.conf.get(r);
            return (
              <tr key={r}>
                <th scope="row"><code>{r}</code></th>
                <td className="sdc-conf-text">{c?.summary ?? <span className="sdc-muted">not observed</span>}</td>
                <td>{c?.tables_counted != null ? `${c.tables_counted} of ${c.tables_written ?? '–'}` : '–'}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function Records({m}: {m: Model}): ReactNode {
  const per = (s: Side, f: (r: RunRow) => number | null, fmt: (x: number) => string = int) => cell(stat(s.runs.map((r) => {
    const row = m.runs.get(r);
    return row ? f(row) : null;
  })), fmt);
  const sum = (s: Side, f: (r: RunRow) => number | null) => {
    const v = s.runs.map((r) => m.runs.get(r)).map((r) => (r ? f(r) : null)).filter((x): x is number => typeof x === 'number');
    if (!v.length) return 'no data';
    return `${v.reduce((a, b) => a + b, 0)} in ${v.filter((x) => x > 0).length} of ${v.length} runs`;
  };
  const steps = (s: Side) => cell(stat(s.runs.map((r) => m.timelines.get(r)?.totals.program_steps ?? null)), int);
  const rows: Array<[ReactNode, (s: Side) => string]> = [
    [<>Program steps, checked by the driver (from the timelines)</>, steps],
    [<>Reviews by a separate node, <code>change.review</code>, per run</>, (s) => per(s, (r) => r.reviews)],
    [<>Reviews rejected, then implemented again</>, (s) => sum(s, (r) => r.review_rejections)],
    [<><code>merge.approve</code> rejections</>, (s) => sum(s, (r) => r.merge_rejections)],
    [<>Rework takes per run</>, (s) => per(s, (r) => r.rework_takes)],
    [<>Rework cost per run, USD</>, (s) => per(s, (r) => r.rework_cost_usd, (x) => x.toFixed(3))],
    [<>Decider calls per run</>, (s) => per(s, (r) => r.decider_calls)],
  ];
  return (
    <>
      <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="What each side records">
        <table className="sdc-table" id="free-records">
          <thead>
            <tr><th scope="col">Median [min, max] (n), or count</th>{m.sides.map((s) => <th key={s.id} scope="col">{s.label}</th>)}</tr>
          </thead>
          <tbody>
            {rows.map(([k, f], i) => (
              <tr key={i}><th scope="row">{k}</th>{m.sides.map((s) => <td key={s.id}>{f(s)}</td>)}</tr>
            ))}
          </tbody>
        </table>
      </div>
      <p>
        The run directories also hold a Canon decision record and evidence records for every step, and every case replays to
        its logged decision and state digests; the free run has neither, and no replay exists for it. Those counts are in the
        write-up, not in a committed per-run file (<a href={`${SDC_WRITEUP}#L101-L117`}><code>docs/llm-is-not-the-runtime.md:101-117</code></a>).
        Rework cost is money the SDC spent on second takes and is part of its cost above.
      </p>
    </>
  );
}

function Held({m}: {m: Model}): ReactNode {
  const r = m.record;
  const settings = (id: string) => r.variants.find((v: any) => v.id === id)?.settings ?? {};
  const freeModel = settings(m.free.id)['free.model'];
  const sameModel = freeModel && m.base.every((b) => settings(b.id)['models.implement.model'] === freeModel);
  const input = r.inputs?.[0];
  const sha = r.input_sha256?.[0];
  return (
    <>
      <p>Held equal, as the record states it:</p>
      <ul>
        {input ? <li>the idea <code>{input.name}</code>: "{input.text}"{sha ? <> (sha256 <code>{sha.slice(0, 12)}</code>)</> : null}</li> : null}
        {sameModel ? <li>the implementing model, <code>{freeModel}</code>, on both sides</li> : null}
        {r.variable ? <li>one variable: <code>{r.variable}</code></li> : null}
        <li>
          the rest of the list (deliverables, effort, Claude Code binary, builds through slot, day and host) is in the write-up (
          <a href={`${SDC_WRITEUP}#L151-L162`}><code>docs/llm-is-not-the-runtime.md:151-162</code></a>)
        </li>
      </ul>
      <p>Not equal, the record's known confounds:</p>
      <ul className="sdc-confounds">
        {r.known_confounds.map((c: any) => <li key={c.key}><strong>{c.key}</strong>: {c.note}</li>)}
      </ul>
    </>
  );
}

export default function FreeAgentComparison({m, quality}: {m: Model; quality?: Quality[]}): ReactNode {
  useBrokenLinks().collectAnchor('quality');
  const q = qualityFor(quality, m.free.runs);
  const sdc = m.base.flatMap((s) => s.runs).map((r) => m.timelines.get(r)).filter((t): t is Tl => Boolean(t));
  const free = m.free.runs.map((r) => m.timelines.get(r)).find((t): t is Tl => Boolean(t)) ?? null;
  const shown = free ? [...sdc, free] : sdc;
  const axis = Math.max(1, ...shown.map((t) => t.totals.wall_s));
  const cmax = Math.max(0, ...shown.map((t) => t.totals.cost_usd ?? 0));
  return (
    <>
      <p>
        The record <code>experiments/{m.record.id}.yaml</code> ran the same idea {m.free.runs.length} times as one agent given
        the idea and the cycle's deliverables and left to run, against {m.base.map((b) => `${b.runs.length} SDC runs of setting ${b.id}`).join(' and ')}.
        The committed timelines are drawn below on one time axis of {dur(axis)} and one cost scale to {usd(cmax)}: the SDC
        runs first, the free run under them.
      </p>
      <div className="sdc-compare">
        {shown.map((t) => (
          <div key={t.run}>
            <h4>{t.kind === 'free-agent' ? 'Free-running agent' : 'SDC'} · <code>{t.run}</code></h4>
            <div className="sdc-tl-viewport" tabIndex={0} role="region" aria-label={`${t.run} on the shared axis`}><Lanes t={t} axis={axis} compact /></div>
            <CostBar label={t.run} cost={t.totals.cost_usd} max={cmax} />
          </div>
        ))}
      </div>
      <h3 id="free-medians-h">Medians</h3>
      <MediansTable m={m} />
      <h3 id="free-verdict-h">The registered hypothesis and its verdict</h3>
      <Verdict m={m} />
      <h3 id="free-sizes-h">What was delivered: the size of the products</h3>
      <p>
        Both sides counted by one program with one rule (<code>experiments/{m.record.id}/sizes.json</code>). The free agent did a
        smaller job: the cost factors above compare a smaller product with a larger one. The counts do not say which product
        does more of what the idea asks.
      </p>
      <Sizes m={m} />
      <h3 id="free-conformance-h">Conformance of both sides</h3>
      <p>
        Observed after the verdict with the same tool on both sides, each product against the specification its own run wrote.
        The suites differ in what they hold and how many scenarios they have, so these figures do not rank the products. They
        decide nothing in the record.
      </p>
      <Conformance m={m} />
      <h3 id="free-records-h">What the SDC run records that the free run does not</h3>
      <p>Only what a program counted. The free agent built and tested its own code; no program or second model checked its work during the run.</p>
      <Records m={m} />
      <h3 id="free-held-h">What was held equal, and the confounds</h3>
      <Held m={m} />
      <h3 id="quality">Quality of the products, blinded</h3>
      {q ? <QualitySection q={q} freeRuns={m.free.runs} /> : <p><strong>Quality evaluation: pending.</strong> A blind evaluation of the {m.sides.flatMap((s) => s.runs).length} products is running; a later release fills this line.</p>}
    </>
  );
}
