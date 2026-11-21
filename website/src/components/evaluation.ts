/**
 * The figures of the evaluation page, computed from `docs/progress.json` (`sdc.progress/1`) alone.
 * A value a run summary lacks is null and is left out of a median, never read as 0.
 */

export interface RunMetrics {
  llm_calls: number | null;
  decider_calls: number | null;
  cost_usd: number | null;
  wall_s: number | null;
  rework_takes: number | null;
  looping_factor: number | null;
  first_pass_yield: number | null;
  waiting_on_person_s: number | null;
  stop_cause: string | null;
  end_kind: string | null;
  outcome: string | null;
  closed: boolean | null;
  delivered: boolean | null;
  forced: number | null;
  unstuck: number | null;
  reopens: number | null;
  merge_rejections: number | null;
  review_rejections: number | null;
  nodes: Record<string, NodeMetrics | null> | null;
}

export interface NodeMetrics {
  instances: number | null;
  real_takes: number | null;
  rework_takes: number | null;
  looping_factor: number | null;
  first_pass_yield: number | null;
}

export interface LedgerRun {
  run: string;
  clean_pass: boolean;
  end: string;
  cost_usd: number | null;
  wall_s: number | null;
  metrics?: RunMetrics | null;
}

export interface LedgerCell {
  experiment: string;
  variant: string;
  setting: string | null;
  clean_passes: number;
  ended: number;
  not_ended: number;
  not_ended_runs: string[];
  runs: LedgerRun[];
}

export interface LedgerEntry {
  id: string;
  kind: 'release' | 'pre-release';
  version: string | null;
  label: string;
  short?: string;
  date: string;
  experiments: Array<string | {id: string}>;
  settings: LedgerCell[];
  limits?: string[];
  totals: {clean_passes: number; ended: number; not_ended: number};
}

export interface LedgerSetting {
  id: string;
  label: string;
  note?: string;
  match: Record<string, string>;
}

export interface Ledger {
  format: string;
  pass_rule: string;
  settings: LedgerSetting[];
  entries: LedgerEntry[];
  limits: string[];
  other_experiments: any[];
}

export const NODES = ['spec.author', 'plan.decompose', 'change.implement', 'change.review'] as const;

export interface Stat {
  n: number;
  median: number;
  min: number;
  max: number;
}

export function median(sorted: number[]): number {
  const m = sorted.length >> 1;
  return sorted.length % 2 ? sorted[m] : (sorted[m - 1] + sorted[m]) / 2;
}

/** Median, min and max of the values that exist; null when none does. */
export function stat(values: Array<number | null | undefined>): Stat | null {
  const v = values.filter((x): x is number => typeof x === 'number' && Number.isFinite(x)).sort((a, b) => a - b);
  if (!v.length) return null;
  return {n: v.length, median: median(v), min: v[0], max: v[v.length - 1]};
}

function versionKey(v: string): number[] {
  return v.split('.').map((p) => Number.parseInt(p, 10) || 0);
}

/** The releases in version order, 0.1.0 first; pre-release trees are not on the x axis. */
export function releases(ledger: Ledger): LedgerEntry[] {
  return ledger.entries
    .filter((e) => e.kind === 'release')
    .sort((a, b) => {
      const x = versionKey(a.version ?? a.id);
      const y = versionKey(b.version ?? b.id);
      for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0);
      return 0;
    });
}

/** The runs of one setting in one entry, over every cell of that setting. */
export function runsOf(entry: LedgerEntry, setting: string): LedgerRun[] {
  return entry.settings.filter((c) => c.setting === setting).flatMap((c) => c.runs);
}

export function cellsOf(entry: LedgerEntry, setting: string): LedgerCell[] {
  return entry.settings.filter((c) => c.setting === setting);
}

/** The settings that have runs in at least one release, in the ledger's order. */
export function chartedSettings(ledger: Ledger): LedgerSetting[] {
  const rel = releases(ledger);
  return ledger.settings.filter((s) => rel.some((e) => cellsOf(e, s.id).length > 0));
}

export type Getter = (r: LedgerRun) => number | null | undefined;

export interface MetricDef {
  id: string;
  title: string;
  unit: string;
  better: 'lower' | 'higher';
  direction: string;
  get: Getter;
  format: (x: number) => string;
}

