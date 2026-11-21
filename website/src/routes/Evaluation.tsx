import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import MetricChart, {CleanPassChart, Marker, SeriesLegend} from '../components/MetricChart';
import {
  METRICS,
  NODES,
  chartedSettings,
  classModels,
  releases as releasesOf,
  runsOf,
  shortModel,
  stopsByCause,
  type Ledger,
  type LedgerEntry,
  type LedgerRun,
  type LedgerSetting,
} from '../components/evaluation';
import {QUALITY_DOC, sideStats, type Quality} from '../components/Quality';

interface Experiment {
  id: string;
  kind: string;
  title: string | null;
  inputs: Array<{name: string; kind: string; text: string | null}>;
  fixed: Record<string, string>;
  variants: Array<{id: string; settings: Record<string, string>; runs: string[]}>;
  runs_per_variant: number | null;
  pass_rule: string | null;
}

interface Props {
  evaluation: {
    progress: Ledger;
    experiments: Experiment[];
    releasesTable: {input?: {name: string; description?: string; sha256: string}} | null;
    nodeClasses: {classes: Record<string, string[]>; default_efforts: Record<string, string>};
    quality?: Quality[];
    fixtures: boolean;
  };
}

const TAG = '0.1.1';

function Cite({path, lines, at = TAG}: {path: string; lines?: [number, number?]; at?: string}): ReactNode {
  const anchor = lines ? `#L${lines[0]}${lines[1] ? `-L${lines[1]}` : ''}` : '';
  const label = lines ? `${path}:${lines[0]}${lines[1] ? `-${lines[1]}` : ''}` : path;
  return (
    <a href={`https://github.com/beyond10x/sdc/blob/${at}/${path}${anchor}`}>
      <code>{label}</code>
    </a>
  );
}

const usd = (x: number | null | undefined) => (typeof x === 'number' ? `$${x.toFixed(2)}` : '–');
const mins = (x: number | null | undefined) => (typeof x === 'number' ? `${Math.round(x / 60)} min` : '–');
const num = (x: number | null | undefined, d = 0) => (typeof x === 'number' ? (d ? x.toFixed(d) : String(x)) : '–');
const pct = (x: number | null | undefined) => (typeof x === 'number' ? `${Math.round(x * 100)} %` : '–');

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
  const anyRuns = ids.some((s) => runsOf(entry, s).length);
  if (!anyRuns) return <p className="sdc-muted">{entry.label}: no data.</p>;
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

export default function Evaluation({evaluation}: Props): ReactNode {
  const {progress, experiments, releasesTable, nodeClasses, fixtures, quality = []} = evaluation;
  const freeRuns = (experiments as any[]).flatMap((e) => (e.treatment ? (e.variants ?? []).find((v: any) => v.id === e.treatment)?.runs ?? [] : []));
  const rel = releasesOf(progress);
  const settings = chartedSettings(progress);
  const labels = new Map(progress.settings.map((s) => [s.id, s.label]));
  const withData = rel.filter((e) => e.settings.length > 0);
  const latest = withData[withData.length - 1] ?? null;
  const exp = (id: string) => experiments.find((e) => e.id === id) ?? null;
  const first = latest?.experiments[0];
  const caseExp = first ? exp(typeof first === 'string' ? first : first.id) : null;
  const idea = caseExp?.inputs.find((i) => i.kind === 'idea') ?? null;
  const seeds = exp('bugfix-seeds-1');
  const seedResult = progress.other_experiments.find((o) => o.id === 'bugfix-seeds-1') ?? null;
  const pre = progress.entries.filter((e) => e.kind === 'pre-release');
  const fixed = caseExp?.fixed ?? {};
  const totals = (e: LedgerEntry) => `${e.totals.clean_passes} of ${e.totals.ended}`;
  return (
    <Layout title="Evaluation" description="The cases the SDC is measured on, every metric per release and setting, and every run.">
      <main className="container sdc-page sdc-eval">
        {fixtures ? <p className="sdc-banner" role="note" data-sdc-fixture-build="true">Fixture build: these figures come from <code>website/tests/fixtures</code>, not from the repository's ledger.</p> : null}
        <header className="sdc-head">
          <p className="b10x-kicker">Evaluation</p>
          <h1>How the cycle performs, release by release</h1>
          <p className="sdc-lede">
            Each release is run on the same case, three times per model setting, with nobody waited on. The charts start
            with the baseline of {rel[0]?.label ?? 'the first release'}; every later release adds a column when its runs
            are committed. A column without runs says "no data".
            {latest ? <> Latest release with runs: <strong>{latest.label}</strong>, {totals(latest)} clean passes.</> : null}
          </p>
        </header>

        <section aria-labelledby="cases">
          <h2 id="cases">The cases</h2>
          <h3>The to-do case</h3>
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

          <h3>The settings</h3>
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
          {quality.map((q) => <QualityCase key={q.experiment} q={q} freeRuns={freeRuns} />)}
          <h3>n, and when a difference counts</h3>
          <p>
            At least three runs per setting on each release tree (<Cite path="docs/improving.md" lines={[11, 12]} />). A
            difference counts only by complete separation: the direction is written down before the runs, every treatment
            run lies beyond every baseline run in that direction, and the medians differ by at least the size the
            hypothesis names. At n = 3 the chance of that separation under exchangeability is 1 in 20, one-sided. A record
            written after its runs never decides (<Cite path="docs/workflow-metrics.md" lines={[143, 155]} />). The charts
            below describe; they do not decide.
          </p>
        </section>

        <section aria-labelledby="metrics">
          <h2 id="metrics">Metrics per release</h2>
          <p>
            One chart per metric. The dot is the median over the runs that have the value, the whisker spans min to max,
            and <code>n</code> counts those runs. Medians are over every ended run of the setting, clean or not. No line
            joins the releases.
          </p>
          <SeriesLegend settings={settings} />
          <CleanPassChart releases={rel} settings={settings} />
          <div className="sdc-metrics">
            {METRICS.map((m) => <MetricChart key={m.id} metric={m} releases={rel} settings={settings} />)}
          </div>
          <h3 id="stops">Stops by cause</h3>
          <p>Ended runs that did not close, by the cause of their stop. Fewer is better.</p>
          {rel.map((e) => <StopsTable key={e.id} entry={e} settings={settings} />)}
        </section>

        <section aria-labelledby="runs">
          <h2 id="runs">Every run</h2>
          <p>
            Per release, every run with its end and metrics, from the run summaries of the experiment records. "–" means
            the summary has no value. Loop is the looping factor, FPY the first-pass yield. Conformance is the line the
            observer printed for the run; Failed splits out the failures refused for a precondition the specification does
            not model.
          </p>
          {[...rel].reverse().map((e) => (
            <section key={e.id} aria-labelledby={`runs-${e.id}`}>
              <h3 id={`runs-${e.id}`}>
                Release {e.label} <span className="sdc-muted">· {e.date}{e.settings.length ? ` · ${totals(e)} clean passes` : ''}</span>
              </h3>
              <RunTable entry={e} settings={progress.settings.map((s) => s.id)} labels={labels} />
              {e.limits?.length ? <ul className="sdc-limits">{e.limits.map((l) => <li key={l}>{l}</li>)}</ul> : null}
            </section>
          ))}
          <h3 id="pre-releases">Before 0.1.0: pre-release trees</h3>
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
