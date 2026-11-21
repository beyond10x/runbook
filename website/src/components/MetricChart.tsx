import type {ReactNode} from 'react';
import {stat, runsOf, cellsOf, type LedgerEntry, type LedgerSetting, type MetricDef, type Stat} from './evaluation';

/**
 * One metric over the releases: x is the release (0.1.0 first, later releases appended by the
 * data), one series per setting. The dot is the median of the runs that have the value, the
 * whisker spans min to max, and n is printed. No line joins the releases, as in the progress
 * chart: with three runs a line would claim more than the runs show. The page passes only the
 * releases that have runs, and the columns share the plot width, so one or two releases fill the
 * plot. Series differ by shape and by position, not by colour alone.
 */

const SHAPES = ['circle', 'square', 'diamond', 'triangle', 'cross'] as const;

export function Marker({i, x, y, r = 5}: {i: number; x: number; y: number; r?: number}): ReactNode {
  const shape = SHAPES[i % SHAPES.length];
  const cls = `sdc-series sdc-series--${i % SHAPES.length}`;
  if (shape === 'circle') return <circle className={cls} cx={x} cy={y} r={r} />;
  if (shape === 'square') return <rect className={cls} x={x - r * 0.9} y={y - r * 0.9} width={r * 1.8} height={r * 1.8} />;
  if (shape === 'diamond') return <path className={cls} d={`M${x},${y - r * 1.25} L${x + r * 1.25},${y} L${x},${y + r * 1.25} L${x - r * 1.25},${y} Z`} />;
  if (shape === 'triangle') return <path className={cls} d={`M${x},${y - r * 1.2} L${x + r * 1.15},${y + r * 0.9} L${x - r * 1.15},${y + r * 0.9} Z`} />;
  return <path className={`${cls} sdc-series--stroke`} d={`M${x - r},${y - r} L${x + r},${y + r} M${x - r},${y + r} L${x + r},${y - r}`} />;
}

export function MarkerIcon({i, size = 12}: {i: number; size?: number}): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true" className="sdc-marker-icon">
      <Marker i={i} x={6} y={6} r={3.5} />
    </svg>
  );
}

export function SeriesLegend({settings}: {settings: LedgerSetting[]}): ReactNode {
  return (
    <ul className="sdc-legend" aria-label="Settings">
      {settings.map((s, i) => (
        <li key={s.id}>
          <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><Marker i={i} x={8} y={8} r={4.5} /></svg>
          <span><strong>{s.label}</strong> <code>{s.id}</code></span>
        </li>
      ))}
    </ul>
  );
}

function niceMax(v: number): number {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  for (const k of [1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10]) if (k * p >= v) return k * p;
  return 10 * p;
}

export type ChartSize = 'large' | 'small';

/** Plot frame per size. The svg fills its card; the view box sets the scale of its text. */
const FRAME = {
  large: {W: 480, H: 268, TOP: 18, BOTTOM: 44, LEFT: 56, RIGHT: 12, SLOT: 34, R: 6},
  small: {W: 360, H: 196, TOP: 16, BOTTOM: 42, LEFT: 50, RIGHT: 10, SLOT: 28, R: 5},
} as const;

function geometry(releases: LedgerEntry[], settings: LedgerSetting[], size: ChartSize) {
  const f = FRAME[size];
  const col = (f.W - f.LEFT - f.RIGHT) / Math.max(1, releases.length);
  const slot = Math.min(f.SLOT, (col - 16) / Math.max(1, settings.length));
  const center = (ri: number) => f.LEFT + ri * col + col / 2;
  const x = (ri: number, si: number) => center(ri) + (si - (settings.length - 1) / 2) * slot;
  return {...f, col, center, x};
}

/** A horizontal line drawn across the plot: a target or a reference measurement. */
export interface RefLine {
  value: number;
  /** The full label, in the key under the caption. */
  label: string;
  /** The short tag written on the line. */
  tag: string;
  kind: 'target' | 'reference';
}

