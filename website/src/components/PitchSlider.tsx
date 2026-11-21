import {useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactNode, type TouchEvent} from 'react';
import Link from '@docusaurus/Link';
import styles from './PitchSlider.module.css';

/** The landing pitch data, written by the sdc-data plugin from `docs/progress.json`. */
export interface PitchData {
  release: string | null;
  input?: string | null;
  /** A free-agent comparison record is committed, so the LLM page shows what an SDC run records. */
  comparison?: boolean;
  settings: Array<{id: string; label: string; clean_passes: number; ended: number; runs: Array<{clean_pass: boolean; waiting_on_person_s: number | null}>}>;
}

interface Props {
  pitch: PitchData;
  /** All protocols of the site index. */
  protocols: number;
  /** The protocols an idea goes through (all but the bug fix). */
  stages: number;
}

interface Slide {
  kicker: string;
  claim: string;
  line: string;
  link: {to: string; label: string};
  graphic: ReactNode;
}

const INTERVAL_MS = 6000;
const SWIPE_PX = 40;

function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    const q = window.matchMedia('(prefers-reduced-motion: reduce)');
    setReduced(q.matches);
    const on = () => setReduced(q.matches);
    q.addEventListener('change', on);
    return () => q.removeEventListener('change', on);
  }, []);
  return reduced;
}

function Box({x, y, w, h, title, sub, tone, dashed}: {x: number; y: number; w: number; h: number; title: string; sub: string; tone?: string; dashed?: boolean}): ReactNode {
  return (
    <g>
      <rect x={x} y={y} width={w} height={h} rx={8} className={dashed ? styles.boxDashed : styles.box} />
      {tone ? <rect x={x} y={y} width={5} height={h} rx={2} className={styles[tone]} /> : null}
      <text x={x + 14} y={y + 21} className={styles.gTitle}>{title}</text>
      <text x={x + 14} y={y + 39} className={styles.gSub}>{sub}</text>
    </g>
  );
}

function Arrow({d, dashed}: {d: string; dashed?: boolean}): ReactNode {
  return <path d={d} className={dashed ? styles.edgeDashed : styles.edge} markerEnd="url(#sdc-pitch-arrow)" />;
}

/** Slide 1: from the idea through the stages to a delivered app, and one dot per run of the release. */
function RunsGraphic({pitch, stages}: {pitch: PitchData; stages: number}): ReactNode {
  const rows = pitch.settings;
  const h = 104 + rows.length * 40;
  const dots = Array.from({length: stages}, (_, i) => 92 + (i * (246 - 92)) / Math.max(1, stages - 1));
  const label = rows.length
    ? `Clean passes, release ${pitch.release}: ${rows.map((s) => `${s.label} ${s.clean_passes} of ${s.ended}`).join('; ')}.`
    : `An idea goes through ${stages} stages to a delivered app.`;
  return (
    <svg viewBox={`0 0 360 ${h}`} className={styles.svg} role="img" aria-label={label}>
      <line x1={70} y1={30} x2={262} y2={30} className={styles.rail} />
      <g>
        <rect x={4} y={14} width={66} height={32} rx={16} className={styles.pill} />
        <text x={37} y={35} textAnchor="middle" className={styles.gTitle}>idea</text>
      </g>
      {dots.map((x, i) => <circle key={i} cx={x} cy={30} r={5} className={styles.stage} />)}
      <g>
        <rect x={262} y={14} width={94} height={32} rx={16} className={styles.pillDone} />
        <text x={309} y={35} textAnchor="middle" className={styles.gTitle}>delivered</text>
      </g>
      <text x={169} y={64} textAnchor="middle" className={styles.gSub}>{stages} stages, under autopilot</text>
      {rows.length ? (
        <>
          <text x={4} y={96} className={styles.gHead}>Clean passes, release {pitch.release}</text>
          {rows.map((s, r) => {
            const y = 124 + r * 40;
            return (
              <g key={s.id}>
                <text x={4} y={y + 5} className={styles.gRow}>{s.label}</text>
                {s.runs.map((run, i) => (
                  <circle key={i} cx={222 + i * 22} cy={y} r={8} className={run.clean_pass ? 'sdc-dot sdc-dot--clean' : 'sdc-dot sdc-dot--ring'} />
                ))}
                <text x={356} y={y + 5} textAnchor="end" className={styles.gCount}>{s.clean_passes} of {s.ended}</text>
              </g>
            );
          })}
        </>
      ) : null}
    </svg>
  );
}