const usd = (x: number) => `$${x.toFixed(2)}`;
const minutes = (x: number) => (x >= 3600 ? `${(x / 3600).toFixed(1)} h` : x >= 60 || x === 0 ? `${Math.round(x / 60)} min` : `${Math.round(x)} s`);
const int = (x: number) => (Number.isInteger(x) ? String(x) : x.toFixed(1));
const ratio = (x: number) => x.toFixed(2);
const share = (x: number) => `${Math.round(x * 100)} %`;

const m = (r: LedgerRun) => r.metrics ?? null;
const node = (r: LedgerRun, n: string) => m(r)?.nodes?.[n] ?? null;

export const METRICS: MetricDef[] = [
  {id: 'cost', title: 'Cost per run', unit: 'USD', better: 'lower', direction: 'Lower is better.', get: (r) => m(r)?.cost_usd ?? r.cost_usd, format: usd},
  {id: 'wall', title: 'Wall time per run', unit: 'time', better: 'lower', direction: 'Lower is better.', get: (r) => m(r)?.wall_s ?? r.wall_s, format: minutes},
  {id: 'llm', title: 'Model calls per run', unit: 'calls', better: 'lower', direction: 'Lower is better: fewer calls for the same end.', get: (r) => m(r)?.llm_calls, format: int},
  {id: 'decider', title: 'Decider calls per run', unit: 'calls', better: 'lower', direction: 'Lower is better: fewer questions the cycle could not settle itself.', get: (r) => m(r)?.decider_calls, format: int},
  {id: 'rework', title: 'Rework takes per run', unit: 'takes', better: 'lower', direction: 'Lower is better: a rework take is a step run again after a rejection or a failed check.', get: (r) => m(r)?.rework_takes, format: int},
  {id: 'review', title: 'Review rejections per run', unit: 'rejections', better: 'lower', direction: 'Lower is better: change.review sent a change back.', get: (r) => m(r)?.review_rejections, format: int},
  {id: 'merge', title: 'Merge rejections per run', unit: 'rejections', better: 'lower', direction: 'Lower is better: merge.approve sent a change back.', get: (r) => m(r)?.merge_rejections, format: int},
  {id: 'waiting', title: 'Waiting on a person per run', unit: 'time', better: 'lower', direction: 'Lower is better; under autopilot it should stay 0.', get: (r) => m(r)?.waiting_on_person_s, format: minutes},
  ...NODES.map<MetricDef>((n) => ({
    id: `loop-${n}`,
    title: `Looping factor of ${n}`,
    unit: 'takes per instance',
    better: 'lower',
    direction: 'Lower is better; 1.00 means every instance of the node ran once.',
    get: (r) => node(r, n)?.looping_factor,
    format: ratio,
  })),
  ...NODES.map<MetricDef>((n) => ({
    id: `fpy-${n}`,
    title: `First-pass yield of ${n}`,
    unit: 'share',
    better: 'higher',
    direction: 'Higher is better; 100 % means no instance of the node needed a second take.',
    get: (r) => node(r, n)?.first_pass_yield,
    format: share,
  })),
];

/** Stops by cause in one entry: cause → setting → count. A run that closed is not a stop. */
export function stopsByCause(entry: LedgerEntry, settings: string[]): Map<string, Map<string, number>> {
  const out = new Map<string, Map<string, number>>();
  for (const s of settings) {
    for (const r of runsOf(entry, s)) {
      const mm = m(r);
      const closed = mm?.closed ?? r.end.startsWith('closed');
      if (closed) continue;
      const cause = mm?.stop_cause ?? mm?.end_kind ?? r.end.split(/[:\s]/)[0] ?? 'unknown';
      const row = out.get(cause) ?? new Map<string, number>();
      row.set(s, (row.get(s) ?? 0) + 1);
      out.set(cause, row);
    }
  }
  return out;
}

/** Which model each class of node ran on, from one experiment variant's settings. */
export function classModels(settings: Record<string, string>): Record<string, string | null> {
  const get = (k: string) => settings[k] ?? null;
  return {
    writer: get('models.writer.model'),
    implement: get('models.implement.model'),
    review: get('models.review.model'),
    other: get('models.other.model'),
    decider: get('decider.model'),
  };
}

export function shortModel(m: string | null): string {
  if (!m) return 'not recorded';
  const x = /claude-([a-z]+)-/.exec(m);
  return x ? x[1][0].toUpperCase() + x[1].slice(1) : m;
}