/** Axis maximum for seconds: a round number of minutes, so the ticks fall on whole minutes. */
function niceTime(seconds: number): number {
  const m = seconds / 60;
  for (const k of [1, 2, 4, 8, 12, 20, 40, 60, 80, 120, 160, 240, 360, 480]) if (k >= m) return k * 60;
  return niceMax(m) * 60;
}

/** Where a tag on a line can go without covering a mark: right end, between columns, left end. */
function placeTag(
  tag: string,
  ly: number,
  obstacles: Array<{x: number; y0: number; y1: number}>,
  bounds: {left: number; right: number; gaps: number[]},
): {x: number; anchor: 'start' | 'middle' | 'end'} {
  const w = tag.length * 8.8 + 6;
  const top = ly - 17;
  const bottom = ly - 1;
  const candidates: Array<{x: number; anchor: 'start' | 'middle' | 'end'; x0: number; x1: number}> = [
    {x: bounds.right - 4, anchor: 'end', x0: bounds.right - 4 - w, x1: bounds.right},
    ...bounds.gaps.map((g) => ({x: g, anchor: 'middle' as const, x0: g - w / 2, x1: g + w / 2})),
    {x: bounds.left + 4, anchor: 'start', x0: bounds.left, x1: bounds.left + 4 + w},
  ];
  const free = candidates.find((c) => obstacles.every((o) => o.x < c.x0 - 8 || o.x > c.x1 + 8 || o.y1 < top || o.y0 > bottom));
  const pick = free ?? candidates[0];
  return {x: pick.x, anchor: pick.anchor};
}