/** Slide 2: evidence and the protocols go into Canon; a program takes the step; a model only revises an artifact. */
function StepGraphic({protocols}: {protocols: number}): ReactNode {
  return (
    <svg viewBox="0 0 360 252" className={styles.svg} role="img" aria-label={`Recorded evidence and ${protocols} YAML protocols go into Canon, which lists what is admissible next. A program takes one step and may run a model to revise one artifact; the step's result is recorded as new evidence.`}>
      <Box x={18} y={8} w={150} h={52} title="Evidence" sub="recorded facts" tone="toneEvidence" />
      <Box x={192} y={8} w={164} h={52} title={`${protocols} protocols`} sub="YAML files" tone="toneOutcome" />
      <Box x={78} y={100} w={220} h={52} title="Canon" sub="what is admissible next" />
      <Box x={18} y={192} w={150} h={52} title="Program" sub="takes one step" tone="toneAction" />
      <Box x={192} y={192} w={164} h={52} title="Model" sub="revises one artifact" tone="toneClaim" dashed />
      <Arrow d="M 110 60 L 140 96" />
      <Arrow d="M 260 60 L 230 96" />
      <Arrow d="M 150 152 L 110 188" />
      <Arrow d="M 168 218 L 188 218" dashed />
      <path d="M 18 218 L 6 218 L 6 34 L 14 34" className={styles.edge} markerEnd="url(#sdc-pitch-arrow)" />
    </svg>
  );
}

/** Slide 3: a hash-chained event log, replay against it, and a simulation that finds a dead end. */
function RecordGraphic(): ReactNode {
  const events = ['action_started', 'revised', 'evaluated'];
  return (
    <svg viewBox="0 0 360 252" className={styles.svg} role="img" aria-label="Events in a hash-chained log, each holding the hash of the one before. Replay evaluates every case again and compares the digests with the log. A simulation walks a protocol's states and marks a state with no next step.">
      <text x={4} y={14} className={styles.gHead}>Event log</text>
      {events.map((e, i) => {
        const x = 2 + i * 120;
        return (
          <g key={e}>
            <rect x={x} y={24} width={112} height={50} rx={7} className={styles.box} />
            <text x={x + 8} y={44} className={styles.gSub}>{i ? 'prev = hash' : 'event'}</text>
            <text x={x + 8} y={63} className={styles.gMonoSm}>{e}</text>
            {i < events.length - 1 ? <Arrow d={`M ${x + 112} 49 L ${x + 120} 49`} /> : null}
          </g>
        );
      })}
      <text x={4} y={104} className={styles.gHead}>Replay</text>
      <g>
        <rect x={2} y={114} width={170} height={40} rx={7} className={styles.box} />
        <text x={14} y={139} className={styles.gMono}>evaluate again</text>
      </g>
      <Arrow d="M 172 134 L 200 134" />
      <g>
        <rect x={204} y={114} width={152} height={40} rx={7} className={styles.boxOk} />
        <text x={216} y={139} className={styles.gTitle}>✓ digests match</text>
      </g>
      <text x={4} y={184} className={styles.gHead}>Simulation, before a run</text>
      <circle cx={20} cy={222} r={9} className={styles.stage} />
      <circle cx={90} cy={206} r={9} className={styles.stage} />
      <circle cx={90} cy={238} r={9} className={styles.stage} />
      <circle cx={160} cy={206} r={9} className={styles.stage} />
      <circle cx={160} cy={238} r={11} className={styles.stuck} />
      <Arrow d="M 29 218 L 78 208" />
      <Arrow d="M 29 226 L 78 236" />
      <Arrow d="M 99 206 L 148 206" />
      <Arrow d="M 99 238 L 146 238" />
      <text x={182} y={243} className={styles.gTitle}>dead end found</text>
    </svg>
  );
}

function slides({pitch, protocols, stages}: Props): Slide[] {
  const runs = pitch.settings.flatMap((s) => s.runs);
  const clean = pitch.settings.reduce((a, s) => a + s.clean_passes, 0);
  const ended = pitch.settings.reduce((a, s) => a + s.ended, 0);
  const waited = runs.filter((r) => (r.waiting_on_person_s ?? 0) > 0).length;
  const noPerson = runs.length > 0 && waited === 0 && runs.every((r) => r.waiting_on_person_s !== null);
  return [
    {
      kicker: 'What it does',
      claim: noPerson ? 'From an idea to a delivered app, unattended.' : 'From an idea to a delivered app.',
      line: pitch.release && ended
        ? `One idea, release ${pitch.release}: ${clean} of ${ended} runs were clean passes; ${waited === 0 ? 'no run' : `${waited} runs`} waited on a person.`
        : 'The clean passes per release and model setting are on the evaluation page.',
      link: {to: '/evaluation', label: 'Every run on the evaluation page'},
      graphic: <RunsGraphic pitch={pitch} stages={stages} />,
    },
    {
      kicker: 'Why it is simple',
      claim: 'The next step is computed from recorded facts.',
      line: `Canon reads ${protocols} YAML protocols and the evidence; a program takes the step; a model only revises an artifact.`,
      link: {to: '/docs/flow', label: 'The flow, drawn from the protocols'},
      graphic: <StepGraphic protocols={protocols} />,
    },
    {
      kicker: 'Why it is solid',
      claim: 'Every step is recorded and can be replayed.',
      line: 'Each step goes to a hash-chained log, replay re-checks every case against it, and simulation searches protocols for dead ends.',
      link: pitch.comparison
        ? {to: '/llm-is-not-the-runtime#free-records-h', label: 'What an SDC run records'}
        : {to: '/llm-is-not-the-runtime', label: 'The LLM is not the runtime'},
      graphic: <RecordGraphic />,
    },
  ];
}

