import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import MetricChart, {CleanPassChart, CostSplitChart, Marker, MarkerIcon, SeriesLegend, type CostSplitRow, type RefLine} from '../components/MetricChart';
import {
  METRICS,
  NODES,
  chartedSettings,
  classModels,
  releases as releasesOf,
  runsOf,
  shortModel,
  stat,
  stopsByCause,
  type Ledger,
  type LedgerEntry,
  type LedgerRun,
  type LedgerSetting,
  type MetricDef,
  type Stat,
} from '../components/evaluation';
import {QUALITY_DOC, sideStats, type Quality} from '../components/Quality';

interface Experiment {
  id: string;
  kind: string;
  status: string | null;
  title: string | null;
  purpose: string | null;
  hypothesis: string | null;
  variable: string | null;
  treatment: string | null;
  verdict: string | null;
  inputs: Array<{name: string; kind: string; text: string | null}>;
  fixed: Record<string, string>;
  variants: Array<{id: string; settings: Record<string, string>; runs: string[]}>;
  runs_per_variant: number | null;
  pass_rule: string | null;
}

interface Target {
  id: string;
  metric: string | null;
  input: string | null;
  target: number | null;
  set: string | null;
  source_experiment: string | null;
}

interface RunCost {
  experiment: string;
  run: string;
  format: string;
  cost_usd: number | null;
  wall_s: number | null;
  cost_by_class: Record<string, number> | null;
}

interface Props {
  evaluation: {
    progress: Ledger;
    experiments: Experiment[];
    releasesTable: {input?: {name: string; description?: string; sha256: string}} | null;
    nodeClasses: {classes: Record<string, string[]>; default_efforts: Record<string, string>};
    quality?: Quality[];
    targets?: {targets: Target[]} | null;
    runs?: RunCost[];
    fixtures: boolean;
  };
}

const TAG = '0.1.1';

function Cite({path, lines, at = TAG}: {path: string; lines?: [number, number?]; at?: string}): ReactNode {
  const anchor = lines ? `#L${lines[0]}${lines[1] ? `-L${lines[1]}` : ''}` : '';
  const label = lines ? `${path}:${lines[0]}${lines[1] ? `-${lines[1]}` : ''}` : path;
  return (
    <a href={`https://github.com/beyond10x/runbook/blob/${at}/${path}${anchor}`}>
      <code>{label}</code>
    </a>
  );
}

const usd = (x: number | null | undefined) => (typeof x === 'number' ? `$${x.toFixed(2)}` : '–');
const mins = (x: number | null | undefined) => (typeof x === 'number' ? `${Math.round(x / 60)} min` : '–');
const num = (x: number | null | undefined, d = 0) => (typeof x === 'number' ? (d ? x.toFixed(d) : String(x)) : '–');
const pct = (x: number | null | undefined) => (typeof x === 'number' ? `${Math.round(x * 100)} %` : '–');
const factor = (x: number) => (x >= 9.95 ? x.toFixed(0) : x.toFixed(1));
const list = (xs: string[]) => xs.join(', ');

const metricById = (id: string): MetricDef => METRICS.find((m) => m.id === id)!;
const COST = metricById('cost');
const WALL = metricById('wall');
const MAIN = new Set(['cost', 'wall']);

/** A committed quality evaluation, described as a case, with its medians per side and a link to the full result. */
function QualityCase({q, freeRuns}: {q: Quality; freeRuns: string[]}): ReactNode {
  const sides = sideStats(q, freeRuns);
  return (
    <>
      <h3 id={`case-${q.experiment}`}>The quality case: {q.products.length} delivered products, blinded</h3>
      <p>
        The delivered products of one idea, from SDC runs and from free-running agent sessions, scored on blinded copies
        by one rubric of {q.n} items written from the idea text before any product was opened, in a real browser,
        including offline use and two people sharing one list (<code>experiments/{q.experiment}/</code>). Median points of{' '}
        {q.n}: {sides.map((s) => `${s.side.id} ${s.score ? `${s.score.median} [${s.score.min}–${s.score.max}]` : 'no data'}`).join(', ')}.
        The result, with cost, the items that differ, the defects found and the limits:{' '}
        <Link to="/llm-is-not-the-runtime#quality">The LLM is not the runtime, quality</Link>; the report:{' '}
        <a href={QUALITY_DOC}><code>docs/quality-0.1.0.md</code></a>.
      </p>
    </>
  );
}