export default function MetricChart({
  metric,
  releases,
  settings,
  size = 'small',
  refs = [],
}: {
  metric: MetricDef;
  releases: LedgerEntry[];
  settings: LedgerSetting[];
  size?: ChartSize;
  refs?: RefLine[];
}): ReactNode {
  const stats: Array<Array<Stat | null>> = releases.map((e) => settings.map((s) => stat(runsOf(e, s.id).map(metric.get))));
  const has = releases.map((e) => settings.some((s) => cellsOf(e, s.id).length > 0));
  const all = stats.flat().filter((s): s is Stat => s !== null);
  const floor = metric.unit === 'time' ? 60 : 1;
  const top = Math.max(floor, ...all.map((s) => s.max), ...refs.map((r) => r.value));
  const max = metric.unit === 'share' ? 1 : metric.unit === 'time' ? niceTime(top) : niceMax(top);
  const g = geometry(releases, settings, size);
  const {W, H, TOP, BOTTOM, LEFT, RIGHT, R} = g;
  const plotH = H - TOP - BOTTOM;
  const y = (v: number) => TOP + plotH - (v / max) * plotH;
  const ticks = size === 'large' ? [0, max / 4, max / 2, (3 * max) / 4, max] : [0, max / 2, max];
  const tickFormat = metric.unit === "time" && max >= 120 ? (t: number) => `${Math.round(t / 60)} min` : metric.format;
  const id = `metric-${metric.id.replace(/[^a-z0-9-]/gi, '-')}`;
  const obstacles = releases.flatMap((_, ri) =>
    settings.flatMap((s, si) => {
      const st = stats[ri][si];
      return st ? [{x: g.x(ri, si), y0: y(st.max) - R - 2, y1: y(st.min) + R + 2}] : [];
    }),
  );
  const gaps = releases.slice(1).map((_, i) => LEFT + (i + 1) * g.col);
  const lines = mergeRefs(refs, y).map((l) => ({...l, ...placeTag(l.tag, l.y, obstacles, {left: LEFT, right: W - RIGHT, gaps})}));
  const desc = releases
    .map((e, ri) =>
      !has[ri]
        ? `${e.label}: no data`
        : `${e.label}: ` +
          settings
            .map((s, si) => {
              const st = stats[ri][si];
              return st ? `${s.label} median ${metric.format(st.median)}, min ${metric.format(st.min)}, max ${metric.format(st.max)}, n ${st.n}` : `${s.label} ${!cellsOf(e, s.id).length ? 'no runs' : metric.missing ? metric.missing(runsOf(e, s.id)) : 'no value'}`;
            })
            .join('; '),
    )
    .join('. ');
  const refDesc = refs.length ? ` Lines: ${refs.map((r) => r.label).join('; ')}.` : '';
  return (
    <figure className={`sdc-metric sdc-metric--${size}`} id={id}>
      <figcaption>
        <strong>
          {metric.title}
          {metric.tag ? <span className="sdc-metric__tag"> ({metric.tag})</span> : null}
        </strong>
        <span className="sdc-metric__dir">{metric.direction}</span>
        {refs.length ? (
          <ul className="sdc-metric__refkey">
            {refs.map((r) => (
              <li key={r.label} className={`sdc-metric__key sdc-metric__key--${r.kind}`}>
                <svg width="22" height="8" viewBox="0 0 22 8" aria-hidden="true"><line x1="0" x2="22" y1="4" y2="4" className="sdc-metric__refline" /></svg>
                {r.label}
              </li>
            ))}
          </ul>
        ) : null}
      </figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={`${metric.title}${metric.tag ? `, ${metric.tag}` : ''}. ${metric.direction} ${desc}.${refDesc}`} className="sdc-metric__svg">
        {ticks.map((t) => (
          <g key={t}>
            <line x1={LEFT} x2={W - RIGHT} y1={y(t)} y2={y(t)} className="sdc-metric__grid" />
            <text x={LEFT - 6} y={y(t) + 4} textAnchor="end" className="sdc-metric__tick">{tickFormat(t)}</text>
          </g>
        ))}
        {lines.map((l) => (
          <g key={l.label} className={`sdc-metric__ref sdc-metric__ref--${l.kind}`}>
            <line x1={LEFT} x2={W - RIGHT} y1={l.y} y2={l.y} className="sdc-metric__refline" />
            <text x={l.x} y={l.y - 5} textAnchor={l.anchor} className="sdc-metric__reflabel">{l.tag}</text>
          </g>
        ))}
        {releases.map((e, ri) => (
          <g key={e.id}>
            <text x={g.center(ri)} y={H - 8} textAnchor="middle" className="sdc-metric__release">{e.label}</text>
            {!has[ri] ? (
              <text x={g.center(ri)} y={TOP + plotH / 2 + 4} textAnchor="middle" className="sdc-metric__nodata">no data</text>
            ) : (
              settings.map((s, si) => {
                const st = stats[ri][si];
                const cx = g.x(ri, si);
                if (!st && metric.missing && cellsOf(e, s.id).length) {
                  const my = TOP + plotH / 2;
                  return (
                    <text key={s.id} x={cx} y={my} transform={`rotate(-90 ${cx} ${my})`} textAnchor="middle" dominantBaseline="middle" className="sdc-metric__missing">
                      {metric.missing(runsOf(e, s.id))}
                    </text>
                  );
                }
                if (!st) return <text key={s.id} x={cx} y={TOP + plotH - 4} textAnchor="middle" className="sdc-metric__n">–</text>;
                return (
                  <g key={s.id}>
                    {st.max > st.min ? <line x1={cx} x2={cx} y1={y(st.min)} y2={y(st.max)} className={`sdc-metric__whisker sdc-series--${si % SHAPES.length}`} /> : null}
                    {st.max > st.min ? <line x1={cx - 4} x2={cx + 4} y1={y(st.max)} y2={y(st.max)} className={`sdc-metric__whisker sdc-series--${si % SHAPES.length}`} /> : null}
                    {st.max > st.min ? <line x1={cx - 4} x2={cx + 4} y1={y(st.min)} y2={y(st.min)} className={`sdc-metric__whisker sdc-series--${si % SHAPES.length}`} /> : null}
                    <Marker i={si} x={cx} y={y(st.median)} r={R} />
                    <text x={cx} y={TOP + plotH + 14} textAnchor="middle" className="sdc-metric__n">n={st.n}</text>
                  </g>
                );
              })
            )}
          </g>
        ))}
      </svg>
    </figure>
  );
}

