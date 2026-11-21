import type {ReactNode} from 'react';
import {stat, runsOf, cellsOf, type LedgerEntry, type LedgerSetting, type MetricDef, type Stat} from './evaluation';

/**
 * One metric over the releases: x is the release (0.1.0 first, later releases appended by the
 * data), one series per setting. The dot is the median of the runs that have the value, the
 * whisker spans min to max, and n is printed. No line joins the releases, as in the progress
 * chart: with three runs a line would claim more than the runs show. A release without runs
 * says "no data", never 0. Series differ by shape and by position, not by colour alone.
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

const H = 196;
const TOP = 16;
const BOTTOM = 42;
const LEFT = 54;
const RIGHT = 10;

function geometry(releases: LedgerEntry[], settings: LedgerSetting[]) {
  const slot = 30;
  const col = Math.max(96, settings.length * slot + 26);
  const width = LEFT + releases.length * col + RIGHT;
  const x = (ri: number, si: number) => LEFT + ri * col + col / 2 + (si - (settings.length - 1) / 2) * slot;
  return {col, width, x};
}

export default function MetricChart({metric, releases, settings}: {metric: MetricDef; releases: LedgerEntry[]; settings: LedgerSetting[]}): ReactNode {
  const stats: Array<Array<Stat | null>> = releases.map((e) => settings.map((s) => stat(runsOf(e, s.id).map(metric.get))));
  const has = releases.map((e) => settings.some((s) => cellsOf(e, s.id).length > 0));
  const all = stats.flat().filter((s): s is Stat => s !== null);
  const floor = metric.unit === 'time' ? 60 : 1;
  const max = metric.unit === 'share' ? 1 : niceMax(Math.max(floor, ...all.map((s) => s.max)));
  const {col, width, x} = geometry(releases, settings);
  const plotH = H - TOP - BOTTOM;
  const y = (v: number) => TOP + plotH - (v / max) * plotH;
  const ticks = [0, max / 2, max];
  const id = `metric-${metric.id.replace(/[^a-z0-9-]/gi, '-')}`;
  const desc = releases
    .map((e, ri) =>
      !has[ri]
        ? `${e.label}: no data`
        : `${e.label}: ` +
          settings
            .map((s, si) => {
              const st = stats[ri][si];
              return st ? `${s.label} median ${metric.format(st.median)}, min ${metric.format(st.min)}, max ${metric.format(st.max)}, n ${st.n}` : `${s.label} no value`;
            })
            .join('; '),
    )
    .join('. ');
  return (
    <figure className="sdc-metric" id={id}>
      <figcaption>
        <strong>{metric.title}</strong>
        <span className="sdc-metric__dir">{metric.direction}</span>
      </figcaption>
      <svg viewBox={`0 0 ${width} ${H}`} width={width} height={H} role="img" aria-label={`${metric.title}. ${metric.direction} ${desc}.`} className="sdc-metric__svg">
        {ticks.map((t) => (
          <g key={t}>
            <line x1={LEFT} x2={width - RIGHT} y1={y(t)} y2={y(t)} className="sdc-metric__grid" />
            <text x={LEFT - 6} y={y(t) + 4} textAnchor="end" className="sdc-metric__tick">{metric.format(t)}</text>
          </g>
        ))}
        {releases.map((e, ri) => (
          <g key={e.id}>
            <text x={LEFT + ri * col + col / 2} y={H - 8} textAnchor="middle" className="sdc-metric__release">{e.label}</text>
            {!has[ri] ? (
              <text x={LEFT + ri * col + col / 2} y={TOP + plotH / 2 + 4} textAnchor="middle" className="sdc-metric__nodata">no data</text>
            ) : (
              settings.map((s, si) => {
                const st = stats[ri][si];
                const cx = x(ri, si);
                if (!st) return <text key={s.id} x={cx} y={TOP + plotH - 4} textAnchor="middle" className="sdc-metric__n">–</text>;
                return (
                  <g key={s.id}>
                    {st.max > st.min ? <line x1={cx} x2={cx} y1={y(st.min)} y2={y(st.max)} className={`sdc-metric__whisker sdc-series--${si % SHAPES.length}`} /> : null}
                    {st.max > st.min ? <line x1={cx - 4} x2={cx + 4} y1={y(st.max)} y2={y(st.max)} className={`sdc-metric__whisker sdc-series--${si % SHAPES.length}`} /> : null}
                    {st.max > st.min ? <line x1={cx - 4} x2={cx + 4} y1={y(st.min)} y2={y(st.min)} className={`sdc-metric__whisker sdc-series--${si % SHAPES.length}`} /> : null}
                    <Marker i={si} x={cx} y={y(st.median)} />
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

/** Clean passes as counts: per release and setting one dot per ended run, filled for a clean pass. */
export function CleanPassChart({releases, settings}: {releases: LedgerEntry[]; settings: LedgerSetting[]}): ReactNode {
  const {col, width, x} = geometry(releases, settings);
  const rows = Math.max(3, ...releases.flatMap((e) => settings.map((s) => runsOf(e, s.id).length)));
  const height = TOP + rows * 16 + BOTTOM + 6;
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
    <figure className="sdc-metric sdc-metric--wide" id="metric-clean-passes">
      <figcaption>
        <strong>Clean passes, k of n ended runs</strong>
        <span className="sdc-metric__dir">Higher is better. A filled dot is a clean pass, a ring a run that ended without one.</span>
      </figcaption>
      <svg viewBox={`0 0 ${width} ${height}`} width={width} height={height} role="img" aria-label={`Clean passes. Higher is better. ${desc}.`} className="sdc-metric__svg">
        {releases.map((e, ri) => {
          const any = settings.some((s) => cellsOf(e, s.id).length);
          return (
            <g key={e.id}>
              <text x={LEFT + ri * col + col / 2} y={height - 8} textAnchor="middle" className="sdc-metric__release">{e.label}</text>
              {!any ? (
                <text x={LEFT + ri * col + col / 2} y={TOP + (rows * 16) / 2} textAnchor="middle" className="sdc-metric__nodata">no data</text>
              ) : (
                settings.map((s, si) => {
                  const runs = runsOf(e, s.id);
                  if (!cellsOf(e, s.id).length) return null;
                  const cx = x(ri, si);
                  const k = runs.filter((r) => r.clean_pass).length;
                  return (
                    <g key={s.id}>
                      {runs.map((r, i) => (
                        <circle key={r.run} cx={cx} cy={TOP + rows * 16 - i * 16 - 6} r={5} className={r.clean_pass ? 'sdc-dot sdc-dot--clean' : 'sdc-dot sdc-dot--ring'} />
                      ))}
                      <text x={cx} y={TOP + rows * 16 + 12} textAnchor="middle" className="sdc-metric__k">{k}/{runs.length}</text>
                      <g transform={`translate(0 ${TOP + rows * 16 + 18})`} aria-hidden="true"><Marker i={si} x={cx} y={4} r={3.5} /></g>
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