export default function PitchSlider(props: Props): ReactNode {
  const items = slides(props);
  const n = items.length;
  const [current, setCurrent] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [hover, setHover] = useState(false);
  const [focus, setFocus] = useState(false);
  const reduced = useReducedMotion();
  const touch = useRef<{x: number; y: number} | null>(null);
  const rotating = playing && !reduced && !hover && !focus;

  useEffect(() => {
    if (!rotating) return;
    const t = window.setTimeout(() => setCurrent((c) => (c + 1) % n), INTERVAL_MS);
    return () => window.clearTimeout(t);
  }, [rotating, current, n]);

  const go = useCallback((i: number) => {
    setPlaying(false);
    setCurrent(((i % n) + n) % n);
  }, [n]);

  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'ArrowRight') { e.preventDefault(); go(current + 1); }
    if (e.key === 'ArrowLeft') { e.preventDefault(); go(current - 1); }
  };
  const onTouchStart = (e: TouchEvent) => { touch.current = {x: e.touches[0].clientX, y: e.touches[0].clientY}; };
  const onTouchEnd = (e: TouchEvent) => {
    const start = touch.current;
    touch.current = null;
    if (!start) return;
    const dx = e.changedTouches[0].clientX - start.x;
    const dy = e.changedTouches[0].clientY - start.y;
    if (Math.abs(dx) >= SWIPE_PX && Math.abs(dx) > Math.abs(dy)) go(current + (dx < 0 ? 1 : -1));
  };

  return (
    <section className={styles.section} aria-labelledby="pitch-title">
      <h2 id="pitch-title" className="b10x-sr-only">Runbook in three claims</h2>
      <div
        className={styles.carousel}
        role="region"
        aria-roledescription="carousel"
        aria-labelledby="pitch-title"
        tabIndex={0}
        data-pitch
        data-current={current}
        onKeyDown={onKey}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        onFocus={() => setFocus(true)}
        onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocus(false); }}
        onTouchStart={onTouchStart}
        onTouchEnd={onTouchEnd}
      >
        <svg width="0" height="0" className={styles.defs} aria-hidden="true" focusable="false">
          <defs>
            <marker id="sdc-pitch-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" className={styles.arrowHead} />
            </marker>
          </defs>
        </svg>
        <div className={styles.viewport} aria-live={rotating ? 'off' : 'polite'}>
          <div className={styles.track} style={{transform: `translateX(-${current * 100}%)`}}>
            {items.map((s, i) => (
              <div
                key={s.kicker}
                className={styles.slide}
                role="group"
                aria-roledescription="slide"
                aria-label={`${i + 1} of ${n}: ${s.kicker}`}
                data-pitch-slide={i}
                inert={i !== current}
              >
                <div className={styles.copy}>
                  <p className={styles.kicker}>{s.kicker}</p>
                  <h3 className={styles.claim} data-pitch-claim>{s.claim}</h3>
                  <p className={styles.line} data-pitch-line>{s.line}</p>
                  <p className={styles.more}><Link to={s.link.to} data-pitch-link>{s.link.label} <span aria-hidden="true">→</span></Link></p>
                </div>
                <div className={styles.figure}>{s.graphic}</div>
              </div>
            ))}
          </div>
        </div>
        <div className={styles.controls}>
          <button type="button" className={styles.arrow} onClick={() => go(current - 1)} aria-label="Previous slide" data-pitch-prev>
            <span aria-hidden="true">←</span>
          </button>
          <div className={styles.dots}>
            {items.map((s, i) => (
              <button
                key={s.kicker}
                type="button"
                className={styles.dot}
                onClick={() => go(i)}
                aria-label={`Slide ${i + 1}: ${s.kicker}`}
                aria-current={i === current ? 'true' : undefined}
                data-pitch-dot={i}
              >
                <span aria-hidden="true" />
              </button>
            ))}
          </div>
          <button type="button" className={styles.arrow} onClick={() => go(current + 1)} aria-label="Next slide" data-pitch-next>
            <span aria-hidden="true">→</span>
          </button>
          {!reduced ? (
            <button type="button" className={styles.play} onClick={() => setPlaying((p) => !p)} data-pitch-play>
              {playing ? 'Pause' : 'Play'}<span className="b10x-sr-only"> the slides</span>
            </button>
          ) : null}
        </div>
      </div>
    </section>
  );
}