/** Lines closer than one label height share one line and one label. */
function mergeRefs(refs: RefLine[], y: (v: number) => number): Array<{y: number; label: string; tag: string; kind: RefLine['kind']}> {
  const out: Array<{y: number; label: string; tag: string; kind: RefLine['kind']}> = [];
  for (const r of [...refs].sort((a, b) => a.value - b.value)) {
    const ry = y(r.value);
    const near = out.find((o) => Math.abs(o.y - ry) < 12);
    if (near) {
      near.label = `${near.label} · ${r.label}`;
      if (r.kind === 'target') {
        near.kind = 'target';
        near.tag = r.tag;
      } else if (near.kind !== 'target') near.tag = `${near.tag} · ${r.tag}`;
    } else out.push({y: ry, label: r.label, tag: r.tag, kind: r.kind});
  }
  return out;
}

/** Clean passes as counts: per release and setting one dot per ended run, filled for a clean pass. */
export function CleanPassChart({releases, settings, size = 'large'}: {releases: LedgerEntry[]; settings: LedgerSetting[]; size?: ChartSize}): ReactNode {
  const g = geometry(releases, settings, size);
  const {W, TOP, BOTTOM, R} = g;
  const step = R * 3;
  const rows = Math.max(3, ...releases.flatMap((e) => settings.map((s) => runsOf(e, s.id).length)));
  const base = TOP + rows * step;
  const height = base + BOTTOM + 8;
  const desc = releases
    .map((e) => {
      const cells = settings.filter((s) => cellsOf(e, s.id).length);
      if (!cells.length) return `${e.label}: no data`;
      return `${e.label}: ` + cells.map((s) => {
        const r = runsOf(e, s.id);
        return `${s.label} ${r.filter((q) => q.clean_pass).length} of ${r.length}`;
      }).join('; ');
    })
    .join('. ');
  return (
    <figure className={`sdc-metric sdc-metric--${size}`} id="metric-clean-passes">
      <figcaption>
        <strong>Clean passes, k of n ended runs</strong>
        <span className="sdc-metric__dir">Higher is better. A filled dot is a clean pass, a ring a run that ended without one.</span>
      </figcaption>
      <svg viewBox={`0 0 ${W} ${height}`} role="img" aria-label={`Clean passes. Higher is better. ${desc}.`} className="sdc-metric__svg">
        <line x1={g.LEFT} x2={W - g.RIGHT} y1={base} y2={base} className="sdc-metric__grid" />
        {releases.map((e, ri) => {
          const any = settings.some((s) => cellsOf(e, s.id).length);
          return (
            <g key={e.id}>
              <text x={g.center(ri)} y={height - 8} textAnchor="middle" className="sdc-metric__release">{e.label}</text>
              {!any ? (
                <text x={g.center(ri)} y={TOP + (rows * step) / 2} textAnchor="middle" className="sdc-metric__nodata">no data</text>
              ) : (
                settings.map((s, si) => {
                  const runs = runsOf(e, s.id);
                  if (!cellsOf(e, s.id).length) return null;
                  const cx = g.x(ri, si);
                  const k = runs.filter((r) => r.clean_pass).length;
                  return (
                    <g key={s.id}>
                      {runs.map((r, i) => (
                        <circle key={r.run} cx={cx} cy={base - i * step - step / 2} r={R} className={r.clean_pass ? 'sdc-dot sdc-dot--clean' : 'sdc-dot sdc-dot--ring'} />
                      ))}
                      <text x={cx} y={base + 14} textAnchor="middle" className="sdc-metric__k">{k}/{runs.length}</text>
                      <g transform={`translate(0 ${base + 20})`} aria-hidden="true"><Marker i={si} x={cx} y={4} r={3.5} /></g>
                    </g>
                  );
                })
              )}
            </g>
          );
        })}
        <text x={4} y={TOP + 10} className="sdc-metric__tick">runs</text>
      </svg>
    </figure>
  );
}

