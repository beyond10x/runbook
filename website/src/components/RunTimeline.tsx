import {useMemo, useState, type KeyboardEvent, type ReactNode} from 'react';

/**
 * One run drawn from its `sdc.run-timeline/1` document: one lane per kind of step on a shared
 * time axis, the totals, and the plain list of steps. The program lane is Canon's decision acted
 * on by a bound program; the model lane is a model executing one action; the decider lane answers
 * in place of a person; the rule lane is a close by rule; idle is time no step owns.
 */

export interface Span {
  t0_s: number;
  t1_s: number;
  lane: 'program' | 'model' | 'decider' | 'rule' | 'idle' | 'tool';
  case: string | null;
  action: string | null;
  cause: any;
  exit?: string;
  evidence?: string[];
  principal?: string;
  model?: string;
  effort?: string;
  class?: string;
  adapter?: string;
  sessions?: number;
  api_requests?: number;
  tool_calls?: Record<string, number>;
  tool_calls_denied?: number;
  tokens?: {input: number; output: number; cache_read: number; cache_write: number};
  cost_usd?: number | null;
  not_called?: boolean | null;
  asked?: string;
  decision?: string;
  ttft_s?: number;
  is_error?: boolean;
}

export interface Timeline {
  format: string;
  run: string;
  kind: 'sdc' | 'free-agent';
  settings: any;
  end: {state: string; outcome: string | null; delivered: boolean | null};
  tree: Array<{case: string; parent: string | null; stage: string; outcome: string | null}> | null;
  totals: Record<string, any>;
  spans: Span[];
  notes: string[];
}

export const LANES: Array<{id: Span['lane']; label: string; note: string}> = [
  {id: 'program', label: 'Program', note: 'a bound program takes the action Canon admitted'},
  {id: 'model', label: 'Model', note: 'a model executes one action'},
  {id: 'decider', label: 'Decider', note: 'a model answers in place of a person'},
  {id: 'tool', label: 'Tool', note: 'a tool the agent called'},
  {id: 'rule', label: 'Rule', note: 'a close by rule'},
  {id: 'idle', label: 'Idle', note: 'no step owns the time'},
];

