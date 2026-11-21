import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import type {Executor, PredLine} from './types';

/** Inline Markdown as the release notes write it: `code` spans only, the rest as text. */
export function InlineMd({text}: {text: string}): ReactNode {
  const parts = text.split(/(`[^`]+`)/g);
  return <>{parts.map((p, i) => (p.startsWith('`') && p.endsWith('`') && p.length > 1 ? <code key={i}>{p.slice(1, -1)}</code> : <span key={i}>{p}</span>))}</>;
}

export function PredicateList({lines, onRef}: {lines: PredLine[]; onRef?: (ref: string) => void}): ReactNode {
  if (!lines.length) return <p className="sdc-muted">always</p>;
  return (
    <ul className="sdc-pred">
      {lines.map((l, i) => (
        <li key={i} style={{['--sdc-depth' as string]: l.depth}}>
          <span className="sdc-pred__text">{l.text}</span>
          {l.reference ? (
            onRef ? (
              <button type="button" className="sdc-ref" onClick={() => onRef(l.reference!)} aria-label={`Select ${l.reference.replace(':', ' ')} in the graph`}>
                <code>{l.reference.slice(l.reference.indexOf(':') + 1)}</code>
              </button>
            ) : (
              <code>{l.reference.slice(l.reference.indexOf(':') + 1)}</code>
            )
          ) : null}
          {l.suffix ? <span className="sdc-pred__suffix"> {l.suffix}</span> : null}
        </li>
      ))}
    </ul>
  );
}

export const EXECUTOR_LABEL: Record<Executor['kind'], string> = {
  program: 'Program',
  model: 'Model call',
  person: 'Person',
  'child-case': 'Opens child case',
  copy: 'Copy',
};

export function executorLabel(e: Executor): string {
  if (e.kind === 'person') return e.bound ? 'Person (bound)' : 'Person (unbound)';
  return EXECUTOR_LABEL[e.kind];
}

export function ExecutorChip({executor}: {executor: Executor}): ReactNode {
  return (
    <span className={`sdc-chip sdc-chip--${executor.kind}`}>
      <span className="sdc-chip__glyph" aria-hidden="true">{GLYPH[executor.kind]}</span>
      {executorLabel(executor)}
    </span>
  );
}

const GLYPH: Record<Executor['kind'], string> = {program: '▸', model: '◆', person: '●', 'child-case': '↳', copy: '⇄'};

export function Fact({label, value, href}: {label: string; value: ReactNode; href?: string}): ReactNode {
  return (
    <div className="sdc-fact">
      <dt>{label}</dt>
      <dd>{href ? <Link href={href}>{value}</Link> : value}</dd>
    </div>
  );
}

export function protocolHref(stem: string): string {
  return `/protocols/${stem}`;
}