/** The cost classes in drawing order, each with its CSS class; a class without cost is left out. */
export const COST_CLASSES: Array<{id: string; label: string}> = [
  {id: 'other', label: 'intake and direction (other)'},
  {id: 'writer', label: 'writer'},
  {id: 'implement', label: 'implement'},
  {id: 'review', label: 'review'},
  {id: 'decider', label: 'decider'},
  {id: 'unstick', label: 'unstick'},
  {id: 'unclassified', label: 'no class'},
];

export interface CostSplitRow {
  setting: LedgerSetting;
  index: number;
  n: number;
  mean: Record<string, number>;
  total: number;
}

const usd = (x: number) => `$${x.toFixed(2)}`;

/**
 * Where the cost of a run goes: per setting one stacked bar of the mean cost per run by node
 * class, all bars on one scale, and the values in a table under it.
 */
export function CostSplitChart({release, rows}: {release: string; rows: CostSplitRow[]}): ReactNode {
  const classes = COST_CLASSES.filter((c) => rows.some((r) => (r.mean[c.id] ?? 0) > 0));
  const W = 480;
  const LEFT = 8;
  const RIGHT = 60;
  const ROW = 50;
  const TOP = 6;
  const height = TOP + rows.length * ROW + 4;
  const max = Math.max(...rows.map((r) => r.total));
  const scale = (v: number) => (v / max) * (W - LEFT - RIGHT);
  const desc = rows
    .map((r) => `${r.setting.label}, n ${r.n}: ${usd(r.total)}, of which ${classes.filter((c) => r.mean[c.id]).map((c) => `${c.label} ${usd(r.mean[c.id])}`).join(', ')}`)
    .join('. ');
  return (
    <figure className="sdc-metric sdc-metric--large" id="cost-split">
      <figcaption>
        <strong>Where the cost of a run goes, release {release}</strong>
        <span className="sdc-metric__dir">Mean cost per run by node class, from the node costs in the run summaries. The decider's cost is counted apart from the node it answered for.</span>
      </figcaption>
      <svg viewBox={`0 0 ${W} ${height}`} role="img" aria-label={`Where the cost of a run goes, release ${release}. ${desc}.`} className="sdc-metric__svg">
        {rows.map((r, ri) => {
          const y0 = TOP + ri * ROW;
          let x = LEFT;
          return (
            <g key={r.setting.id} className="sdc-split__row">
              <g aria-hidden="true"><Marker i={r.index} x={LEFT + 5} y={y0 + 9} r={4} /></g>
              <text x={LEFT + 16} y={y0 + 13} className="sdc-split__label">{r.setting.label} <tspan className="sdc-split__n">n={r.n}</tspan></text>
              {classes.map((c) => {
                const v = r.mean[c.id] ?? 0;
                if (!v) return null;
                const w = scale(v);
                const seg = <rect key={c.id} x={x} y={y0 + 20} width={w} height={20} className={`sdc-split__seg sdc-cls--${c.id}`} />;
                x += w;
                return seg;
              })}
              <text x={x + 6} y={y0 + 35} className="sdc-split__total">{usd(r.total)}</text>
            </g>
          );
        })}
      </svg>
      <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Mean cost per run by node class">
        <table className="sdc-table sdc-table--compact sdc-split__table">
          <thead>
            <tr>
              <th scope="col">Class</th>
              {rows.map((r) => <th key={r.setting.id} scope="col"><MarkerIcon i={r.index} /> {r.setting.id}</th>)}
            </tr>
          </thead>
          <tbody>
            {classes.map((c) => (
              <tr key={c.id}>
                <th scope="row"><span className={`sdc-split__swatch sdc-cls--${c.id}`} aria-hidden="true" /> {c.label}</th>
                {rows.map((r) => <td key={r.setting.id}>{r.mean[c.id] ? usd(r.mean[c.id]) : '–'}</td>)}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </figure>
  );
}