export const usd = (x: number | null | undefined) => (typeof x === 'number' ? `$${x < 0.1 ? x.toFixed(3) : x.toFixed(2)}` : '–');
export const dur = (s: number) => (s === 0 ? '0 s' : s >= 60 ? `${Math.floor(s / 60)} min ${Math.round(s % 60)} s` : s >= 10 ? `${Math.round(s)} s` : `${s.toFixed(s < 1 ? 2 : 1)} s`);
export const kilo = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(2)} M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)} k` : String(n));

function tickStep(total: number): number {
  for (const s of [30, 60, 120, 300, 600, 900, 1800, 3600]) if (total / s <= 8) return s;
  return 7200;
}

export function lanesOf(t: Timeline): typeof LANES {
  return LANES.filter((l) => t.spans.some((s) => s.lane === l.id));
}

const W = 1000;
const ROW = 38;

/** Count and time of one lane, from the totals where the document has them, else from the spans. */
export function laneSummary(t: Timeline, lane: Span['lane']): string {
  const spans = t.spans.filter((s) => s.lane === lane);
  const sum = spans.reduce((a, s) => a + (s.t1_s - s.t0_s), 0);
  const x = t.totals;
  const time = (k: string) => (typeof x[k] === 'number' ? x[k] : sum);
  const n = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;
  switch (lane) {
    case 'program':
      return `${n(x.program_steps ?? spans.length, 'step', 'steps')} · ${dur(time('program_s'))}`;
    case 'model': {
      const called = spans.filter((s) => !s.not_called).length;
      return t.kind === 'free-agent' ? `${n(spans.length, 'request', 'requests')} · ${dur(time('model_s'))}` : `${n(called, 'call', 'calls')} · ${dur(time('model_s'))}`;
    }
    case 'decider':
      return `${n(x.decider_calls ?? spans.length, 'call', 'calls')} · ${dur(time('decider_s'))}`;
    case 'rule':
      return `${n(x.rule_steps ?? spans.length, 'step', 'steps')} · ${dur(time('rule_s'))}`;
    case 'tool':
      return `${n(spans.length, 'call', 'calls')} · ${dur(time('tool_s'))}`;
    default:
      return `${n(spans.length, 'gap', 'gaps')} · ${dur(time('idle_s'))}`;
  }
}

/** The lanes of one run; `axis` is the shared time axis in seconds when two runs are compared. */
export function Lanes({t, axis, selected, onSelect, compact}: {t: Timeline; axis?: number; selected?: number | null; onSelect?: (i: number | null) => void; compact?: boolean}): ReactNode {
  const total = axis ?? t.totals.wall_s;
  const lanes = lanesOf(t);
  const x = (s: number) => (s / total) * W;
  const height = lanes.length * ROW + 22;
  const step = tickStep(total);
  const ticks: number[] = [];
  for (let s = 0; s <= total + 0.001; s += step) ticks.push(s);
  const key = (ev: KeyboardEvent, i: number) => {
    if (ev.key === 'Enter' || ev.key === ' ') {
      ev.preventDefault();
      onSelect?.(selected === i ? null : i);
    } else if (ev.key === 'Escape') onSelect?.(null);
  };
  return (
    <div className={`sdc-tl${compact ? ' sdc-tl--compact' : ''}`}>
      <ul className="sdc-tl__labels" aria-label="Lanes" style={{paddingBottom: 22}}>
        {lanes.map((l) => (
          <li key={l.id} style={{height: ROW}} data-lane={l.id}>
            <strong>{l.label}</strong>
            <span>{laneSummary(t, l.id)}</span>
          </li>
        ))}
      </ul>
      <svg className="sdc-tl__svg" viewBox={`0 0 ${W} ${height}`} preserveAspectRatio="none" style={{height}} role="group" aria-label={`Timeline of ${t.run}: ${lanes.map((l) => l.label).join(', ')} lanes over ${dur(t.totals.wall_s)}`}>
        {ticks.map((s) => (
          <g key={s}>
            <line x1={x(s)} x2={x(s)} y1={0} y2={lanes.length * ROW} className="sdc-tl__grid" />
          </g>
        ))}
        {lanes.map((l, li) => <line key={l.id} x1={0} x2={W} y1={(li + 1) * ROW} y2={(li + 1) * ROW} className="sdc-tl__grid" />)}
        {t.spans.map((s, i) => {
          const li = lanes.findIndex((l) => l.id === s.lane);
          if (li < 0) return null;
          const w = Math.max(s.lane === 'rule' ? 3 : 1.2, x(s.t1_s) - x(s.t0_s));
          const focusable = !compact && (s.lane === 'model' || s.lane === 'decider') && !s.not_called;
          const cls = `sdc-tl__span sdc-tl__span--${s.lane}${selected === i ? ' is-selected' : ''}${s.not_called ? ' is-not-called' : ''}`;
          return (
            <rect
              key={i}
              x={Math.min(x(s.t0_s), W - w)}
              y={li * ROW + 5}
              width={w}
              height={ROW - 10}
              className={cls}
              {...(focusable
                ? {tabIndex: 0, role: 'button', 'aria-pressed': selected === i, 'aria-label': `${s.lane} ${s.action ?? ''} in ${s.case ?? ''}, ${dur(s.t1_s - s.t0_s)}, ${usd(s.cost_usd)}`, onKeyDown: (ev: KeyboardEvent) => key(ev, i)}
                : {})}
              onClick={onSelect ? () => onSelect(selected === i ? null : i) : undefined}
            >
              <title>{`${s.action ?? s.cause ?? s.lane} · ${s.case ?? ''} · ${dur(s.t1_s - s.t0_s)}${typeof s.cost_usd === 'number' ? ` · ${usd(s.cost_usd)}` : ''}`}</title>
            </rect>
          );
        })}
      </svg>
      <div className="sdc-tl__axis" aria-hidden="true">
        {ticks.map((s) => <span key={s} style={{left: `${(s / total) * 100}%`}}>{s === 0 ? '0' : `${Math.round(s / 60)} min`}</span>)}
      </div>
    </div>
  );
}

function causeText(c: any): string {
  if (!c) return 'no recorded cause';
  if (typeof c === 'string') return c;
  const claims = (c.claims ?? []).map((k: any) => `${k.claim} = ${k.value}`).join(', ');
  return `decision ${c.decision_seq ?? '?'}${claims ? ` read ${claims}` : ''}${c.retry_of ? `, retry of ${c.retry_of}` : ''}`;
}

export function SpanDetail({s}: {s: Span | null}): ReactNode {
  if (!s) return <p className="sdc-muted">Select a model or decider span, or a step below, to see its figures.</p>;
  const tools = s.tool_calls ? Object.entries(s.tool_calls).map(([k, v]) => `${k} ${v}`).join(', ') : '';
  return (
    <dl className="sdc-kv sdc-tl__detail">
      <dt>Step</dt>
      <dd><code>{s.action ?? s.lane}</code> in <code>{s.case ?? '–'}</code>, {s.lane}</dd>
      <dt>Time</dt>
      <dd>{dur(s.t0_s)} to {dur(s.t1_s)} ({dur(s.t1_s - s.t0_s)})</dd>
      {s.model ? (<><dt>Model</dt><dd><code>{s.model}</code>{s.effort ? `, effort ${s.effort}` : ''}{s.class ? `, class ${s.class}` : ''}{s.adapter ? `, ${s.adapter}` : ''}</dd></>) : null}
      {s.not_called ? (<><dt>Call</dt><dd>not called: the node exited without calling the model and said why; such a take does not count toward its runs</dd></>) : null}
      {typeof s.api_requests === 'number' ? (<><dt>API requests</dt><dd>{s.api_requests}</dd></>) : null}
      {tools ? (<><dt>Tool calls</dt><dd>{tools}{s.tool_calls_denied ? `; ${s.tool_calls_denied} denied` : ''}</dd></>) : null}
      {s.tokens ? (<><dt>Tokens</dt><dd>input {kilo(s.tokens.input)}, output {kilo(s.tokens.output)}, cache read {kilo(s.tokens.cache_read)}, cache write {kilo(s.tokens.cache_write)}</dd></>) : null}
      {s.cost_usd !== undefined ? (<><dt>Cost</dt><dd>{usd(s.cost_usd)}</dd></>) : null}
      {s.asked ? (<><dt>Asked</dt><dd><code>{s.asked}</code> → {s.decision}</dd></>) : null}
      {s.lane === 'program' || s.lane === 'model' ? (<><dt>Cause</dt><dd>{causeText(s.cause)}</dd></>) : null}
      {s.exit !== undefined ? (<><dt>Exit</dt><dd>{s.exit}</dd></>) : null}
      {s.principal ? (<><dt>By</dt><dd><code>{s.principal}</code></dd></>) : null}
    </dl>
  );
}

export function Totals({t}: {t: Timeline}): ReactNode {
  const x = t.totals;
  const tok = x.tokens ?? {};
  const items: Array<[string, string]> = [
    ['Wall time', dur(x.wall_s)],
    ['Cost', usd(x.cost_usd)],
    ['Model time', dur(x.model_s ?? 0)],
    ['Decider time', dur(x.decider_s ?? 0)],
    ['Program time', dur(x.program_s ?? 0)],
    ['Idle', dur(x.idle_s ?? 0)],
    ['Program steps', String(x.program_steps ?? '–')],
    ['Model sessions', String(x.model_sessions ?? '–')],
    ['API requests', String(x.api_requests ?? '–')],
    ['Model tool calls', String(x.model_tool_calls ?? '–')],
    ['Decider calls', String(x.decider_calls ?? '–')],
    ['Output tokens', kilo(tok.output ?? 0)],
    ['Waiting on a person', dur(x.waiting_on_person_s ?? 0)],
  ];
  return (
    <dl className="sdc-totals" aria-label={`Totals of ${t.run}`}>
      {items.map(([k, v]) => (
        <div key={k}><dt>{k}</dt><dd>{v}</dd></div>
      ))}
    </dl>
  );
}

export function StepList({t, selected, onSelect}: {t: Timeline; selected: number | null; onSelect: (i: number | null) => void}): ReactNode {
  const [lane, setLane] = useState<string>('all');
  const rows = useMemo(() => t.spans.map((s, i) => ({s, i})).filter(({s}) => s.lane !== 'idle'), [t]);
  const shown = rows.filter(({s}) => lane === 'all' || s.lane === lane);
  const lanes = lanesOf(t).filter((l) => l.id !== 'idle');
  return (
    <div className="sdc-steps">
      <div className="sdc-filter__chips" role="group" aria-label="Show steps of one lane">
        {[{id: 'all', label: 'All'}, ...lanes].map((l) => (
          <button key={l.id} type="button" className="sdc-chip sdc-chip--toggle" aria-pressed={lane === l.id} onClick={() => setLane(l.id)}>
            {l.label} <span className="sdc-muted">{l.id === 'all' ? rows.length : rows.filter(({s}) => s.lane === l.id).length}</span>
          </button>
        ))}
      </div>
      <div className="sdc-table-wrap sdc-steps__scroll" tabIndex={0} role="region" aria-label={`Steps of ${t.run}`}>
        <table className="sdc-table">
          <thead>
            <tr><th scope="col">At</th><th scope="col">Action</th><th scope="col">Case</th><th scope="col">Lane</th><th scope="col">Duration</th><th scope="col">Cost</th></tr>
          </thead>
          <tbody>
            {shown.map(({s, i}) => (
              <tr key={i} className={selected === i ? 'is-selected' : undefined}>
                <td>{dur(s.t0_s)}</td>
                <td><button type="button" className="sdc-linkbutton" onClick={() => onSelect(selected === i ? null : i)} aria-pressed={selected === i}><code>{s.action ?? s.lane}</code></button></td>
                <td><code>{s.case ?? '–'}</code></td>
                <td>{s.lane}{s.not_called ? ' (not called)' : ''}</td>
                <td>{dur(s.t1_s - s.t0_s)}</td>
                <td>{typeof s.cost_usd === 'number' ? usd(s.cost_usd) : '–'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