/** The failed count, with the scenarios refused for a precondition the specification does not model split out. */
function failedCell(run: LedgerRun): ReactNode {
  const c = run.conformance;
  if (!c) return <span className="sdc-muted">–</span>;
  if (!c.measured || typeof c.f !== 'number') return <span className="sdc-muted">not measured</span>;
  if (!c.p) return String(c.f);
  return (
    <>
      {c.f} <span className="sdc-muted">(of which {c.p}: precondition not in the specification)</span>
    </>
  );
}

function RunTable({entry, settings, labels}: {entry: LedgerEntry; settings: string[]; labels: Map<string, string>}): ReactNode {
  const rows: Array<{setting: string; run: LedgerRun}> = settings.flatMap((s) => runsOf(entry, s).map((run) => ({setting: s, run})));
  if (!rows.length) return <p className="sdc-muted">No runs on this tree yet.</p>;
  return (
    <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label={`Runs of ${entry.label}`}>
      <table className="sdc-table sdc-table--runs">
        <thead>
          <tr>
            <th scope="col">Setting</th>
            <th scope="col">End</th>
            <th scope="col">Clean</th>
            <th scope="col">Conformance (lower bound)</th>
            <th scope="col">Failed</th>
            <th scope="col">Cost</th>
            <th scope="col">Wall</th>
            <th scope="col">Model calls</th>
            <th scope="col">Decider calls</th>
            <th scope="col">Rework takes</th>
            <th scope="col">Review rej.</th>
            <th scope="col">Merge rej.</th>
            <th scope="col">Waiting</th>
            {NODES.map((n) => <th key={n} scope="col">Loop / FPY <code>{n}</code></th>)}
          </tr>
        </thead>
        <tbody>
          {rows.map(({setting, run}) => {
            const m = run.metrics ?? null;
            return (
              <tr key={run.run}>
                <td>{labels.get(setting) ?? setting}</td>
                <td><code>{run.end}</code></td>
                <td>{run.clean_pass ? '✓ yes' : 'no'}</td>
                <td className="sdc-conf-text">{run.conformance ? run.conformance.summary : <span className="sdc-muted">not observed</span>}</td>
                <td className="sdc-conf-failed">{failedCell(run)}</td>
                <td>{usd(m?.cost_usd ?? run.cost_usd)}</td>
                <td>{mins(m?.wall_s ?? run.wall_s)}</td>
                <td>{num(m?.llm_calls)}</td>
                <td>{num(m?.decider_calls)}</td>
                <td>{num(m?.rework_takes)}</td>
                <td>{num(m?.review_rejections)}</td>
                <td>{num(m?.merge_rejections)}</td>
                <td>{mins(m?.waiting_on_person_s)}</td>
                {NODES.map((n) => {
                  const x = m?.nodes?.[n] ?? null;
                  return <td key={n}>{num(x?.looping_factor, 2)} / {pct(x?.first_pass_yield)}</td>;
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function StopsTable({entry, settings}: {entry: LedgerEntry; settings: LedgerSetting[]}): ReactNode {
  const ids = settings.map((s) => s.id);
  const stops = stopsByCause(entry, ids);
  if (!stops.size) return <p>{entry.label}: no run stopped; every ended run closed.</p>;
  return (
    <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label={`Stops by cause in ${entry.label}`}>
      <table className="sdc-table sdc-table--compact">
        <caption>{entry.label}</caption>
        <thead>
          <tr>
            <th scope="col">Cause</th>
            {settings.map((s, i) => (
              <th key={s.id} scope="col">
                <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><Marker i={i} x={6} y={6} r={3.5} /></svg> {s.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {[...stops.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([cause, row]) => (
            <tr key={cause}>
              <th scope="row"><code>{cause}</code></th>
              {settings.map((s) => <td key={s.id}>{runsOf(entry, s.id).length ? row.get(s.id) ?? 0 : '–'}</td>)}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** One headline tile: a label, one row per setting or side, and where the figures come from. */
interface TileRow {
  key: string;
  marker: number | null;
  name: string;
  value: ReactNode;
}

function Tile({id, label, big, rows, note}: {id: string; label: string; big?: ReactNode; rows: TileRow[]; note: ReactNode}): ReactNode {
  return (
    <div className="sdc-tile" id={id}>
      <p className="sdc-tile__label">{label}</p>
      {big ? <p className="sdc-tile__big">{big}</p> : null}
      <ul className="sdc-tile__rows">
        {rows.map((r) => (
          <li key={r.key}>
            <span className="sdc-tile__name">{r.marker !== null ? <MarkerIcon i={r.marker} /> : null}{r.name}</span>
            <strong className="sdc-tile__value">{r.value}</strong>
          </li>
        ))}
      </ul>
      <p className="sdc-tile__note">{note}</p>
    </div>
  );
}

/** "n = 3 each" when every stat has the same n, else the n of each. */
function nNote(stats: Array<Stat | null>): string {
  const ns = Array.from(new Set(stats.filter((s): s is Stat => s !== null).map((s) => s.n)));
  return ns.length === 1 ? `n = ${ns[0]} runs each` : `n = ${ns.join(' / ')} runs`;
}

/** The conformance figure of one setting in one release: summed k and n over the measured runs. */
function conformanceOf(runs: LedgerRun[]): {k: number; n: number} | 'not observed' | 'not measured' | null {
  if (!runs.length) return null;
  const measured = runs.filter((r) => r.conformance?.measured && typeof r.conformance.k === 'number' && typeof r.conformance.n === 'number');
  if (!measured.length) return runs.some((r) => r.conformance) ? 'not measured' : 'not observed';
  return {k: measured.reduce((a, r) => a + r.conformance!.k!, 0), n: measured.reduce((a, r) => a + r.conformance!.n!, 0)};
}

const STATE: Record<string, string> = {complete: 'ended', abandoned: 'ended', running: 'running', planned: 'planned'};
const STATE_ORDER = ['running', 'planned', 'ended'];

export default function Evaluation({evaluation}: Props): ReactNode {
  const {progress, experiments, releasesTable, nodeClasses, fixtures, quality = [], targets = null, runs: runCosts = []} = evaluation;
  const freeRuns = (experiments as any[]).flatMap((e) => (e.treatment ? (e.variants ?? []).find((v: any) => v.id === e.treatment)?.runs ?? [] : []));
  const rel = releasesOf(progress);
  const settings = chartedSettings(progress);
  const labels = new Map(progress.settings.map((s) => [s.id, s.label]));
  const charted = rel.filter((e) => e.settings.length > 0);
  const noRuns = rel.filter((e) => e.settings.length === 0);
  const latest = charted[charted.length - 1] ?? null;
  const exp = (id: string) => experiments.find((e) => e.id === id) ?? null;
  const first = latest?.experiments[0];
  const caseExp = first ? exp(typeof first === 'string' ? first : first.id) : null;
  const idea = caseExp?.inputs.find((i) => i.kind === 'idea') ?? null;
  const seeds = exp('bugfix-seeds-1');
  const seedResult = progress.other_experiments.find((o) => o.id === 'bugfix-seeds-1') ?? null;
  const pre = progress.entries.filter((e) => e.kind === 'pre-release');
  const fixed = caseExp?.fixed ?? {};
  const totals = (e: LedgerEntry) => `${e.totals.clean_passes} of ${e.totals.ended}`;

  const latestSettings = latest ? settings.map((s, i) => ({s, i, runs: runsOf(latest, s.id)})).filter((x) => x.runs.length) : [];
  const costStats = latestSettings.map((x) => stat(x.runs.map(COST.get)));
  const wallStats = latestSettings.map((x) => stat(x.runs.map(WALL.get)));

  const target = targets?.targets.find((t) => t.metric === 'run.cost_usd' && typeof t.target === 'number') ?? null;
  const freeExp = target?.source_experiment ?? experiments.find((e) => e.variable === 'runner')?.id ?? null;
  const freeAgentRuns = runCosts.filter((r) => r.experiment === freeExp && r.format === 'sdc.free-agent-metrics/1');
  const freeCost = stat(freeAgentRuns.map((r) => r.cost_usd));
  const freeWall = stat(freeAgentRuns.map((r) => r.wall_s));
  const costRefs: RefLine[] = [
    ...(target ? [{value: target.target!, label: `target ${usd(target.target)} per run (${target.id})`, tag: 'target', kind: 'target' as const}] : []),
    ...(freeCost ? [{value: freeCost.median, label: `free agent median ${usd(freeCost.median)} (${freeExp}, n = ${freeCost.n})`, tag: 'free agent', kind: 'reference' as const}] : []),
  ];
  const wallRefs: RefLine[] = freeWall ? [{value: freeWall.median, label: `free agent median ${WALL.format(freeWall.median)} (${freeExp}, n = ${freeWall.n})`, tag: 'free agent', kind: 'reference'}] : [];

  const q = quality[0] ?? null;
  const qSides = q ? sideStats(q, freeRuns) : [];
  const qRuns = new Set(q?.products.map((p) => p.run) ?? []);
  const qRelease = q ? rel.find((e) => e.settings.some((c) => c.runs.some((r) => qRuns.has(r.run)))) ?? null : null;

  const confRelease = [...charted].reverse().find((e) => settings.some((s) => {
    const c = conformanceOf(runsOf(e, s.id));
    return c !== null && typeof c === 'object';
  })) ?? null;
  const confRows = confRelease ? settings.map((s, i) => ({s, i, c: conformanceOf(runsOf(confRelease, s.id))})).filter((x) => x.c !== null) : [];

  const byRun = new Map(runCosts.filter((r) => r.cost_by_class).map((r) => [r.run, r.cost_by_class!]));
  const splitRelease = [...charted].reverse().find((e) => e.settings.some((c) => c.runs.some((r) => byRun.has(r.run)))) ?? null;
  const splitRows: CostSplitRow[] = splitRelease
    ? settings
        .map((s, index) => {
          const parts = runsOf(splitRelease, s.id).map((r) => byRun.get(r.run)).filter((x): x is Record<string, number> => Boolean(x));
          const mean: Record<string, number> = {};
          for (const p of parts) for (const [k, v] of Object.entries(p)) mean[k] = (mean[k] ?? 0) + v / parts.length;
          return {setting: s, index, n: parts.length, mean, total: Object.values(mean).reduce((a, v) => a + v, 0)};
        })
        .filter((r) => r.n > 0)
    : [];

  const ledgerIds = new Set(progress.entries.flatMap((e) => e.experiments.map((x) => (typeof x === 'string' ? x : x.id))));
  const queue = experiments
    .filter((e) => e.id && !ledgerIds.has(e.id))
    .map((e) => ({e, state: STATE[e.status ?? ''] ?? e.status ?? '–'}))
    .sort((a, b) => {
      const d = (STATE_ORDER.indexOf(a.state) + 1 || 9) - (STATE_ORDER.indexOf(b.state) + 1 || 9);
      return d || a.e.id.localeCompare(b.e.id);
    });

  return (
    <Layout title="Evaluation" description="How the SDC performs per release: clean passes, cost and time per run, the improvement experiments, every run, and how it is measured.">
      <main className="container sdc-page sdc-eval">
        {fixtures ? <p className="sdc-banner" role="note" data-sdc-fixture-build="true">Fixture build: these figures come from <code>website/tests/fixtures</code>, not from the repository's ledger.</p> : null}
        <header className="sdc-head">
          <p className="b10x-kicker">Evaluation</p>
          <h1>How the cycle performs, release by release</h1>
          <p className="sdc-lede">
            Each release runs the same idea in each model setting, from the idea to a close or a stop, with nobody waited
            on; every figure on this page comes from the committed experiment records.
          </p>
          <nav className="sdc-jump" aria-label="On this page">
            <a href="#results">Results</a>
            <a href="#experiments">Experiments</a>
            <a href="#runs">Every run</a>
            <a href="#method">How it is measured</a>
            <a href="#limits">Limits</a>
          </nav>
        </header>

        {latest ? (
          <section className="sdc-tiles" aria-label="Headline figures" id="tiles">
            <Tile
              id="tile-clean"
              label="Clean passes"
              rows={latestSettings.map((x) => ({key: x.s.id, marker: x.i, name: x.s.id, value: `${x.runs.filter((r) => r.clean_pass).length} of ${x.runs.length}`}))}
              note={<>release {latest.label} · k of n ended runs</>}
            />
            <Tile
              id="tile-cost"
              label="Median cost per run"
              rows={latestSettings.map((x, j) => ({key: x.s.id, marker: x.i, name: x.s.id, value: costStats[j] ? usd(costStats[j]!.median) : '–'}))}
              note={<>release {latest.label} · USD · {nNote(costStats)}</>}
            />
            <Tile
              id="tile-wall"
              label="Median wall time per run"
              rows={latestSettings.map((x, j) => ({key: x.s.id, marker: x.i, name: x.s.id, value: wallStats[j] ? WALL.format(wallStats[j]!.median) : '–'}))}
              note={<>release {latest.label} · {nNote(wallStats)}</>}
            />
            {target ? (
              <Tile
                id="tile-target"
                label="Cost target per run"
                big={usd(target.target)}
                rows={latestSettings.map((x, j) => {
                  const s = costStats[j];
                  const f = s ? s.median / target.target! : null;
                  return {key: x.s.id, marker: x.i, name: x.s.id, value: f === null ? '–' : f > 1 ? `×${factor(f)} to go` : `met (×${f.toFixed(2)})`};
                })}
                note={<>{freeCost ? <>median of the free-running agent, <code>{freeExp}</code>, n = {freeCost.n}</> : <>set {target.set}</>} · factor against release {latest.label}</>}
              />
            ) : null}
            {q && qSides.some((s) => s.score) ? (
              <Tile
                id="tile-quality"
                label="Quality of the delivered product"
                rows={qSides.filter((s) => s.score).map((s) => ({key: s.side.id, marker: null, name: s.side.id, value: `${s.score!.median} of ${q.n}`}))}
                note={<>median rubric points, blinded · {q.products.length} products{qRelease ? <> · release {qRelease.label}</> : null}</>}
              />
            ) : null}
            {confRelease ? (
              <Tile
                id="tile-conformance"
                label="Spec scenarios proven passed"
                rows={confRows.map((x) => ({key: x.s.id, marker: x.i, name: x.s.id, value: typeof x.c === 'object' && x.c ? `${x.c.k} of ${x.c.n}` : String(x.c)}))}
                note={<>lower bound, summed over runs · release {confRelease.label}</>}
              />
            ) : null}
          </section>
        ) : null}

        <section aria-labelledby="results">
          <h2 id="results">Results</h2>
          <p>
            One column per release with committed runs. The dot is the median over the runs that have the value, the
            whisker spans min to max, and <code>n</code> counts those runs. Medians are over every ended run of the
            setting, clean or not. No line joins the releases.
          </p>
          <SeriesLegend settings={settings} />
          <div className="sdc-main-charts">
            <CleanPassChart releases={charted} settings={settings} />
            <MetricChart metric={COST} releases={charted} settings={settings} size="large" refs={costRefs} />
            <MetricChart metric={WALL} releases={charted} settings={settings} size="large" refs={wallRefs} />
            {splitRelease && splitRows.length ? <CostSplitChart release={splitRelease.label} rows={splitRows} /> : null}
          </div>
          {noRuns.length ? <p className="sdc-muted" id="no-runs">No runs yet on: {list(noRuns.map((e) => e.label))}.</p> : null}
          {target ? (
            <p className="sdc-muted">
              The cost target is <code>{target.id}</code> in <Cite path="experiments/targets.yaml" at="main" />, set {target.set}
              {freeCost ? <> from the median cost of the free-running agent (<code>{freeExp}</code>, n = {freeCost.n})</> : null}.
            </p>
          ) : null}
          <h3 id="more-metrics">More metrics</h3>
          <div className="sdc-metrics">
            {METRICS.filter((m) => !MAIN.has(m.id)).map((m) => <MetricChart key={m.id} metric={m} releases={charted} settings={settings} />)}
          </div>
          <h3 id="stops">Stops by cause</h3>
          <p>Ended runs that did not close, by the cause of their stop. Fewer is better.</p>
          {[...charted].reverse().map((e) => <StopsTable key={e.id} entry={e} settings={settings} />)}
        </section>

        <section aria-labelledby="experiments">
          <h2 id="experiments">Improvement experiments</h2>
          <p>
            Each record changes one thing against a baseline, or reads runs that exist. The release baselines are under
            Every run. Source: <code>experiments/&lt;id&gt;.yaml</code> and <Cite path="experiments/INDEX.md" at="main" />.
          </p>
          <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Improvement experiments">
            <table className="sdc-table sdc-table--experiments">
              <thead>
                <tr>
                  <th scope="col">Record</th>
                  <th scope="col">What is changed</th>
                  <th scope="col">Hypothesis or question</th>
                  <th scope="col">State</th>
                  <th scope="col">Verdict</th>
                </tr>
              </thead>
              <tbody>
                {queue.map(({e, state}) => (
                  <tr key={e.id} id={`exp-${e.id}`}>
                    <th scope="row"><code>{e.id}</code><span className="sdc-muted sdc-exp__kind">{e.kind}</span></th>
                    <td className="sdc-exp__what">
                      {e.title ?? '–'}
                      {e.variable ? <span className="sdc-muted sdc-exp__var"><code>{e.variable}</code>{e.treatment ? <> → <code>{e.treatment}</code></> : null}</span> : null}
                    </td>
                    <td className="sdc-exp__text">
                      {e.hypothesis ? <><span className="sdc-muted">Hypothesis: </span>{e.hypothesis}</> : e.purpose ? <><span className="sdc-muted">Question: </span>{e.purpose}</> : '–'}
                    </td>
                    <td><span className={`sdc-state sdc-state--${state}`}>{state}</span></td>
                    <td>{e.verdict ? <strong>{e.verdict}</strong> : <span className="sdc-muted">–</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section aria-labelledby="runs">
          <h2 id="runs">Every run</h2>
          <p>
            Per release, every run with its end and metrics, from the run summaries of the experiment records. "–" means
            the summary has no value. Loop is the looping factor, FPY the first-pass yield. Conformance is the line the
            observer printed for the run; Failed splits out the failures refused for a precondition the specification does
            not model.
          </p>
          {[...charted].reverse().map((e) => (
            <section key={e.id} aria-labelledby={`runs-${e.id}`}>
              <h3 id={`runs-${e.id}`}>
                Release {e.label} <span className="sdc-muted">· {e.date} · {totals(e)} clean passes</span>
              </h3>
              <RunTable entry={e} settings={progress.settings.map((s) => s.id)} labels={labels} />
              {e.limits?.length ? <ul className="sdc-limits">{e.limits.map((l) => <li key={l}>{l}</li>)}</ul> : null}
            </section>
          ))}
          {noRuns.length ? <p className="sdc-muted" id="runs-none">No runs yet on: {list(noRuns.map((e) => e.label))}.</p> : null}
          <h3 id="pre-releases">Before the first release: pre-release trees</h3>
          <p>
            Runs on trees that were never released, registered retrospectively. They are not on the charts and decide
            nothing; they show where the work started.
          </p>
          {pre.map((e) => (
            <details key={e.id} className="sdc-details">
              <summary>
                <strong>{e.label}</strong> <span className="sdc-muted">· {e.short} · {totals(e)} clean passes</span>
              </summary>
              <RunTable entry={e} settings={progress.settings.map((s) => s.id)} labels={labels} />
              {e.limits?.length ? <ul className="sdc-limits">{e.limits.map((l) => <li key={l}>{l}</li>)}</ul> : null}
            </details>
          ))}
        </section>

        <section aria-labelledby="method">
          <h2 id="method">How it is measured</h2>
          <h3 id="cases">The to-do case</h3>
          {idea ? (
            <blockquote className="sdc-idea">
              <p>{idea.text}</p>
            </blockquote>
          ) : null}
          <p>
            Input <code>{releasesTable?.input?.name ?? idea?.name}</code>
            {releasesTable?.input?.description ? <>: {releasesTable.input.description}</> : null}. Every release runs this
            idea text (sha256 <code>{releasesTable?.input?.sha256.slice(0, 12)}</code>), from the idea to a close or a
            stop.
          </p>
          {seeds && seedResult ? (
            <>
              <h3>The bug-fix seeds</h3>
              <p>
                <Link to="/protocols/sdc-bugfix"><code>sdc.bugfix</code></Link> fixes one bug, red test first. It ran once per
                seed on each model ({seeds.id}, a retrospective record, so it decides nothing). These runs are not in the
                release charts.
              </p>
              <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Bug-fix seeds">
                <table className="sdc-table">
                  <thead><tr><th scope="col">Seed</th><th scope="col">The bug as given</th></tr></thead>
                  <tbody>
                    {seeds.inputs.filter((i) => i.kind === 'seed').map((i) => (
                      <tr key={i.name}><th scope="row"><code>{i.name}</code></th><td>{i.text}</td></tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <p>
                Clean passes:{' '}
                {seedResult.variants.map((v: any, i: number) => (
                  <span key={v.id}>{i ? ', ' : ''}{v.id} {v.clean_passes} of {v.ended}</span>
                ))}
                .
              </p>
            </>
          ) : null}

          <h3 id="settings">The settings</h3>
          <p>A setting fixes which model each class of node runs on. The decider answers in place of a person.</p>
          <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Settings and models">
            <table className="sdc-table">
              <thead>
                <tr>
                  <th scope="col">Setting</th>
                  <th scope="col">writer</th>
                  <th scope="col">implement</th>
                  <th scope="col">review</th>
                  <th scope="col">other</th>
                  <th scope="col">decider</th>
                </tr>
              </thead>
              <tbody>
                {settings.map((s, i) => {
                  const v = caseExp?.variants.find((x) => x.id === s.id);
                  const models = classModels(v?.settings ?? s.match);
                  return (
                    <tr key={s.id}>
                      <th scope="row">
                        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><Marker i={i} x={6} y={6} r={3.5} /></svg> {s.label}
                      </th>
                      {(['writer', 'implement', 'review', 'other', 'decider'] as const).map((c) => <td key={c}>{shortModel(models[c])}</td>)}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <dl className="sdc-classes">
            {Object.entries(nodeClasses.classes).filter(([, nodes]) => nodes.length).map(([cls, nodes]) => (
              <div key={cls}>
                <dt><code>{cls}</code> <span className="sdc-muted">effort {nodeClasses.default_efforts[cls] ?? '–'}</span></dt>
                <dd>{nodes.map((n, i) => <span key={n}>{i ? ', ' : ''}<code>{n}</code></span>)}</dd>
              </div>
            ))}
          </dl>
          <p className="sdc-muted">
            Classes and default efforts: <Cite path="driver/examples/sdc/tools/node-classes.json" />. Models per setting:
            the experiment record <code>{caseExp?.id}</code>. The decider's default effort is{' '}
            {nodeClasses.default_efforts.decider}.
          </p>

          <h3>How a run is started</h3>
          <p>
            <code>sdc-experiment run &lt;id&gt;</code> starts the runs of a prospective record, each with{' '}
            <code>scripts/run-trial.sh --autopilot</code> (<Cite path="driver/src/bin/sdc-experiment.rs" lines={[1507]} />).
            Autopilot is autonomy level A5: the decider takes the scope answers and the findings, its decision is final,
            and a stop that needs a person ends the run with a stop record by <code>rule:autopilot</code>{' '}
            (<Cite path="docs/autonomy-levels.md" lines={[449]} />). Nobody is waited on.
            {fixed.max_llm_calls ? <> Budgets of the latest record: {fixed.max_llm_calls} model calls, {fixed['decider.max_decider_calls'] ?? '–'} decider calls, {Math.round(Number(fixed.max_wall_seconds) / 60)} minutes.</> : null}
          </p>
          <h3>A clean pass, and what stops a run</h3>
          <p>
            A clean pass is a run that closed, delivered, with no reopen, no forced step and no unstick (
            <code>{progress.pass_rule}</code>, <Cite path="docs/workflow-metrics.md" lines={[177, 178]} />). A run stops
            with a named verdict when nothing useful is left: <code>writer_spent</code>, <code>stuck</code>, a spent step,
            model-call or wall-time budget, or a wait for a person, which under autopilot ends the run (
            <Cite path="docs/driver.md" lines={[164, 172]} />). The third rejection of one approval stops it with{' '}
            <code>rework_spent</code> (<Cite path="docs/releases/0.1.0.md" lines={[9]} />).
          </p>
          <h3 id="conformance">Conformance of the delivered product</h3>
          <p>
            After a run closes, its delivered product is started and held to the specification that run itself wrote,
            through three route tables per product, each written by a separate model session of the node{' '}
            <code>conformance.table</code> and checked by a program against the specification and the delivered tree (
            <Cite path="docs/conformance.md" lines={[3, 5]} at="main" />, <Cite path="docs/conformance.md" lines={[37, 40]} at="main" />).
            The figure is a lower bound: a scenario counts as passed only when every table that reached it passed it,
            what no table reached stays outside k, and a run without an observation shows "not observed", never 0 (
            <Cite path="docs/conformance.md" lines={[23, 27]} at="main" />). Writing the tables costs about 1 to 3 USD
            of node cost per product (<Cite path="docs/conformance.md" lines={[18, 19]} at="main" />).
          </p>
          {quality.map((qq) => <QualityCase key={qq.experiment} q={qq} freeRuns={freeRuns} />)}
          <h3>n, and when a difference counts</h3>
          <p>
            At least three runs per setting on each release tree (<Cite path="docs/improving.md" lines={[11, 12]} />). A
            difference counts only by complete separation: the direction is written down before the runs, every treatment
            run lies beyond every baseline run in that direction, and the medians differ by at least the size the
            hypothesis names. At n = 3 the chance of that separation under exchangeability is 1 in 20, one-sided. A record
            written after its runs never decides (<Cite path="docs/workflow-metrics.md" lines={[143, 155]} />). The charts
            describe; they do not decide.
          </p>
        </section>

        <section aria-labelledby="limits">
          <h2 id="limits">Limits</h2>
          <ul className="sdc-limits">{progress.limits.map((l) => <li key={l}>{l}</li>)}</ul>
          <p className="sdc-muted">
            Source: <Cite path="docs/progress.json" />, written by <code>sdc-experiment progress</code> from{' '}
            <Cite path="experiments/releases.yaml" /> and the experiment records.
          </p>
        </section>
      </main>
    </Layout>
  );
}
