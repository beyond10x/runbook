import {useState, type ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import {Lanes, SpanDetail, StepList, Totals, dur, usd, LANES, type Timeline} from '../components/RunTimeline';
import {stat} from '../components/evaluation';
import FreeAgentComparison, {ResultLede, comparisonModel, type ConfRow, type RunRow, type SizeRow, type Tl} from '../components/FreeAgentComparison';
import type {Quality} from '../components/Quality';

interface Entry {
  experiment: string;
  file: string;
  doc: Timeline;
}

interface Props {
  timelines: {timelines: Entry[]; experiments: any[]; runs?: RunRow[]; conformance?: ConfRow[]; sizes?: SizeRow[]; quality?: Quality[]; fixtures: boolean};
}

const CANON = 'https://github.com/beyond10x/canon/blob/8f1f4527439b813f4c0c0159c8b4ea7cd4131994';
const SDC = 'https://github.com/beyond10x/sdc/blob/0.1.1';

function Cite({base, path, lines, label}: {base: string; path: string; lines: string; label: string}): ReactNode {
  const [a, b] = lines.split('-');
  return (
    <a href={`${base}/${path}#L${a}${b ? `-L${b}` : ''}`}>
      <code>{label}:{lines}</code>
    </a>
  );
}

function Medians({sdc, free}: {sdc: Timeline[]; free: Timeline[]}): ReactNode {
  const rows: Array<[string, (t: Timeline) => number | null, (x: number) => string]> = [
    ['Wall time', (t) => t.totals.wall_s, dur],
    ['Cost', (t) => t.totals.cost_usd ?? null, (x) => usd(x)],
    ['API requests', (t) => t.totals.api_requests ?? null, (x) => String(Math.round(x))],
    ['Model tool calls', (t) => t.totals.model_tool_calls ?? null, (x) => String(Math.round(x))],
    ['Model time', (t) => t.totals.model_s ?? null, dur],
    ['Idle', (t) => t.totals.idle_s ?? null, dur],
  ];
  const cell = (ts: Timeline[], f: (t: Timeline) => number | null, fmt: (x: number) => string) => {
    const s = stat(ts.map(f));
    return s ? `${fmt(s.median)} [${fmt(s.min)}, ${fmt(s.max)}] (n=${s.n})` : 'no data';
  };
  const delivered = (ts: Timeline[]) => `${ts.filter((t) => t.end.delivered).length} of ${ts.length}`;
  return (
    <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Medians, SDC against a free-running agent">
      <table className="sdc-table">
        <thead>
          <tr><th scope="col">Median [min, max] (n)</th><th scope="col">SDC</th><th scope="col">Free-running agent</th></tr>
        </thead>
        <tbody>
          <tr><th scope="row">Delivered</th><td>{delivered(sdc)}</td><td>{delivered(free)}</td></tr>
          {rows.map(([k, f, fmt]) => (
            <tr key={k}><th scope="row">{k}</th><td>{cell(sdc, f, fmt)}</td><td>{cell(free, f, fmt)}</td></tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function CostBar({t, max}: {t: Timeline; max: number}): ReactNode {
  const c = t.totals.cost_usd ?? 0;
  return (
    <div className="sdc-costbar" aria-label={`Cost ${usd(c)} on a scale to ${usd(max)}`} role="img">
      <span className="sdc-costbar__fill" style={{width: `${max ? (c / max) * 100 : 0}%`}} />
      <span className="sdc-costbar__label">{usd(c)}</span>
    </div>
  );
}

function Compare({sdc, free, experiments}: {sdc: Timeline[]; free: Timeline[]; experiments: any[]}): ReactNode {
  if (!free.length) {
    return (
      <p className="sdc-banner" role="note">
        Runs pending: no free-running agent timeline is committed yet. When the runs are in{' '}
        <code>experiments/&lt;id&gt;/timelines/</code>, they are drawn here under the SDC run, on the same time axis and
        cost scale, with the medians and the recorded hypothesis and verdict.
      </p>
    );
  }
  const a = sdc[0];
  const b = free[0];
  const axis = Math.max(a.totals.wall_s, b.totals.wall_s);
  const cmax = Math.max(a.totals.cost_usd ?? 0, b.totals.cost_usd ?? 0);
  const ids = new Set(free.map((t) => (t as any).__experiment));
  const record = experiments.find((e) => ids.has(e.id) && (e.hypothesis || e.verdict)) ?? null;
  return (
    <>
      <p>
        The same idea, run once by the SDC (top) and once by one agent given the idea and left to run (bottom). Both use
        one time axis of {dur(axis)} and one cost scale to {usd(cmax)}.
      </p>
      <div className="sdc-compare">
        <h4>SDC · <code>{a.run}</code></h4>
        <div className="sdc-tl-viewport" tabIndex={0} role="region" aria-label="SDC run on the shared axis"><Lanes t={a} axis={axis} compact /></div>
        <CostBar t={a} max={cmax} />
        <h4>Free-running agent · <code>{b.run}</code> <span className="sdc-muted">{b.settings?.model}</span></h4>
        <div className="sdc-tl-viewport" tabIndex={0} role="region" aria-label="Free-running agent on the shared axis"><Lanes t={b} axis={axis} compact /></div>
        <CostBar t={b} max={cmax} />
      </div>
      <Medians sdc={sdc} free={free} />
      {record ? (
        <dl className="sdc-kv">
          <dt>Record</dt><dd><code>{record.id}</code></dd>
          <dt>Hypothesis</dt><dd>{record.hypothesis ?? 'none recorded'}</dd>
          <dt>Verdict</dt><dd>{record.verdict ?? 'not computed'}{record.why ? `: ${record.why}` : ''}</dd>
        </dl>
      ) : <p className="sdc-muted">No hypothesis or verdict is recorded for these runs yet.</p>}
    </>
  );
}

export default function LlmIsNotTheRuntime({timelines}: Props): ReactNode {
  const all = timelines.timelines.map((e) => Object.assign({}, e.doc, {__experiment: e.experiment, __file: e.file}));
  const sdc = all.filter((t) => t.kind === 'sdc' && t.end?.delivered);
  const free = all.filter((t) => t.kind === 'free-agent');
  const model = comparisonModel(all as Tl[], timelines.experiments, timelines.runs ?? [], timelines.conformance ?? [], timelines.sizes ?? []);
  const [pick, setPick] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const t = sdc[pick] ?? null;
  return (
    <Layout title="The LLM is not the runtime" description="Canon decides what is legitimate; a program takes the step; a model executes one action. One run, step by step.">
      <main className="container sdc-page sdc-llm">
        {timelines.fixtures ? <p className="sdc-banner" role="note" data-sdc-fixture-build="true">Fixture build: the runs below come from <code>website/tests/fixtures</code>.</p> : null}
        <header className="sdc-head">
          <p className="b10x-kicker">Principle</p>
          <h1>The LLM is not the runtime</h1>
          {model ? <ResultLede m={model} quality={timelines.quality} /> : null}
          <p className="sdc-lede">
            Canon says what is legitimate, a program takes the step, and a model is called only to execute one action. Below
            is the principle as Canon states it, how the SDC applies it, one run drawn step by step, and the comparison with a free-running agent.
          </p>
        </header>

        <section aria-labelledby="canon">
          <h2 id="canon">As Canon states it</h2>
          <blockquote>
            <p>A workflow answers "what executes next?". Canon answers "what is currently legitimate?".</p>
            <footer><Cite base={CANON} path="website/docs/index.md" lines="39" label="canon/website/docs/index.md" /></footer>
          </blockquote>
          <blockquote>
            <p>Canon does not execute anything. It is not a workflow engine, planner, scheduler, issue tracker, database, agent harness, authorization server or CI runner.</p>
            <footer><Cite base={CANON} path="website/docs/index.md" lines="56-57" label="canon/website/docs/index.md" /></footer>
          </blockquote>
          <blockquote>
            <p>Canon does not execute reasoning or tools.</p>
            <footer><Cite base={CANON} path="docs/design/canon-protocol-calculus-design.md" lines="1153" label="canon/docs/design/canon-protocol-calculus-design.md" /></footer>
          </blockquote>
        </section>

        <section aria-labelledby="sdc">
          <h2 id="sdc">How the SDC applies it</h2>
          <blockquote>
            <p>
              The driver reads the <code>canon-decision/1</code>, runs the tool bound to one admissible action, turns the
              tool's result into a <code>canon-evidence/1</code> record, and evaluates again. An LLM is only ever one
              action's executor. Its output becomes an artifact revision, never evidence and never the next state.
            </p>
            <footer><Cite base={SDC} path="docs/driver.md" lines="3-6" label="docs/driver.md" /></footer>
          </blockquote>
          <p>
            So a run has a few kinds of step, and the timeline gives each its own lane. Which action is admissible and which
            outcome is legitimate is decided by Canon from the protocol; the <Link to="/protocols">protocol browser</Link>{' '}
            shows each protocol's actions and what they wait on.
          </p>
          <ul className="sdc-lanes-key">
            {LANES.filter((l) => l.id !== 'tool').map((l) => (
              <li key={l.id}><span className={`sdc-lanes-key__sw sdc-tl__span--${l.id}`} aria-hidden="true" /> <strong>{l.label}</strong>: {l.note}.</li>
            ))}
          </ul>
        </section>

        <section aria-labelledby="run">
          <h2 id="run">One successful run{t ? <>, from <code>{(t as any).__experiment}</code></> : null}</h2>
          {!t ? (
            <p className="sdc-banner" role="note">Timeline pending: no <code>sdc.run-timeline/1</code> document of a delivered run is committed yet.</p>
          ) : (
            <>
              {sdc.length > 1 ? (
                <div className="sdc-filter__chips" role="group" aria-label="Run">
                  {sdc.map((x, i) => (
                    <button key={x.run} type="button" className="sdc-chip sdc-chip--toggle" aria-pressed={pick === i} onClick={() => { setPick(i); setSelected(null); }}>
                      <code>{x.run}</code>
                    </button>
                  ))}
                </div>
              ) : null}
              <p>
                <code>{t.run}</code>: writer {t.settings?.models?.writer}, implement {t.settings?.models?.implement}, review{' '}
                {t.settings?.models?.review}, other nodes {t.settings?.models?.other}, decider {t.settings?.decider?.model},
                autonomy {t.settings?.autonomy}. It ended <code>{t.end.state}</code> with <code>{t.end.outcome}</code>, delivered, over{' '}
                {t.tree?.length ?? 0} cases. Source: <code>{(t as any).__file}</code>.
              </p>
              <Totals t={t} />
              <div className="sdc-tl-viewport" tabIndex={0} role="region" aria-label="Timeline; scroll sideways on a narrow screen">
                <Lanes t={t} selected={selected} onSelect={setSelected} />
              </div>
              <div className="sdc-tl__panel" aria-live="polite">
                <SpanDetail s={selected !== null ? t.spans[selected] : null} />
              </div>
              {t.notes?.length ? <ul className="sdc-limits">{t.notes.map((n) => <li key={n}>{n}</li>)}</ul> : null}
              <h3 id="steps">Every step</h3>
              <StepList t={t} selected={selected} onSelect={setSelected} />
            </>
          )}
        </section>

        <section aria-labelledby="free">
          <h2 id="free">Against a free-running agent</h2>
          {model ? <FreeAgentComparison m={model} quality={timelines.quality} /> : <Compare sdc={sdc} free={free} experiments={timelines.experiments} />}
        </section>
      </main>
    </Layout>
  );
}
